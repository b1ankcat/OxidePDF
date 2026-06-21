
pub(super) fn enforce_artifact_output_bytes(
    artifact: &Artifact,
    limits: &ResourceLimits,
) -> Result<(), OxideError> {
    // A parsed object tree has no byte length until serialized. When a
    // max_output_bytes limit is in force, serialize to measure the true size so
    // the limit still bounds object-level outputs; when no limit is set, skip
    // the serialization entirely and keep the chain parse/serialize-free.
    if let Artifact::PdfObject(_) = artifact {
        if limits.max_output_bytes.is_some() {
            let size = artifact.output_bytes()?.len();
            return crate::enforce_output_bytes(size, limits);
        }
        return Ok(());
    }
    crate::enforce_output_bytes(artifact_size(artifact), limits)
}

pub(super) fn artifact_size(artifact: &Artifact) -> usize {
    match artifact {
        Artifact::Pdf(pdf) => pdf.bytes.len(),
        // A parsed object tree has no serialized byte length until it is
        // written. Intermediate object artifacts are not subject to output-byte
        // limits; the precise check runs at the output boundary after
        // serialization (see the CLI output path).
        Artifact::PdfObject(_) => 0,
        Artifact::Image(image) => image.bytes.len(),
        Artifact::Text(text) => text_artifact_size(text),
        Artifact::Svg(svg) => svg.bytes.len(),
        Artifact::Bytes(bytes) => bytes.bytes.len(),
    }
}

/// Estimates the in-memory footprint of a text artifact, including the
/// page-level diagnostics that `text.text.len()` alone omits. Undercounting
/// here would let a diagnostics-heavy artifact slip past `max_output_bytes`.
fn text_artifact_size(text: &TextArtifact) -> usize {
    let diagnostics_size = text
        .diagnostics
        .iter()
        .map(|diagnostic| {
            std::mem::size_of::<TextExtractionDiagnostic>() + diagnostic.message.len()
        })
        .sum::<usize>();
    text.text.len() + diagnostics_size
}

fn enforce_timeout(started_at: Instant, timeout: Option<Duration>) -> Result<(), OxideError> {
    if timeout.is_some_and(|timeout| started_at.elapsed() >= timeout) {
        return Err(resource_limit("timeout_ms"));
    }

    Ok(())
}

pub(super) fn invalid_workflow(reason: impl Into<String>) -> OxideError {
    OxideError::InvalidWorkflow {
        reason: reason.into(),
    }
}
