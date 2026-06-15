pub fn verify_pdf_signatures(
    input: &[u8],
    options: &SignatureOptions,
    limits: &ResourceLimits,
) -> Result<TextArtifact, OxideError> {
    enforce_input_bytes(input.len(), limits)?;
    match options.mode {
        SignatureMode::List => {
            let document = load_pdf(input)?;
            enforce_max_pages(document.get_pages().len(), limits)?;
            list_pdf_signatures(input, &document, limits)
        }
        SignatureMode::Verify => {
            let trust_anchors = load_trust_anchors(options.trust_anchors.as_deref())?;
            let document = load_pdf(input)?;
            enforce_max_pages(document.get_pages().len(), limits)?;
            verify_pdf_signatures_report(input, &trust_anchors, &document, limits)
        }
    }
}

/// Adds a digital signature to a PDF.
pub fn add_pdf_signature(
    input: &[u8],
    options: &SignatureAddOptions,
    limits: &ResourceLimits,
) -> Result<Vec<u8>, OxideError> {
    enforce_input_bytes(input.len(), limits)?;
    ensure_pdf_magic(input)?;
    if options.field_name.trim().is_empty() {
        return Err(OxideError::InvalidInput {
            reason: "signature field name must not be empty".to_owned(),
        });
    }
    if options.contents_reserved_bytes.unwrap_or(16_384) == 0 {
        return Err(OxideError::InvalidInput {
            reason: "signature Contents reservation must be greater than zero".to_owned(),
        });
    }

    let mut document = load_pdf(input)?;
    enforce_max_pages(document.get_pages().len(), limits)?;
    if !discover_pdf_signature_dictionaries(&document)?.is_empty() {
        return Err(OxideError::UnsupportedPdfFeature {
            feature: "incremental signing of PDFs with existing signatures".to_owned(),
        });
    }

    let certificate = load_signing_certificate(&options.certificate)?;
    let signing_key = load_p256_signing_key(&options.private_key)?;
    let reserved_bytes = options.contents_reserved_bytes.unwrap_or(16_384);
    let placeholder_pdf =
        add_signature_placeholder(&mut document, &options.field_name, reserved_bytes)?;
    let signature_window = find_signature_placeholder_window(&placeholder_pdf, reserved_bytes)?;
    let byte_range = [
        0usize,
        signature_window.hex_start - 1,
        signature_window.hex_end + 1,
        placeholder_pdf.len() - (signature_window.hex_end + 1),
    ];
    let byte_ranged_pdf =
        fill_byte_range_placeholder(placeholder_pdf, signature_window, byte_range)?;
    let signed_bytes = signed_bytes_from_range(&byte_ranged_pdf, byte_range).ok_or_else(|| {
        OxideError::InvalidInput {
            reason: "signature ByteRange could not be assembled".to_owned(),
        }
    })?;
    let cms = build_p256_cms_signature(&signed_bytes, &certificate, &signing_key)?;
    if cms.len() > reserved_bytes {
        return Err(OxideError::InvalidInput {
            reason: "signature Contents reservation is too small for CMS output".to_owned(),
        });
    }
    let signed_pdf =
        fill_signature_placeholder(byte_ranged_pdf, signature_window, &cms, reserved_bytes)?;
    enforce_output_bytes(signed_pdf.len(), limits)?;

    Ok(signed_pdf)
}

/// Deletes a signature field without claiming any remaining signature is valid.
pub fn delete_pdf_signature_field(
    input: &[u8],
    options: &SignatureDeleteFieldOptions,
    limits: &ResourceLimits,
) -> Result<Vec<u8>, OxideError> {
    enforce_input_bytes(input.len(), limits)?;
    let mut document = load_pdf(input)?;
    enforce_max_pages(document.get_pages().len(), limits)?;
    if options.field_name.trim().is_empty() {
        return Err(OxideError::InvalidInput {
            reason: "signature field name must not be empty".to_owned(),
        });
    }

    let field_id = signature_field_id_by_name(&document, &options.field_name)?;
    let is_signed = document
        .get_object(field_id)
        .and_then(lopdf::Object::as_dict)
        .map(signature_field_has_value_material)
        .unwrap_or(false);
    if is_signed && !options.destructive {
        return Err(OxideError::InvalidInput {
            reason: "signature field contains signed value material; pass destructive delete explicitly to remove it".to_owned(),
        });
    }

    remove_signature_field_references(&mut document, field_id)?;
    document.objects.remove(&field_id);
    let bytes = save_pdf(document)?;
    enforce_output_bytes(bytes.len(), limits)?;
    Ok(bytes)
}

/// Adds or reports a document timestamp token.
pub fn add_pdf_timestamp(
    input: &[u8],
    options: &TimestampAddOptions,
    limits: &ResourceLimits,
) -> Result<TextArtifact, OxideError> {
    enforce_input_bytes(input.len(), limits)?;
    ensure_pdf_magic(input)?;
    let token_path = timestamp_token_path(options)?;
    let token = std::fs::read(token_path).map_err(|_| OxideError::Io)?;
    let status = if ContentInfo::from_der(&token).is_ok() || SignedData::from_der(&token).is_ok() {
        signature_check(
            SignatureCheckState::Indeterminate,
            "timestamp token DER parsed as CMS; RFC 3161 imprint validation is not yet implemented",
        )
    } else {
        signature_check(
            SignatureCheckState::Failed,
            "timestamp token is not valid CMS DER",
        )
    };
    let report = TimestampReport {
        status,
        input_preserved: true,
        diagnostics: vec![signature_diagnostic(
            "timestamp_not_embedded",
            "explicit timestamp token was inspected without modifying the PDF",
        )],
    };
    let text = serde_json::to_string_pretty(&report).map_err(|_| OxideError::Internal)?;
    enforce_output_bytes(text.len(), limits)?;

    Ok(TextArtifact {
        text,
        diagnostics: Vec::new(),
    })
}

fn timestamp_token_path(options: &TimestampAddOptions) -> Result<&std::path::Path, OxideError> {
    match (options.tsa_url.as_ref(), options.token.as_deref()) {
        (None, Some(token)) => Ok(token),
        (Some(_), None) => Err(OxideError::UnsupportedPdfFeature {
            feature: "live TSA timestamp requests".to_owned(),
        }),
        _ => Err(OxideError::InvalidInput {
            reason: "timestamp add requires exactly one of tsa_url or token".to_owned(),
        }),
    }
}
