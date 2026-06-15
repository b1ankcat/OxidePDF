/// Resolves a single PDF input to owned bytes, serializing an upstream parsed
/// object artifact when necessary.
///
/// Byte-consuming operators (inspect, render, security, compare, sign) read the
/// PDF as bytes. When a prior object-level operator hands them a parsed
/// document, it is serialized here so the chain keeps working; byte inputs are
/// returned without copying beyond the borrow.
fn single_pdf_input_bytes(inputs: &[Artifact]) -> Result<std::borrow::Cow<'_, [u8]>, OxideError> {
    if inputs.len() != 1 {
        return Err(OxideError::InvalidInput {
            reason: "operator requires exactly one PDF input".to_owned(),
        });
    }
    match &inputs[0] {
        Artifact::PdfObject(_) => Ok(std::borrow::Cow::Owned(
            inputs[0].output_bytes()?.into_owned(),
        )),
        _ => Ok(std::borrow::Cow::Borrowed(pdf_bytes(&inputs[0])?)),
    }
}

/// Materializes any parsed-object PDF inputs to serialized byte artifacts.
///
/// Multi-input or byte-producing operators (overlay, image edit, attachment,
/// imposition) consume their PDF input as bytes. When an upstream object-level
/// operator hands them a parsed document, it is serialized here — the one
/// defined serialization point for these operators — so they keep working in a
/// chain. Non-object artifacts are passed through by clone.
fn materialize_object_inputs(inputs: &[Artifact]) -> Result<Vec<Artifact>, OxideError> {
    inputs
        .iter()
        .map(|artifact| match artifact {
            Artifact::PdfObject(_) => Artifact::pdf(artifact.output_bytes()?),
            other => Ok(other.clone()),
        })
        .collect()
}

/// Resolves a single PDF input to an owned, parsed document for an object-level
/// operator.
///
/// An upstream object artifact is reused without re-parsing: its parsed document
/// is cloned from the shared `Arc` (the executor keeps the artifact in the store
/// during the layer, so the `Arc` is shared and cannot be moved out). A
/// byte-backed input is parsed once. Any other artifact kind is rejected. This
/// avoids the serialize-then-reparse roundtrip that a byte-only pipeline pays
/// between every chained PDF operator.
fn single_pdf_document(inputs: &[Artifact]) -> Result<lopdf::Document, OxideError> {
    if inputs.len() != 1 {
        return Err(OxideError::InvalidInput {
            reason: "operator requires exactly one PDF input".to_owned(),
        });
    }
    match &inputs[0] {
        Artifact::PdfObject(artifact) => Ok((*artifact.document).clone()),
        Artifact::Pdf(pdf) => load_pdf(pdf.bytes.as_slice()),
        Artifact::Bytes(bytes) => load_pdf(bytes.bytes.as_slice()),
        _ => Err(OxideError::InvalidInput {
            reason: "expected PDF input artifact".to_owned(),
        }),
    }
}

fn single_pdf_document_for_inspect(
    inputs: &[Artifact],
    limits: &ResourceLimits,
) -> Result<lopdf::Document, OxideError> {
    if inputs.len() != 1 {
        return Err(OxideError::InvalidInput {
            reason: "operator requires exactly one PDF input".to_owned(),
        });
    }
    match &inputs[0] {
        Artifact::PdfObject(artifact) => Ok((*artifact.document).clone()),
        Artifact::Pdf(pdf) => {
            enforce_input_bytes(pdf.bytes.len(), limits)?;
            load_pdf(pdf.bytes.as_slice())
        }
        Artifact::Bytes(bytes) => {
            enforce_input_bytes(bytes.bytes.len(), limits)?;
            load_pdf(bytes.bytes.as_slice())
        }
        _ => Err(OxideError::InvalidInput {
            reason: "expected PDF input artifact".to_owned(),
        }),
    }
}

fn two_pdf_inputs(inputs: &[Artifact]) -> Result<(&[u8], &[u8]), OxideError> {
    if inputs.len() != 2 {
        return Err(OxideError::InvalidInput {
            reason: "compare requires exactly two PDF inputs".to_owned(),
        });
    }

    Ok((pdf_bytes(&inputs[0])?, pdf_bytes(&inputs[1])?))
}

fn single_svg_input(inputs: &[Artifact]) -> Result<&[u8], OxideError> {
    if inputs.len() != 1 {
        return Err(OxideError::InvalidInput {
            reason: "svg2pdf requires exactly one SVG input".to_owned(),
        });
    }

    match &inputs[0] {
        Artifact::Svg(svg) => Ok(&svg.bytes),
        Artifact::Bytes(bytes) => Ok(&bytes.bytes),
        _ => Err(OxideError::InvalidInput {
            reason: "expected SVG input artifact".to_owned(),
        }),
    }
}
