fn overlay_inputs(
    inputs: &[Artifact],
    kind: OverlayKind,
) -> Result<(&[u8], Option<&[u8]>), OxideError> {
    match kind {
        OverlayKind::Watermark
        | OverlayKind::Text
        | OverlayKind::Stamp
        | OverlayKind::SignatureAppearance => {
            let [pdf] = inputs else {
                return Err(OxideError::InvalidInput {
                    reason: "text overlay requires exactly one PDF input".to_owned(),
                });
            };
            Ok((pdf_bytes(pdf)?, None))
        }
        OverlayKind::Image | OverlayKind::Svg | OverlayKind::PdfPage => {
            let [pdf, overlay] = inputs else {
                return Err(OxideError::InvalidInput {
                    reason: "overlay requires PDF input and overlay input".to_owned(),
                });
            };
            let pdf = pdf_bytes(pdf)?;
            Ok((pdf, Some(overlay_bytes(overlay, kind)?)))
        }
    }
}

fn overlay_bytes(artifact: &Artifact, kind: OverlayKind) -> Result<&[u8], OxideError> {
    match kind {
        OverlayKind::Image => image_bytes(artifact),
        OverlayKind::Svg => svg_bytes(artifact),
        OverlayKind::PdfPage => pdf_bytes(artifact),
        OverlayKind::Watermark
        | OverlayKind::Text
        | OverlayKind::Stamp
        | OverlayKind::SignatureAppearance => Err(OxideError::InvalidInput {
            reason: "overlay requires PDF input and overlay input".to_owned(),
        }),
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
