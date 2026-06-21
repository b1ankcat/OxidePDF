pub fn image_artifacts_to_pdf(
    inputs: &[Artifact],
    options: &ImageToPdfOptions,
    limits: &ResourceLimits,
) -> Result<PdfArtifact, OxideError> {
    if inputs.is_empty() {
        return Err(OxideError::InvalidInput {
            reason: "img2pdf requires at least one image input".to_owned(),
        });
    }
    enforce_max_pages(inputs.len(), limits)?;

    let mut images = Vec::with_capacity(inputs.len());
    let mut total_pixels = 0u64;
    for input in inputs {
        let bytes = image_bytes(input)?;
        let decoded = decode_limited_image(bytes, limits)?;
        let pixels = u64::from(decoded.width) * u64::from(decoded.height);
        total_pixels = total_pixels
            .checked_add(pixels)
            .ok_or_else(|| resource_limit("max_pixels"))?;
        enforce_max_pixels(total_pixels, limits)?;
        images.push(decoded);
    }

    let layout = ImageLayout::from_options(options)?;
    let bytes = write_images_pdf(&images, layout)?;
    enforce_output_bytes(bytes.len(), limits)?;
    Ok(PdfArtifact {
        bytes: crate::ArtifactBytes::from_vec(bytes)?,
    })
}

/// Converts an SVG artifact into a PDF. Defaults to vector output.
pub fn svg_to_pdf(
    input: &[u8],
    options: &SvgToPdfOptions,
    limits: &ResourceLimits,
) -> Result<PdfArtifact, OxideError> {
    enforce_input_bytes(input.len(), limits)?;
    let tree = parse_svg(input)?;
    let pixels = svg_pixel_count(&tree)?;
    enforce_max_pixels(pixels, limits)?;
    enforce_max_pages(1, limits)?;

    let bytes = if options.rasterize {
        let image = rasterize_svg(&tree)?;
        write_images_pdf(&[image], ImageLayout::OriginalSize)?
    } else {
        let conversion_options = svg2pdf::ConversionOptions {
            embed_text: false,
            ..svg2pdf::ConversionOptions::default()
        };
        svg2pdf::to_pdf(&tree, conversion_options, svg2pdf::PageOptions::default())
            .map_err(|_| OxideError::WritePdf)?
    };

    enforce_output_bytes(bytes.len(), limits)?;
    Ok(PdfArtifact {
        bytes: crate::ArtifactBytes::from_vec(bytes)?,
    })
}

/// Renders a one-based PDF page to PNG bytes.
pub fn render_pdf_page(
    input: &[u8],
    options: &RenderOptions,
    limits: &ResourceLimits,
) -> Result<ImageArtifact, OxideError> {
    enforce_input_bytes(input.len(), limits)?;
    ensure_pdf_magic(input)?;
    let format = options.format.as_deref().unwrap_or("png");
    if format != "png" {
        return Err(OxideError::InvalidInput {
            reason: format!("unsupported render format '{format}'"),
        });
    }
    if options.page == 0 {
        return Err(OxideError::InvalidInput {
            reason: "page number must be one or greater".to_owned(),
        });
    }
    let scale = options.scale.unwrap_or(1.0);
    if !scale.is_finite() || scale <= 0.0 {
        return Err(OxideError::InvalidInput {
            reason: "render scale must be greater than zero".to_owned(),
        });
    }

    let pdf = hayro::hayro_syntax::Pdf::new(input.to_vec()).map_err(|_| OxideError::RenderPdf)?;
    let page_count = pdf.pages().len();
    enforce_max_pages(page_count, limits)?;
    let page_index = usize::try_from(options.page - 1).map_err(|_| OxideError::InvalidInput {
        reason: format!("page {} is out of range 1-{page_count}", options.page),
    })?;
    let page = pdf
        .pages()
        .get(page_index)
        .ok_or_else(|| OxideError::InvalidInput {
            reason: format!("page {} is out of range 1-{page_count}", options.page),
        })?;

    // Bound the scaled output resolution before rendering so a large scale (or a
    // huge MediaBox) cannot force the renderer to allocate a giant pixmap. The
    // pixel budget runs against the requested scale, not just the output PNG.
    let (page_width, page_height) = page.render_dimensions();
    let scaled_width = f64::from(page_width) * f64::from(scale);
    let scaled_height = f64::from(page_height) * f64::from(scale);
    if !scaled_width.is_finite() || !scaled_height.is_finite() {
        return Err(OxideError::InvalidInput {
            reason: "render scale produces non-finite output dimensions".to_owned(),
        });
    }
    let scaled_pixels = (scaled_width.ceil() as u64).saturating_mul(scaled_height.ceil() as u64);
    enforce_max_pixels(scaled_pixels, limits)?;

    let cache = hayro::RenderCache::new();
    let interpreter_settings = hayro::hayro_interpret::InterpreterSettings::default();
    let render_settings = hayro::RenderSettings {
        x_scale: scale,
        y_scale: scale,
        bg_color: hayro::vello_cpu::color::palette::css::WHITE,
        ..Default::default()
    };
    let pixmap = hayro::render(page, &cache, &interpreter_settings, &render_settings);
    let bytes = pixmap.into_png().map_err(|_| OxideError::RenderPdf)?;
    if bytes.is_empty() {
        return Err(OxideError::RenderPdf);
    }
    enforce_output_bytes(bytes.len(), limits)?;

    Ok(ImageArtifact {
        bytes: crate::ArtifactBytes::from_vec(bytes)?,
    })
}

/// Extracts plain text from a PDF and records page-level diagnostics.
pub fn extract_text_from_pdf(
    input: &[u8],
    options: &ExtractTextOptions,
    limits: &ResourceLimits,
) -> Result<TextArtifact, OxideError> {
    enforce_input_bytes(input.len(), limits)?;
    ensure_pdf_magic(input)?;
    let format = options.format.as_deref().unwrap_or("plain");
    if format != "plain" {
        return Err(OxideError::InvalidInput {
            reason: format!("unsupported text extraction format '{format}'"),
        });
    }

    let pages =
        pdf_extract::extract_text_from_mem_by_pages(input).map_err(map_pdf_extract_error)?;
    if pages.is_empty() {
        return Err(OxideError::InvalidInput {
            reason: "PDF contains no pages".to_owned(),
        });
    }
    enforce_max_pages(pages.len(), limits)?;

    let diagnostics = pages
        .iter()
        .enumerate()
        .filter_map(|(index, page)| match page.trim().is_empty() {
            true => Some(TextExtractionDiagnostic {
                page: (index + 1) as u32,
                code: TextExtractionDiagnosticCode::NoTextLayer,
                message: "page has no extractable text layer".to_owned(),
            }),
            false => None,
        })
        .collect::<Vec<_>>();
    if diagnostics.len() == pages.len() {
        return Err(OxideError::InvalidInput {
            reason: "PDF has no extractable text layer".to_owned(),
        });
    }

    let artifact = TextArtifact {
        text: pages.concat(),
        diagnostics,
    };
    enforce_output_bytes(artifact.text.len(), limits)?;
    Ok(artifact)
}
