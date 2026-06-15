fn list_pdf_signatures(
    input: &[u8],
    document: &lopdf::Document,
    limits: &ResourceLimits,
) -> Result<TextArtifact, OxideError> {
    let mut diagnostics = Vec::new();
    let signatures = discover_pdf_signature_dictionaries(document)?
        .into_iter()
        .map(|dictionary| signature_list_entry(input, dictionary))
        .collect::<Vec<_>>();

    if signatures.is_empty() {
        diagnostics.push(signature_diagnostic(
            "no_signatures",
            "PDF contains no signature dictionaries",
        ));
    }

    let report = SignatureListReport {
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

#[derive(Debug, Clone, Copy)]
struct SignaturePlaceholderWindow {
    byte_range_start: usize,
    byte_range_end: usize,
    hex_start: usize,
    hex_end: usize,
}

fn add_signature_placeholder(
    document: &mut lopdf::Document,
    field_name: &str,
    reserved_bytes: usize,
) -> Result<Vec<u8>, OxideError> {
    let pages = document.get_pages();
    let Some(page_id) = pages.values().next().copied() else {
        return Err(OxideError::InvalidInput {
            reason: "PDF contains no pages to host a signature field".to_owned(),
        });
    };
    let sig_value_id = document.new_object_id();
    let sig_field_id = document.new_object_id();
    let placeholder = vec![0u8; reserved_bytes];
    let sig_value = dictionary! {
        "Type" => "Sig",
        "Filter" => "Adobe.PPKLite",
        "SubFilter" => "adbe.pkcs7.detached",
        "ByteRange" => lopdf::Object::Array(vec![
            lopdf::Object::Integer(9999999999),
            lopdf::Object::Integer(9999999999),
            lopdf::Object::Integer(9999999999),
            lopdf::Object::Integer(9999999999),
        ]),
        "Contents" => lopdf::Object::String(placeholder, lopdf::StringFormat::Hexadecimal),
    };
    document
        .objects
        .insert(sig_value_id, lopdf::Object::Dictionary(sig_value));

    let sig_field = dictionary! {
        "Type" => "Annot",
        "Subtype" => "Widget",
        "FT" => "Sig",
        "T" => lopdf::Object::string_literal(field_name),
        "V" => sig_value_id,
        "Rect" => lopdf::Object::Array(vec![0.into(), 0.into(), 0.into(), 0.into()]),
        "P" => page_id,
    };
    document
        .objects
        .insert(sig_field_id, lopdf::Object::Dictionary(sig_field));
    append_reference_to_page_annots(document, page_id, sig_field_id)?;
    append_signature_field_to_acroform(document, sig_field_id)?;

    save_pdf(document.clone())
}

fn append_reference_to_page_annots(
    document: &mut lopdf::Document,
    page_id: lopdf::ObjectId,
    field_id: lopdf::ObjectId,
) -> Result<(), OxideError> {
    let page = document
        .get_object_mut(page_id)
        .and_then(lopdf::Object::as_dict_mut)
        .map_err(|_| OxideError::ParsePdf)?;
    if let Ok(annots) = page
        .get_mut(b"Annots")
        .and_then(lopdf::Object::as_array_mut)
    {
        annots.push(field_id.into());
    } else {
        page.set("Annots", lopdf::Object::Array(vec![field_id.into()]));
    }

    Ok(())
}

fn append_signature_field_to_acroform(
    document: &mut lopdf::Document,
    field_id: lopdf::ObjectId,
) -> Result<(), OxideError> {
    let acroform_id = {
        let catalog = document.catalog().map_err(|_| OxideError::ParsePdf)?;
        match catalog.get(b"AcroForm") {
            Ok(lopdf::Object::Reference(id)) => Some(*id),
            Ok(_) => None,
            Err(_) => None,
        }
    };
    let acroform_id = if let Some(acroform_id) = acroform_id {
        acroform_id
    } else {
        let acroform_id = document.new_object_id();
        document.objects.insert(
            acroform_id,
            lopdf::Object::Dictionary(dictionary! {
                "Fields" => lopdf::Object::Array(Vec::<lopdf::Object>::new()),
            }),
        );
        let catalog = document.catalog_mut().map_err(|_| OxideError::ParsePdf)?;
        catalog.set("AcroForm", acroform_id);
        acroform_id
    };
    let acroform = document
        .get_object_mut(acroform_id)
        .and_then(lopdf::Object::as_dict_mut)
        .map_err(|_| OxideError::ParsePdf)?;
    if let Ok(fields) = acroform
        .get_mut(b"Fields")
        .and_then(lopdf::Object::as_array_mut)
    {
        fields.push(field_id.into());
    } else {
        acroform.set("Fields", lopdf::Object::Array(vec![field_id.into()]));
    }

    Ok(())
}

fn find_signature_placeholder_window(
    pdf: &[u8],
    reserved_bytes: usize,
) -> Result<SignaturePlaceholderWindow, OxideError> {
    let byte_range_marker = b"/ByteRange";
    let byte_range_token_end =
        find_subslice(pdf, byte_range_marker).ok_or_else(|| OxideError::InvalidInput {
            reason: "signature ByteRange placeholder was not found".to_owned(),
        })? + byte_range_marker.len();
    let array_open = pdf[byte_range_token_end..]
        .iter()
        .position(|byte| *byte == b'[')
        .ok_or_else(|| OxideError::InvalidInput {
            reason: "signature ByteRange placeholder is malformed".to_owned(),
        })?;
    let byte_range_start = byte_range_token_end + array_open + 1;
    let byte_range_end = byte_range_start
        + find_subslice(&pdf[byte_range_start..], b"]").ok_or_else(|| {
            OxideError::InvalidInput {
                reason: "signature ByteRange placeholder is malformed".to_owned(),
            }
        })?;
    let zero_hex = vec![b'0'; reserved_bytes * 2];
    let hex_start = find_subslice(pdf, &zero_hex).ok_or_else(|| OxideError::InvalidInput {
        reason: "signature Contents placeholder was not found".to_owned(),
    })?;
    let hex_end = hex_start + zero_hex.len();

    Ok(SignaturePlaceholderWindow {
        byte_range_start,
        byte_range_end,
        hex_start,
        hex_end,
    })
}

fn signed_bytes_from_range(pdf: &[u8], byte_range: [usize; 4]) -> Option<Vec<u8>> {
    let first_end = byte_range[0].checked_add(byte_range[1])?;
    let second_end = byte_range[2].checked_add(byte_range[3])?;
    let mut bytes = Vec::with_capacity(byte_range[1].checked_add(byte_range[3])?);
    bytes.extend_from_slice(pdf.get(byte_range[0]..first_end)?);
    bytes.extend_from_slice(pdf.get(byte_range[2]..second_end)?);
    Some(bytes)
}

fn fill_byte_range_placeholder(
    mut pdf: Vec<u8>,
    window: SignaturePlaceholderWindow,
    byte_range: [usize; 4],
) -> Result<Vec<u8>, OxideError> {
    let byte_range_text = format!(
        "{:010} {:010} {:010} {:010}",
        byte_range[0], byte_range[1], byte_range[2], byte_range[3]
    );
    if byte_range_text.len() != window.byte_range_end - window.byte_range_start {
        return Err(OxideError::InvalidInput {
            reason: "signature ByteRange placeholder width changed during serialization".to_owned(),
        });
    }
    pdf[window.byte_range_start..window.byte_range_end].copy_from_slice(byte_range_text.as_bytes());

    Ok(pdf)
}

fn fill_signature_placeholder(
    mut pdf: Vec<u8>,
    window: SignaturePlaceholderWindow,
    cms: &[u8],
    reserved_bytes: usize,
) -> Result<Vec<u8>, OxideError> {
    let mut cms_hex = vec![b'0'; reserved_bytes * 2];
    write_hex_upper(cms, &mut cms_hex)?;
    pdf[window.hex_start..window.hex_end].copy_from_slice(&cms_hex);

    Ok(pdf)
}

fn write_hex_upper(input: &[u8], output: &mut [u8]) -> Result<(), OxideError> {
    if output.len() < input.len() * 2 {
        return Err(OxideError::Internal);
    }
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    for (index, byte) in input.iter().enumerate() {
        output[index * 2] = HEX[(byte >> 4) as usize];
        output[index * 2 + 1] = HEX[(byte & 0x0f) as usize];
    }
    Ok(())
}

fn build_p256_cms_signature(
    signed_bytes: &[u8],
    certificate: &Certificate,
    signing_key: &p256::ecdsa::SigningKey,
) -> Result<Vec<u8>, OxideError> {
    let digest = sha2::Sha256::digest(signed_bytes);
    let encap_content_info = cms::signed_data::EncapsulatedContentInfo {
        econtent_type: const_oid::db::rfc5911::ID_DATA,
        econtent: None,
    };
    let sid = SignerIdentifier::IssuerAndSerialNumber(cms::cert::IssuerAndSerialNumber {
        issuer: certificate.tbs_certificate.issuer.clone(),
        serial_number: certificate.tbs_certificate.serial_number.clone(),
    });
    let digest_algorithm = AlgorithmIdentifierOwned {
        oid: const_oid::db::rfc5912::ID_SHA_256,
        parameters: None,
    };
    let signer_info = SignerInfoBuilder::new(
        signing_key,
        sid,
        digest_algorithm.clone(),
        &encap_content_info,
        Some(&digest),
    )
    .map_err(|_| OxideError::InvalidInput {
        reason: "CMS signer info could not be prepared".to_owned(),
    })?;
    let mut signed_data = SignedDataBuilder::new(&encap_content_info);
    signed_data
        .add_digest_algorithm(digest_algorithm)
        .map_err(|_| OxideError::InvalidInput {
            reason: "CMS digest algorithm could not be added".to_owned(),
        })?;
    signed_data
        .add_certificate(CertificateChoices::Certificate(certificate.clone()))
        .map_err(|_| OxideError::InvalidInput {
            reason: "CMS certificate could not be added".to_owned(),
        })?;
    signed_data
        .add_signer_info::<p256::ecdsa::SigningKey, p256::ecdsa::DerSignature>(signer_info)
        .map_err(|_| OxideError::InvalidInput {
            reason: "CMS signer info could not be signed".to_owned(),
        })?;
    let content_info = signed_data.build().map_err(|_| OxideError::InvalidInput {
        reason: "CMS SignedData could not be encoded".to_owned(),
    })?;
    content_info.to_der().map_err(|_| OxideError::InvalidInput {
        reason: "CMS ContentInfo could not be encoded".to_owned(),
    })
}

fn load_signing_certificate(path: &std::path::Path) -> Result<Certificate, OxideError> {
    let pem = std::fs::read(path).map_err(|_| OxideError::Io)?;
    let pem = std::str::from_utf8(&pem).map_err(|_| OxideError::InvalidInput {
        reason: "signing certificate file contains no valid PEM certificate".to_owned(),
    })?;
    let certificates = parsed_trust_anchors(pem)?;
    certificates
        .into_iter()
        .next()
        .ok_or_else(|| OxideError::InvalidInput {
            reason: "signing certificate file contains no valid PEM certificate".to_owned(),
        })
}

fn load_p256_signing_key(path: &std::path::Path) -> Result<p256::ecdsa::SigningKey, OxideError> {
    let pem = zeroize::Zeroizing::new(std::fs::read_to_string(path).map_err(|_| OxideError::Io)?);
    p256::ecdsa::SigningKey::from_pkcs8_pem(&pem).map_err(|_| OxideError::InvalidInput {
        reason: "private key file must contain an unencrypted P-256 PKCS#8 PEM key".to_owned(),
    })
}

