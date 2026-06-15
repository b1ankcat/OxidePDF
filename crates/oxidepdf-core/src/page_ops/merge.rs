/// Merges multiple PDF artifacts into a single PDF.
pub fn merge_pdf_artifacts(inputs: &[Artifact]) -> Result<PdfArtifact, OxideError> {
    merge_pdf_artifacts_with_limits(inputs, &ResourceLimits::default())
}

/// Merges multiple PDF artifacts into a single PDF while enforcing resource limits.
pub fn merge_pdf_artifacts_with_limits(
    inputs: &[Artifact],
    limits: &ResourceLimits,
) -> Result<PdfArtifact, OxideError> {
    let merged = merge_artifacts_to_document(inputs, limits)?;
    let bytes = save_pdf(merged)?;
    enforce_output_bytes(bytes.len(), limits)?;
    Ok(PdfArtifact {
        bytes: crate::ArtifactBytes::from_vec(bytes)?,
    })
}

/// Merges two or more PDF input artifacts into one parsed document.
///
/// Each input is reused as a parsed document when it is already an object
/// artifact, or parsed once from bytes otherwise, so a merge can sit mid-chain
/// without a serialize/reparse roundtrip on either side.
pub(crate) fn merge_artifacts_to_document(
    inputs: &[Artifact],
    limits: &ResourceLimits,
) -> Result<lopdf::Document, OxideError> {
    if inputs.len() < 2 {
        return Err(OxideError::InvalidInput {
            reason: "merge requires at least two PDF inputs".to_owned(),
        });
    }

    let mut documents = Vec::with_capacity(inputs.len());
    let mut total_pages = 0usize;
    for input in inputs {
        let document = pdf_document_from_artifact(input, limits)?;
        total_pages = total_pages
            .checked_add(document.get_pages().len())
            .ok_or_else(|| resource_limit("max_pages"))?;
        enforce_max_pages(total_pages, limits)?;
        documents.push(document);
    }

    merge_documents(documents)
}

/// Resolves a PDF-bearing artifact to an owned parsed document: an object
/// artifact is cloned from its shared tree, byte artifacts are parsed once
/// (after an input-size check). Other artifact kinds are rejected.
pub(crate) fn pdf_document_from_artifact(
    artifact: &Artifact,
    limits: &ResourceLimits,
) -> Result<lopdf::Document, OxideError> {
    match artifact {
        Artifact::PdfObject(object) => Ok((*object.document).clone()),
        Artifact::Pdf(_) | Artifact::Bytes(_) => {
            let bytes = pdf_bytes(artifact)?;
            enforce_input_bytes(bytes.len(), limits)?;
            load_pdf(bytes)
        }
        _ => Err(OxideError::InvalidInput {
            reason: "expected PDF input artifact".to_owned(),
        }),
    }
}
