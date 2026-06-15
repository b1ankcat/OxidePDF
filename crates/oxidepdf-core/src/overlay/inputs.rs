fn overlay_inputs(
    inputs: &[Artifact],
    kind: OverlayKind,
) -> Result<(&[u8], Option<&[u8]>), OxideError> {
    match kind {
        OverlayKind::Watermark
        | OverlayKind::Text
        | OverlayKind::Stamp
        | OverlayKind::SignatureAppearance => {
            if inputs.len() != 1 {
                return Err(OxideError::InvalidInput {
                    reason: "text overlay requires exactly one PDF input".to_owned(),
                });
            }
            Ok((pdf_bytes(&inputs[0])?, None))
        }
        OverlayKind::Image | OverlayKind::Svg | OverlayKind::PdfPage => {
            if inputs.len() != 2 {
                return Err(OxideError::InvalidInput {
                    reason: "overlay requires PDF input and overlay input".to_owned(),
                });
            }
            let pdf = pdf_bytes(&inputs[0])?;
            let overlay = match kind {
                OverlayKind::Image => image_bytes(&inputs[1])?,
                OverlayKind::Svg => svg_bytes(&inputs[1])?,
                OverlayKind::PdfPage => pdf_bytes(&inputs[1])?,
                OverlayKind::Watermark
                | OverlayKind::Text
                | OverlayKind::Stamp
                | OverlayKind::SignatureAppearance => unreachable!(),
            };
            Ok((pdf, Some(overlay)))
        }
    }
}

fn image_bytes(artifact: &Artifact) -> Result<&[u8], OxideError> {
    match artifact {
        Artifact::Image(image) => Ok(&image.bytes),
        Artifact::Bytes(bytes) => Ok(&bytes.bytes),
        _ => Err(OxideError::InvalidInput {
            reason: "expected image input artifact".to_owned(),
        }),
    }
}

fn svg_bytes(artifact: &Artifact) -> Result<&[u8], OxideError> {
    match artifact {
        Artifact::Svg(svg) => Ok(&svg.bytes),
        Artifact::Bytes(bytes) => Ok(&bytes.bytes),
        _ => Err(OxideError::InvalidInput {
            reason: "expected SVG input artifact".to_owned(),
        }),
    }
}
