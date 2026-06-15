pub fn edit_pdf_colors(
    input: &[u8],
    options: &ColorEditOptions,
    limits: &ResourceLimits,
) -> Result<PdfArtifact, OxideError> {
    enforce_input_bytes(input.len(), limits)?;
    let mut document = load_pdf(input)?;
    edit_colors_on_document(&mut document, options, limits)?;
    let bytes = save_pdf(document)?;
    enforce_output_bytes(bytes.len(), limits)?;
    Ok(PdfArtifact {
        bytes: crate::ArtifactBytes::from_vec(bytes)?,
    })
}

/// Rewrites RGB color operators of an already-parsed document.
pub(crate) fn edit_colors_on_document(
    document: &mut lopdf::Document,
    options: &ColorEditOptions,
    limits: &ResourceLimits,
) -> Result<(), OxideError> {
    if options.rasterize_pages {
        return Err(OxideError::UnsupportedPdfFeature {
            feature: "color rasterize_pages is not supported by the vector content path".to_owned(),
        });
    }
    let page_count = document.get_pages().len() as u32;
    enforce_max_pages(page_count as usize, limits)?;
    let pages = match options.pages.as_deref() {
        Some(pages) => parse_page_range(pages, page_count)?,
        None => (1..=page_count).collect(),
    };
    validate_color_options(options)?;
    let page_map = document.get_pages();
    for page in pages {
        let page_id = *page_map
            .get(&page)
            .ok_or_else(|| OxideError::InvalidInput {
                reason: format!("page {page} is out of range"),
            })?;
        rewrite_page_colors(document, page_id, options)?;
    }
    Ok(())
}

#[derive(Debug, Serialize)]
struct ImageReport {
    images: Vec<ImageResourceReport>,
}

#[derive(Debug, Serialize)]
struct ImageResourceReport {
    page: u32,
    name: String,
    object_id: String,
    width: i64,
    height: i64,
}
