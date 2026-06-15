pub(crate) fn run_pdf_security(
    options: &PdfSecurityOptions,
    inputs: &[Artifact],
    limits: &ResourceLimits,
) -> Result<Artifact, OxideError> {
    let input = single_pdf_input_bytes(inputs)?;
    match options {
        PdfSecurityOptions::Encrypt(options) => {
            encrypt_pdf(&input, options, limits).map(Artifact::Pdf)
        }
        PdfSecurityOptions::Decrypt(options) => {
            decrypt_pdf(&input, options, limits).map(Artifact::Pdf)
        }
        PdfSecurityOptions::PermissionsGet(options) => {
            inspect_pdf_permissions(&input, options, limits).map(Artifact::Text)
        }
        PdfSecurityOptions::PermissionsSet(options) => {
            set_pdf_permissions(&input, options, limits).map(Artifact::Pdf)
        }
    }
}

pub(crate) fn run_pdf_compare(
    options: &PdfCompareOptions,
    inputs: &[Artifact],
    limits: &ResourceLimits,
) -> Result<Artifact, OxideError> {
    let inputs = materialize_object_inputs(inputs)?;
    let (left, right) = two_pdf_inputs(&inputs)?;
    match options {
        PdfCompareOptions::Report(options) => {
            compare_pdf_report(left, right, options, limits).map(Artifact::Text)
        }
        PdfCompareOptions::VisualDiff(options) => {
            compare_pdf_visual_diff(left, right, options, limits).map(Artifact::Image)
        }
    }
}

pub(crate) fn run_pdf_sign(
    options: &PdfSignOptions,
    inputs: &[Artifact],
    limits: &ResourceLimits,
) -> Result<Artifact, OxideError> {
    match options {
        PdfSignOptions::Add(options) => {
            let input = single_pdf_input_bytes(inputs)?;
            add_pdf_signature(&input, options, limits).and_then(|bytes| Artifact::pdf(&bytes))
        }
        PdfSignOptions::List(options) => {
            let input = single_pdf_input_bytes(inputs)?;
            verify_pdf_signatures(&input, options, limits).map(Artifact::Text)
        }
        PdfSignOptions::Verify(options) => {
            let input = single_pdf_input_bytes(inputs)?;
            verify_pdf_signatures(&input, options, limits).map(Artifact::Text)
        }
        PdfSignOptions::DeleteField(options) => {
            let input = single_pdf_input_bytes(inputs)?;
            delete_pdf_signature_field(&input, options, limits)
                .and_then(|bytes| Artifact::pdf(&bytes))
        }
        PdfSignOptions::Timestamp(options) => {
            let input = single_pdf_input_bytes(inputs)?;
            add_pdf_timestamp(&input, options, limits).map(Artifact::Text)
        }
    }
}
