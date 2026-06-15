fn verify_pdf_signatures_report(
    input: &[u8],
    trust_anchors: &TrustAnchors,
    document: &lopdf::Document,
    limits: &ResourceLimits,
) -> Result<TextArtifact, OxideError> {
    let mut diagnostics = Vec::new();
    if trust_anchors.certificates.is_empty() {
        diagnostics.push(signature_diagnostic(
            "trust_anchors_missing",
            "no explicit trust anchors were provided; trusted conclusion is not possible",
        ));
    }
    let signatures = discover_pdf_signature_dictionaries(document)?
        .into_iter()
        .map(|dictionary| signature_entry_report(input, dictionary, trust_anchors))
        .collect::<Vec<_>>();

    if signatures.is_empty() {
        diagnostics.push(signature_diagnostic(
            "no_signatures",
            "PDF contains no signature dictionaries",
        ));
    }

    let verdict = overall_signature_verdict(&signatures, &diagnostics);
    let report = SignatureVerificationReport {
        verdict,
        trust_anchor_count: trust_anchors.certificates.len(),
        signatures,
        diagnostics,
    };
    let text = serde_json::to_string_pretty(&report).map_err(|_| OxideError::Internal)?;
    enforce_output_bytes(text.len(), limits)?;

    Ok(TextArtifact {
        text,
        diagnostics: Vec::new(),
    })
}

#[derive(Debug, Clone)]
struct DiscoveredSignatureDictionary<'a> {
    field_name: Option<String>,
    dictionary: &'a Dictionary,
}

#[derive(Debug, Clone)]
struct TrustAnchors {
    certificates: Vec<Certificate>,
}

fn load_trust_anchors(path: Option<&std::path::Path>) -> Result<TrustAnchors, OxideError> {
    let Some(path) = path else {
        return Ok(TrustAnchors {
            certificates: Vec::new(),
        });
    };
    let pem = std::fs::read(path).map_err(|_| OxideError::Io)?;
    let pem = std::str::from_utf8(&pem).map_err(|_| OxideError::InvalidInput {
        reason: "trust anchors file contains no valid PEM certificates".to_owned(),
    })?;
    let certificates = parsed_trust_anchors(pem)?;
    if certificates.is_empty() {
        return Err(OxideError::InvalidInput {
            reason: "trust anchors file contains no valid PEM certificates".to_owned(),
        });
    }

    Ok(TrustAnchors { certificates })
}

fn signature_list_entry(
    input: &[u8],
    discovered: DiscoveredSignatureDictionary<'_>,
) -> SignatureListEntry {
    let mut diagnostics = Vec::new();
    let byte_range = byte_range_verification(input, discovered.dictionary, &mut diagnostics);
    let contents = contents_verification(discovered.dictionary, &byte_range, &mut diagnostics);

    SignatureListEntry {
        field_name: discovered.field_name,
        subfilter: discovered
            .dictionary
            .get(b"SubFilter")
            .ok()
            .and_then(pdf_name),
        byte_range,
        contents,
    }
}

fn parsed_trust_anchors(pem: &str) -> Result<Vec<Certificate>, OxideError> {
    const BEGIN: &str = "-----BEGIN CERTIFICATE-----";
    const END: &str = "-----END CERTIFICATE-----";

    let mut rest = pem;
    let mut certificates = Vec::new();
    while let Some(begin) = rest.find(BEGIN) {
        rest = &rest[begin..];
        let Some(end) = rest.find(END) else {
            return Err(OxideError::InvalidInput {
                reason: "trust anchors file contains no valid PEM certificates".to_owned(),
            });
        };
        let block_end = end + END.len();
        let block = &rest[..block_end];
        let (label, der) =
            pem_rfc7468::decode_vec(block.as_bytes()).map_err(|_| OxideError::InvalidInput {
                reason: "trust anchors file contains no valid PEM certificates".to_owned(),
            })?;
        if label != "CERTIFICATE" {
            return Err(OxideError::InvalidInput {
                reason: "trust anchors file contains no valid PEM certificates".to_owned(),
            });
        }
        let certificate = Certificate::from_der(&der).map_err(|_| OxideError::InvalidInput {
            reason: "trust anchors file contains no valid PEM certificates".to_owned(),
        })?;
        certificates.push(certificate);
        rest = &rest[block_end..];
    }

    Ok(certificates)
}

