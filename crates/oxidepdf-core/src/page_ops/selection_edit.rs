/// Splits a PDF by keeping the specified one-based pages.
pub fn split_pdf(input: &[u8], pages: &str) -> Result<PdfArtifact, OxideError> {
    split_pdf_with_limits(input, pages, &ResourceLimits::default())
}

/// Splits a PDF by keeping the specified one-based pages while enforcing resource limits.
pub fn split_pdf_with_limits(
    input: &[u8],
    pages: &str,
    limits: &ResourceLimits,
) -> Result<PdfArtifact, OxideError> {
    enforce_input_bytes(input.len(), limits)?;
    let mut document = load_pdf(input)?;
    split_on_document(&mut document, pages, limits)?;
    let bytes = save_pdf(document)?;
    enforce_output_bytes(bytes.len(), limits)?;
    Ok(PdfArtifact {
        bytes: crate::ArtifactBytes::from_vec(bytes)?,
    })
}

/// Keeps the specified one-based pages of an already-parsed document. Shared by
/// the byte-level entry point and the object-level operator path.
pub(crate) fn split_on_document(
    document: &mut lopdf::Document,
    pages: &str,
    limits: &ResourceLimits,
) -> Result<(), OxideError> {
    enforce_max_pages(document.get_pages().len(), limits)?;
    let selected_pages = parse_page_range(pages, document.get_pages().len() as u32)?;
    keep_pages(document, &selected_pages)
}

/// Extracts selected PDF pages.
pub fn extract_pdf_pages(input: &[u8], pages: &str) -> Result<PdfArtifact, OxideError> {
    extract_pdf_pages_with_limits(input, pages, &ResourceLimits::default())
}

/// Extracts selected PDF pages while enforcing resource limits.
pub fn extract_pdf_pages_with_limits(
    input: &[u8],
    pages: &str,
    limits: &ResourceLimits,
) -> Result<PdfArtifact, OxideError> {
    split_pdf_with_limits(input, pages, limits)
}

/// Reorders a PDF by an explicit one-based page sequence.
pub fn reorder_pdf(input: &[u8], pages: &str) -> Result<PdfArtifact, OxideError> {
    reorder_pdf_with_limits(input, pages, &ResourceLimits::default())
}

/// Reorders a PDF by an explicit one-based page sequence while enforcing resource limits.
pub fn reorder_pdf_with_limits(
    input: &[u8],
    pages: &str,
    limits: &ResourceLimits,
) -> Result<PdfArtifact, OxideError> {
    enforce_input_bytes(input.len(), limits)?;
    let mut document = load_pdf(input)?;
    // Reorder and split share the same primitive: select pages in the given
    // order. `keep_pages` preserves input order, so this reorders too.
    split_on_document(&mut document, pages, limits)?;
    let bytes = save_pdf(document)?;
    enforce_output_bytes(bytes.len(), limits)?;
    Ok(PdfArtifact {
        bytes: crate::ArtifactBytes::from_vec(bytes)?,
    })
}

/// Rotates selected PDF pages by 90, 180, or 270 degrees.
pub fn rotate_pdf(input: &[u8], pages: &str, degrees: i16) -> Result<PdfArtifact, OxideError> {
    rotate_pdf_with_limits(input, pages, degrees, &ResourceLimits::default())
}

/// Rotates selected PDF pages by 90, 180, or 270 degrees while enforcing resource limits.
pub fn rotate_pdf_with_limits(
    input: &[u8],
    pages: &str,
    degrees: i16,
    limits: &ResourceLimits,
) -> Result<PdfArtifact, OxideError> {
    enforce_input_bytes(input.len(), limits)?;
    let mut document = load_pdf(input)?;
    rotate_on_document(&mut document, pages, degrees, limits)?;
    let bytes = save_pdf(document)?;
    enforce_output_bytes(bytes.len(), limits)?;
    Ok(PdfArtifact {
        bytes: crate::ArtifactBytes::from_vec(bytes)?,
    })
}

