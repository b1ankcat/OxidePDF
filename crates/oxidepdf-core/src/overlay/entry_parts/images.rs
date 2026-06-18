pub fn inspect_pdf_images(
    input: &[u8],
    _options: &ImageInspectOptions,
) -> Result<TextArtifact, OxideError> {
    inspect_pdf_images_with_limits(input, _options, &default_inspect_limits())
}

pub fn inspect_pdf_images_with_limits(
    input: &[u8],
    _options: &ImageInspectOptions,
    limits: &ResourceLimits,
) -> Result<TextArtifact, OxideError> {
    let document = load_pdf_with_limits(input, limits)?;
    inspect_images_on_document(&document, limits)
}

pub(crate) fn inspect_images_on_document(
    document: &lopdf::Document,
    limits: &ResourceLimits,
) -> Result<TextArtifact, OxideError> {
    let mut images = Vec::new();
    for (page, page_id) in document.get_pages() {
        for (name, id, dict) in page_image_xobjects(document, page_id)? {
            let width = required_image_dimension(&dict, b"Width")?;
            let height = required_image_dimension(&dict, b"Height")?;
            images.push(ImageResourceReport {
                page,
                name: String::from_utf8_lossy(&name).into_owned(),
                object_id: format!("{} {}", id.0, id.1),
                width,
                height,
            });
        }
    }
    images.sort_by(|left, right| {
        left.page
            .cmp(&right.page)
            .then_with(|| left.name.cmp(&right.name))
    });
    let text =
        serde_json::to_string_pretty(&ImageReport { images }).map_err(|_| OxideError::Internal)?;
    enforce_output_bytes(text.len(), limits)?;
    Ok(TextArtifact {
        text,
        diagnostics: Vec::new(),
    })
}

/// Adds, replaces, or deletes image XObject resources.
pub fn edit_pdf_images_artifacts(
    inputs: &[Artifact],
    options: &ImageEditOptions,
    limits: &ResourceLimits,
) -> Result<PdfArtifact, OxideError> {
    let pdf = inputs.first().ok_or_else(|| OxideError::InvalidInput {
        reason: "image edit requires a PDF input".to_owned(),
    })?;
    let pdf = pdf_bytes(pdf)?;
    enforce_input_bytes(pdf.len(), limits)?;
    let mut document = load_pdf(pdf)?;
    enforce_max_pages(document.get_pages().len(), limits)?;

    match options.action {
        ImageEditAction::Add => {
            let [_, image] = inputs else {
                return Err(OxideError::InvalidInput {
                    reason: "image add requires PDF input and image input".to_owned(),
                });
            };
            let image = decode_limited_image(image_bytes(image)?, limits)?;
            let page = options.page.ok_or_else(|| OxideError::InvalidInput {
                reason: "image add requires page".to_owned(),
            })?;
            let name = required_image_name(options.name.as_deref(), "image add")?;
            add_image_to_page(&mut document, page, name, &image)?;
        }
        ImageEditAction::Replace => {
            let [_, image] = inputs else {
                return Err(OxideError::InvalidInput {
                    reason: "image replace requires PDF input and image input".to_owned(),
                });
            };
            let image = decode_limited_image(image_bytes(image)?, limits)?;
            let name = required_image_name(options.name.as_deref(), "image replace")?;
            replace_image_resource(&mut document, name, &image)?;
        }
        ImageEditAction::Delete => {
            let [_] = inputs else {
                return Err(OxideError::InvalidInput {
                    reason: "image delete requires exactly one PDF input".to_owned(),
                });
            };
            let name = required_image_name(options.name.as_deref(), "image delete")?;
            delete_image_resource(&mut document, name)?;
        }
    }

    let bytes = save_pdf(document)?;
    enforce_output_bytes(bytes.len(), limits)?;
    Ok(PdfArtifact {
        bytes: crate::ArtifactBytes::from_vec(bytes)?,
    })
}

/// Extracts the decoded RGB payload for a named image XObject.
pub fn extract_pdf_image(
    input: &[u8],
    options: &ImageExtractOptions,
    limits: &ResourceLimits,
) -> Result<BytesArtifact, OxideError> {
    enforce_input_bytes(input.len(), limits)?;
    let document = load_pdf(input)?;
    let name = options.name.as_bytes();
    for (_, page_id) in document.get_pages() {
        for (resource_name, stream_id, _) in page_image_xobjects(&document, page_id)? {
            if resource_name == name {
                let stream = document
                    .get_object(stream_id)
                    .and_then(Object::as_stream)
                    .map_err(|_| OxideError::ParsePdf)?;
                let bytes = stream
                    .get_plain_content()
                    .map_err(|_| OxideError::ParsePdf)?;
                enforce_output_bytes(bytes.len(), limits)?;
                return Ok(BytesArtifact {
                    bytes: crate::ArtifactBytes::from_vec(bytes)?,
                });
            }
        }
    }
    Err(OxideError::InvalidInput {
        reason: format!("image '{}' not found", options.name),
    })
}
