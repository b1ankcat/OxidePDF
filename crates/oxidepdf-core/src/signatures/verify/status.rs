fn signature_check(status: SignatureCheckState, detail: impl Into<String>) -> SignatureCheckStatus {
    SignatureCheckStatus {
        status,
        detail: detail.into(),
    }
}

fn signature_diagnostic(
    code: impl Into<String>,
    message: impl Into<String>,
) -> SignatureDiagnostic {
    SignatureDiagnostic {
        code: code.into(),
        message: message.into(),
    }
}

fn overall_signature_verdict(
    signatures: &[SignatureEntryReport],
    diagnostics: &[SignatureDiagnostic],
) -> SignatureVerdict {
    if signatures.is_empty()
        || diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "no_signatures")
    {
        return SignatureVerdict::Indeterminate;
    }
    if signatures.iter().any(|signature| {
        signature
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "unsupported_subfilter")
    }) {
        return SignatureVerdict::Unsupported;
    }
    if signatures.iter().any(|signature| {
        signature
            .diagnostics
            .iter()
            .any(is_invalid_signature_diagnostic)
            || signature.cms_status.status == SignatureCheckState::Failed
            || signature.digest_status.status == SignatureCheckState::Failed
            || signature.signature_status.status == SignatureCheckState::Failed
            || signature.certificate_chain_status.status == SignatureCheckState::Failed
    }) {
        return SignatureVerdict::Invalid;
    }
    if signatures.iter().any(|signature| {
        signature.cms_status.status == SignatureCheckState::Unsupported
            || signature.signature_status.status == SignatureCheckState::Unsupported
            || signature.timestamp_status.status == SignatureCheckState::Unsupported
    }) {
        return SignatureVerdict::Unsupported;
    }
    // All signatures passed digest, signature, and chain-to-anchor checks.
    if signatures.iter().all(|signature| {
        signature.digest_status.status == SignatureCheckState::Passed
            && signature.signature_status.status == SignatureCheckState::Passed
            && signature.certificate_chain_status.status == SignatureCheckState::Passed
    }) {
        return SignatureVerdict::Trusted;
    }

    SignatureVerdict::Indeterminate
}

fn is_invalid_signature_diagnostic(diagnostic: &SignatureDiagnostic) -> bool {
    matches!(
        diagnostic.code.as_str(),
        "missing_byte_range"
            | "byte_range_out_of_bounds"
            | "byte_range_not_ordered"
            | "missing_contents"
            | "contents_not_covered_by_gap"
    )
}