fn discover_pdf_signature_dictionaries(
    document: &lopdf::Document,
) -> Result<Vec<DiscoveredSignatureDictionary<'_>>, OxideError> {
    let mut signatures = Vec::new();
    if let Ok(catalog) = document.catalog() {
        if let Ok(acroform) = catalog
            .get(b"AcroForm")
            .and_then(|object| deref_dictionary(document, object))
        {
            if let Ok(fields) = acroform.get(b"Fields").and_then(lopdf::Object::as_array) {
                for field in fields {
                    collect_signature_fields(document, field, None, &mut signatures)?;
                }
            }
        }
    }
    for (_, page_id) in document.get_pages() {
        let Ok(page) = document
            .get_object(page_id)
            .and_then(lopdf::Object::as_dict)
        else {
            continue;
        };
        let Ok(annots) = page.get(b"Annots").and_then(lopdf::Object::as_array) else {
            continue;
        };
        for annot in annots {
            collect_signature_fields(document, annot, None, &mut signatures)?;
        }
    }
    signatures.dedup_by_key(|signature| std::ptr::from_ref(signature.dictionary) as usize);

    Ok(signatures)
}

fn collect_signature_fields<'a>(
    document: &'a lopdf::Document,
    object: &'a lopdf::Object,
    inherited_name: Option<String>,
    signatures: &mut Vec<DiscoveredSignatureDictionary<'a>>,
) -> Result<(), OxideError> {
    let dictionary = deref_dictionary(document, object).map_err(|_| OxideError::ParsePdf)?;
    let field_name = dictionary
        .get(b"T")
        .ok()
        .and_then(pdf_string)
        .or(inherited_name);
    if dictionary.get(b"FT").and_then(lopdf::Object::as_name).ok() == Some(b"Sig") {
        if let Ok(value) = dictionary.get(b"V") {
            if let Ok(signature_dictionary) = deref_dictionary(document, value) {
                signatures.push(DiscoveredSignatureDictionary {
                    field_name: field_name.clone(),
                    dictionary: signature_dictionary,
                });
            }
        } else if dictionary.get(b"ByteRange").is_ok() {
            signatures.push(DiscoveredSignatureDictionary {
                field_name: field_name.clone(),
                dictionary,
            });
        }
    } else if dictionary.get(b"ByteRange").is_ok() && dictionary.get(b"Contents").is_ok() {
        signatures.push(DiscoveredSignatureDictionary {
            field_name: field_name.clone(),
            dictionary,
        });
    }
    if let Ok(kids) = dictionary.get(b"Kids").and_then(lopdf::Object::as_array) {
        for kid in kids {
            collect_signature_fields(document, kid, field_name.clone(), signatures)?;
        }
    }

    Ok(())
}

fn deref_dictionary<'a>(
    document: &'a lopdf::Document,
    object: &'a lopdf::Object,
) -> lopdf::Result<&'a Dictionary> {
    match object {
        lopdf::Object::Reference(id) => document.get_object(*id).and_then(lopdf::Object::as_dict),
        lopdf::Object::Dictionary(dictionary) => Ok(dictionary),
        _ => object.as_dict(),
    }
}

fn signature_entry_report(
    input: &[u8],
    discovered: DiscoveredSignatureDictionary<'_>,
    trust_anchors: &TrustAnchors,
) -> SignatureEntryReport {
    let mut diagnostics = Vec::new();
    let subfilter = discovered
        .dictionary
        .get(b"SubFilter")
        .ok()
        .and_then(pdf_name);
    if !matches!(
        subfilter.as_deref(),
        Some("adbe.pkcs7.detached") | Some("ETSI.CAdES.detached")
    ) {
        diagnostics.push(signature_diagnostic(
            "unsupported_subfilter",
            "signature SubFilter is not supported",
        ));
    }

    let byte_range = byte_range_verification(input, discovered.dictionary, &mut diagnostics);
    let contents = contents_verification(discovered.dictionary, &byte_range, &mut diagnostics);
    let cms_report = cms_verification(
        discovered.dictionary,
        &byte_range,
        input,
        trust_anchors,
        &mut diagnostics,
    );

    SignatureEntryReport {
        field_name: discovered.field_name,
        subfilter,
        byte_range,
        contents,
        cms_status: cms_report.cms_status,
        digest_status: cms_report.digest_status,
        signature_status: cms_report.signature_status,
        certificate_chain_status: cms_report.certificate_chain_status,
        revocation_status: signature_check(
            SignatureCheckState::Indeterminate,
            "offline revocation status is not confirmed; no network lookup is performed",
        ),
        timestamp_status: signature_check(
            SignatureCheckState::Indeterminate,
            "no RFC 3161 timestamp token was found in the signature",
        ),
        diagnostics,
    }
}
