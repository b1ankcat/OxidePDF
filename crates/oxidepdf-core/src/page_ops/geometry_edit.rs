/// Crops selected PDF pages.
pub fn crop_pdf_pages(input: &[u8], options: &CropPagesOptions) -> Result<PdfArtifact, OxideError> {
    crop_pdf_pages_with_limits(input, options, &ResourceLimits::default())
}

/// Crops selected PDF pages while enforcing resource limits.
pub fn crop_pdf_pages_with_limits(
    input: &[u8],
    options: &CropPagesOptions,
    limits: &ResourceLimits,
) -> Result<PdfArtifact, OxideError> {
    enforce_input_bytes(input.len(), limits)?;
    let mut document = load_pdf(input)?;
    crop_pages_on_document(&mut document, options, limits)?;
    let bytes = save_pdf(document)?;
    enforce_output_bytes(bytes.len(), limits)?;
    Ok(PdfArtifact {
        bytes: crate::ArtifactBytes::from_vec(bytes)?,
    })
}

/// Crops selected pages of an already-parsed document.
pub(crate) fn crop_pages_on_document(
    document: &mut lopdf::Document,
    options: &CropPagesOptions,
    limits: &ResourceLimits,
) -> Result<(), OxideError> {
    let crop_box = validated_rect(options.left, options.bottom, options.right, options.top)?;
    let page_count = document.get_pages().len() as u32;
    enforce_max_pages(page_count as usize, limits)?;
    let selected_pages = selected_or_all_pages(options.pages.as_deref(), page_count)?;
    let pages = document.get_pages();
    for page_number in selected_pages {
        let page_id = pages
            .get(&page_number)
            .copied()
            .ok_or_else(|| OxideError::InvalidInput {
                reason: format!("page {page_number} is out of range"),
            })?;
        let page = document
            .get_object_mut(page_id)
            .and_then(Object::as_dict_mut)
            .map_err(|_| OxideError::ParsePdf)?;
        page.set("CropBox", crop_box_object(crop_box));
    }
    Ok(())
}

/// Scales selected PDF pages.
pub fn scale_pdf_pages(
    input: &[u8],
    options: &ScalePagesOptions,
) -> Result<PdfArtifact, OxideError> {
    scale_pdf_pages_with_limits(input, options, &ResourceLimits::default())
}

/// Scales selected PDF pages while enforcing resource limits.
pub fn scale_pdf_pages_with_limits(
    input: &[u8],
    options: &ScalePagesOptions,
    limits: &ResourceLimits,
) -> Result<PdfArtifact, OxideError> {
    enforce_input_bytes(input.len(), limits)?;
    let mut document = load_pdf(input)?;
    scale_pages_on_document(&mut document, options, limits)?;
    let bytes = save_pdf(document)?;
    enforce_output_bytes(bytes.len(), limits)?;
    Ok(PdfArtifact {
        bytes: crate::ArtifactBytes::from_vec(bytes)?,
    })
}

/// Scales selected pages of an already-parsed document.
pub(crate) fn scale_pages_on_document(
    document: &mut lopdf::Document,
    options: &ScalePagesOptions,
    limits: &ResourceLimits,
) -> Result<(), OxideError> {
    if !options.factor.is_finite() || options.factor <= 0.0 {
        return Err(OxideError::InvalidInput {
            reason: "scale factor must be greater than zero".to_owned(),
        });
    }
    let page_count = document.get_pages().len() as u32;
    enforce_max_pages(page_count as usize, limits)?;
    let selected_pages = selected_or_all_pages(options.pages.as_deref(), page_count)?;
    let pages = document.get_pages();
    for page_number in selected_pages {
        let page_id = pages
            .get(&page_number)
            .copied()
            .ok_or_else(|| OxideError::InvalidInput {
                reason: format!("page {page_number} is out of range"),
            })?;
        scale_page_boxes(document, page_id, options.factor)?;
        prepend_page_transform(document, page_id, options.factor)?;
    }
    Ok(())
}

