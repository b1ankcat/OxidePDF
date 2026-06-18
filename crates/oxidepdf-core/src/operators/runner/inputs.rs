/// Resolves a single PDF input to owned bytes, serializing an upstream parsed
/// object artifact when necessary.
///
/// Byte-consuming operators (inspect, render, security, compare, sign) read the
/// PDF as bytes. When a prior object-level operator hands them a parsed
/// document, it is serialized here so the chain keeps working; byte inputs are
/// returned without copying beyond the borrow.
fn single_pdf_input_bytes<'a>(
    inputs: &'a [Artifact],
    limits: &ResourceLimits,
) -> Result<std::borrow::Cow<'a, [u8]>, OxideError> {
    let input = single_input(inputs, "operator requires exactly one PDF input")?;
    match input {
        Artifact::PdfObject(_) => {
            let bytes = input.output_bytes()?.into_owned();
            enforce_input_bytes(bytes.len(), limits)?;
            Ok(std::borrow::Cow::Owned(bytes))
        }
        _ => {
            let bytes = pdf_bytes(input)?;
            enforce_input_bytes(bytes.len(), limits)?;
            Ok(std::borrow::Cow::Borrowed(bytes))
        }
    }
}

/// Materializes any parsed-object PDF inputs to serialized byte artifacts.
///
/// Multi-input or byte-producing operators (overlay, image edit, attachment,
/// imposition) consume their PDF input as bytes. When an upstream object-level
/// operator hands them a parsed document, it is serialized here — the one
/// defined serialization point for these operators — so they keep working in a
/// chain. Non-object artifacts are passed through by clone.
fn materialize_object_inputs(
    inputs: &[Artifact],
    limits: &ResourceLimits,
) -> Result<Vec<Artifact>, OxideError> {
    inputs
        .iter()
        .map(|artifact| match artifact {
            Artifact::PdfObject(_) => {
                let bytes = artifact.output_bytes()?.into_owned();
                enforce_input_bytes(bytes.len(), limits)?;
                Artifact::pdf(bytes)
            }
            Artifact::Pdf(pdf) => {
                enforce_input_bytes(pdf.bytes.len(), limits)?;
                Ok(artifact.clone())
            }
            Artifact::Bytes(bytes) => {
                enforce_input_bytes(bytes.bytes.len(), limits)?;
                Ok(artifact.clone())
            }
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
fn single_pdf_document(
    inputs: &[Artifact],
    limits: &ResourceLimits,
) -> Result<lopdf::Document, OxideError> {
    match single_input(inputs, "operator requires exactly one PDF input")? {
        Artifact::PdfObject(artifact) => {
            let bytes = save_pdf((*artifact.document).clone())?;
            enforce_input_bytes(bytes.len(), limits)?;
            load_pdf(&bytes)
        }
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

fn single_pdf_document_for_inspect(
    inputs: &[Artifact],
    limits: &ResourceLimits,
) -> Result<lopdf::Document, OxideError> {
    match single_input(inputs, "operator requires exactly one PDF input")? {
        Artifact::PdfObject(artifact) => {
            let bytes = save_pdf((*artifact.document).clone())?;
            enforce_input_bytes(bytes.len(), limits)?;
            load_pdf(&bytes)
        }
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

fn two_pdf_inputs<'a>(
    inputs: &'a [Artifact],
    limits: &ResourceLimits,
) -> Result<(&'a [u8], &'a [u8]), OxideError> {
    let [left, right] = inputs else {
        return Err(OxideError::InvalidInput {
            reason: "compare requires exactly two PDF inputs".to_owned(),
        });
    };

    let left = pdf_bytes(left)?;
    let right = pdf_bytes(right)?;
    enforce_input_bytes(left.len(), limits)?;
    enforce_input_bytes(right.len(), limits)?;
    Ok((left, right))
}

fn single_svg_input(inputs: &[Artifact]) -> Result<&[u8], OxideError> {
    match single_input(inputs, "svg2pdf requires exactly one SVG input")? {
        Artifact::Svg(svg) => Ok(&svg.bytes),
        Artifact::Bytes(bytes) => Ok(&bytes.bytes),
        _ => Err(OxideError::InvalidInput {
            reason: "expected SVG input artifact".to_owned(),
        }),
    }
}

fn single_input<'a>(inputs: &'a [Artifact], reason: &str) -> Result<&'a Artifact, OxideError> {
    let [input] = inputs else {
        return Err(OxideError::InvalidInput {
            reason: reason.to_owned(),
        });
    };

    Ok(input)
}