/// Rotates selected pages of an already-parsed document.
pub(crate) fn rotate_on_document(
    document: &mut lopdf::Document,
    pages: &str,
    degrees: i16,
    limits: &ResourceLimits,
) -> Result<(), OxideError> {
    enforce_max_pages(document.get_pages().len(), limits)?;
    let selected_pages = parse_page_range(pages, document.get_pages().len() as u32)?;
    let degrees = normalize_rotation(degrees)?;
    let pages = document.get_pages();

    for page_number in selected_pages {
        let page_id = pages
            .get(&page_number)
            .ok_or_else(|| OxideError::InvalidInput {
                reason: format!("page {page_number} is out of range"),
            })?;
        let page_dict = document
            .get_object_mut(*page_id)
            .and_then(lopdf::Object::as_dict_mut)
            .map_err(|_| OxideError::ParsePdf)?;
        let current_rotation = page_dict
            .get(b"Rotate")
            .and_then(lopdf::Object::as_i64)
            .unwrap_or(0);
        page_dict.set(
            "Rotate",
            (current_rotation + i64::from(degrees)).rem_euclid(360),
        );
    }

    Ok(())
}

/// Deletes selected PDF pages.
pub fn delete_pdf_pages(input: &[u8], pages: &str) -> Result<PdfArtifact, OxideError> {
    delete_pdf_pages_with_limits(input, pages, &ResourceLimits::default())
}

/// Deletes selected PDF pages while enforcing resource limits.
pub fn delete_pdf_pages_with_limits(
    input: &[u8],
    pages: &str,
    limits: &ResourceLimits,
) -> Result<PdfArtifact, OxideError> {
    enforce_input_bytes(input.len(), limits)?;
    let mut document = load_pdf(input)?;
    delete_pages_on_document(&mut document, pages, limits)?;
    let bytes = save_pdf(document)?;
    enforce_output_bytes(bytes.len(), limits)?;
    Ok(PdfArtifact {
        bytes: crate::ArtifactBytes::from_vec(bytes)?,
    })
}

/// Deletes selected pages of an already-parsed document.
pub(crate) fn delete_pages_on_document(
    document: &mut lopdf::Document,
    pages: &str,
    limits: &ResourceLimits,
) -> Result<(), OxideError> {
    let page_count = document.get_pages().len() as u32;
    enforce_max_pages(page_count as usize, limits)?;
    let deleted_pages = parse_page_range(pages, page_count)?;
    if deleted_pages.len() as u32 == page_count {
        return Err(OxideError::InvalidInput {
            reason: "delete_pages must leave at least one page".to_owned(),
        });
    }
    let kept_pages = (1..=page_count)
        .filter(|page| !deleted_pages.contains(page))
        .collect::<Vec<_>>();
    keep_pages(document, &kept_pages)
}

/// Deletes structurally blank pages.
pub fn delete_blank_pdf_pages(
    input: &[u8],
    options: &DeleteBlankPagesOptions,
) -> Result<PdfArtifact, OxideError> {
    delete_blank_pdf_pages_with_limits(input, options, &ResourceLimits::default())
}

/// Deletes structurally blank pages while enforcing resource limits.
pub fn delete_blank_pdf_pages_with_limits(
    input: &[u8],
    options: &DeleteBlankPagesOptions,
    limits: &ResourceLimits,
) -> Result<PdfArtifact, OxideError> {
    enforce_input_bytes(input.len(), limits)?;
    let mut document = load_pdf(input)?;
    delete_blank_pages_on_document(&mut document, options, limits)?;
    let bytes = save_pdf(document)?;
    enforce_output_bytes(bytes.len(), limits)?;
    Ok(PdfArtifact {
        bytes: crate::ArtifactBytes::from_vec(bytes)?,
    })
}

/// Deletes structurally blank pages of an already-parsed document.
pub(crate) fn delete_blank_pages_on_document(
    document: &mut lopdf::Document,
    _options: &DeleteBlankPagesOptions,
    limits: &ResourceLimits,
) -> Result<(), OxideError> {
    let page_count = document.get_pages().len() as u32;
    enforce_max_pages(page_count as usize, limits)?;
    let page_map = document.get_pages();
    let mut blank_pages = Vec::new();
    for (page_number, page_id) in page_map {
        if page_is_structurally_blank(document, page_id)? {
            blank_pages.push(page_number);
        }
    }
    if blank_pages.is_empty() {
        return Err(OxideError::InvalidInput {
            reason: "PDF contains no structurally blank pages".to_owned(),
        });
    }
    if blank_pages.len() as u32 == page_count {
        return Err(OxideError::InvalidInput {
            reason: "delete_blank_pages must leave at least one page".to_owned(),
        });
    }
    let kept_pages = (1..=page_count)
        .filter(|page| !blank_pages.contains(page))
        .collect::<Vec<_>>();
    keep_pages(document, &kept_pages)
}