/// Combines all pages into one tall page.
pub fn pdf_to_single_page(
    input: &[u8],
    options: &SinglePageOptions,
) -> Result<PdfArtifact, OxideError> {
    pdf_to_single_page_with_limits(input, options, &ResourceLimits::default())
}

/// Combines all pages into one tall page while enforcing resource limits.
pub fn pdf_to_single_page_with_limits(
    input: &[u8],
    options: &SinglePageOptions,
    limits: &ResourceLimits,
) -> Result<PdfArtifact, OxideError> {
    enforce_input_bytes(input.len(), limits)?;
    let mut document = load_pdf(input)?;
    single_page_on_document(&mut document, options, limits)?;
    let bytes = save_pdf(document)?;
    enforce_output_bytes(bytes.len(), limits)?;
    Ok(PdfArtifact {
        bytes: crate::ArtifactBytes::from_vec(bytes)?,
    })
}

/// Combines all pages of an already-parsed document into one tall page.
pub(crate) fn single_page_on_document(
    document: &mut lopdf::Document,
    _options: &SinglePageOptions,
    limits: &ResourceLimits,
) -> Result<(), OxideError> {
    let page_ids = document.get_pages().into_values().collect::<Vec<_>>();
    enforce_max_pages(page_ids.len(), limits)?;
    if page_ids.len() < 2 {
        return Err(OxideError::InvalidInput {
            reason: "single_page requires at least two pages".to_owned(),
        });
    }

    let mut max_width = 0.0f32;
    let mut total_height = 0.0f32;
    let mut page_sizes = Vec::with_capacity(page_ids.len());
    for page_id in &page_ids {
        let (width, height) = page_size(document, *page_id)?;
        if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
            return Err(OxideError::ParsePdf);
        }
        max_width = max_width.max(width);
        total_height += height;
        page_sizes.push((width, height));
    }
    if !max_width.is_finite() || !total_height.is_finite() {
        return Err(OxideError::ParsePdf);
    }

    let first_page = page_ids[0];
    let mut offset = total_height;
    let mut operations = Vec::new();
    let mut merged_resources = Dictionary::new();
    for (page_id, (_width, height)) in page_ids.iter().zip(page_sizes.iter()) {
        offset -= height;
        let content = document
            .get_page_content(*page_id)
            .map_err(|_| OxideError::ParsePdf)?;
        operations.push(lopdf::content::Operation::new("q", vec![]));
        operations.push(lopdf::content::Operation::new(
            "cm",
            vec![
                Object::Real(1.0),
                Object::Real(0.0),
                Object::Real(0.0),
                Object::Real(1.0),
                Object::Real(0.0),
                Object::Real(offset),
            ],
        ));
        operations.extend(
            lopdf::content::Content::decode(&content)
                .map_err(|_| OxideError::ParsePdf)?
                .operations,
        );
        operations.push(lopdf::content::Operation::new("Q", vec![]));
        merge_page_resources_into(document, *page_id, &mut merged_resources)?;
    }

    let merged_content = lopdf::content::Content { operations }
        .encode()
        .map_err(|_| OxideError::WritePdf)?;
    let content_id = document.add_object(Stream::new(Dictionary::new(), merged_content));
    {
        let page = document
            .get_object_mut(first_page)
            .and_then(Object::as_dict_mut)
            .map_err(|_| OxideError::ParsePdf)?;
        page.set("Contents", Object::Reference(content_id));
        page.set(
            "MediaBox",
            crop_box_object([0.0, 0.0, max_width, total_height]),
        );
        page.set(
            "CropBox",
            crop_box_object([0.0, 0.0, max_width, total_height]),
        );
        page.set("Resources", Object::Dictionary(merged_resources));
    }
    rebuild_pages_tree(document, &[first_page])
}
