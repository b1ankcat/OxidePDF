pub fn watermark_pdf_artifacts(
    inputs: &[Artifact],
    options: &WatermarkOptions,
    limits: &ResourceLimits,
) -> Result<PdfArtifact, OxideError> {
    let kind = match options.kind {
        WatermarkKind::Text => OverlayKind::Watermark,
        WatermarkKind::Image => OverlayKind::Image,
        WatermarkKind::Svg => OverlayKind::Svg,
    };
    overlay_pdf_artifacts(
        inputs,
        &OverlayOptions {
            kind,
            text: options.text.clone(),
            font: options.font.clone(),
            font_path: options.font_path.clone(),
            font_size: options.font_size,
            opacity: options.opacity,
            rotation: options.rotation,
            position: options.position.clone(),
            pages: options.pages.clone(),
            scale: options.scale,
            rasterize: options.rasterize,
            source_page: None,
        },
        limits,
    )
}

/// Adds text, image, SVG, stamp, signature appearance, or PDF page overlays.
pub fn overlay_pdf_artifacts(
    inputs: &[Artifact],
    options: &OverlayOptions,
    limits: &ResourceLimits,
) -> Result<PdfArtifact, OxideError> {
    let (pdf_input, overlay_input) = overlay_inputs(inputs, options.kind)?;
    enforce_input_bytes(pdf_input.len(), limits)?;
    let mut document = load_pdf(pdf_input)?;
    let page_count = document.get_pages().len() as u32;
    enforce_max_pages(page_count as usize, limits)?;
    let pages = match options.pages.as_deref() {
        Some(pages) => parse_page_range(pages, page_count)?,
        None => (1..=page_count).collect(),
    };
    let settings = WatermarkSettings::from_options(options)?;

    match options.kind {
        OverlayKind::Watermark
        | OverlayKind::Text
        | OverlayKind::Stamp
        | OverlayKind::SignatureAppearance => {
            let text = options
                .text
                .as_deref()
                .filter(|text| !text.is_empty())
                .ok_or_else(|| OxideError::InvalidInput {
                    reason: "text overlay requires non-empty text".to_owned(),
                })?;
            let font = resolve_watermark_font(options)?;
            append_text_watermark(&mut document, &pages, text, &font, settings)?;
        }
        OverlayKind::Image => {
            let image = decode_limited_image(
                overlay_input.ok_or_else(|| OxideError::InvalidInput {
                    reason: "image overlay requires an image input".to_owned(),
                })?,
                limits,
            )?;
            append_image_watermark(&mut document, &pages, &image, settings)?;
        }
        OverlayKind::Svg => {
            let svg = overlay_input.ok_or_else(|| OxideError::InvalidInput {
                reason: "SVG overlay requires an SVG input".to_owned(),
            })?;
            enforce_input_bytes(svg.len(), limits)?;
            let tree = parse_svg(svg)?;
            let pixels = svg_pixel_count(&tree)?;
            enforce_max_pixels(pixels, limits)?;
            if options.rasterize {
                let image = rasterize_svg(&tree)?;
                append_image_watermark(&mut document, &pages, &image, settings)?;
            } else {
                append_svg_watermark(&mut document, &pages, &tree, settings)?;
            }
        }
        OverlayKind::PdfPage => {
            let source = overlay_input.ok_or_else(|| OxideError::InvalidInput {
                reason: "PDF page overlay requires a second PDF input".to_owned(),
            })?;
            enforce_input_bytes(source.len(), limits)?;
            append_pdf_page_overlay(&mut document, &pages, source, options.source_page, settings)?;
        }
    }

    let bytes = save_pdf(document)?;
    enforce_output_bytes(bytes.len(), limits)?;
    Ok(PdfArtifact {
        bytes: crate::ArtifactBytes::from_vec(bytes)?,
    })
}
