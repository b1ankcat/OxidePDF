use crate::{
    add_resource_dict_entry, enforce_input_bytes, enforce_max_pages, enforce_output_bytes,
    load_pdf, merge_resource_dictionary, object_to_f32, page_size, pdf_bytes, rebuild_pages_tree,
    remap_imported_references, resource_limit, save_pdf, Artifact, OxideError, PdfArtifact,
    ResourceLimits,
};
use lopdf::{dictionary, Dictionary, Object, Stream};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Options for merge.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MergeOptions {}

/// Options for split.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SplitOptions {
    /// Page range, for example `1,3-5`.
    pub pages: String,
}

/// Options for reorder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReorderOptions {
    /// Explicit page sequence, for example `3,1,2`.
    pub pages: String,
}

/// Options for rotate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RotateOptions {
    /// Page range, for example `1,3-5`.
    pub pages: String,
    /// Rotation in degrees. Validation happens in the workflow validator.
    pub degrees: i16,
}

/// Options for page-selection edits.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageSelectionOptions {
    /// Page range, for example `1,3-5`.
    pub pages: String,
}

/// Options for deleting structurally blank pages.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DeleteBlankPagesOptions {}

/// Options for cropping pages.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CropPagesOptions {
    /// Page range, for example `1,3-5`.
    pub pages: Option<String>,
    /// Left coordinate of the new CropBox.
    pub left: f32,
    /// Bottom coordinate of the new CropBox.
    pub bottom: f32,
    /// Right coordinate of the new CropBox.
    pub right: f32,
    /// Top coordinate of the new CropBox.
    pub top: f32,
}

/// Options for scaling pages.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScalePagesOptions {
    /// Page range, for example `1,3-5`.
    pub pages: Option<String>,
    /// Scale factor applied to page boxes and page contents.
    pub factor: f32,
}

/// Options for combining pages into one tall page.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SinglePageOptions {}

/// Options for N-up page layout.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NUpOptions {
    /// Number of columns on each output page.
    pub columns: u32,
    /// Number of rows on each output page.
    pub rows: u32,
}

/// Options for booklet imposition.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct BookletOptions {}

/// Options for adding page numbers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PageNumbersOptions {
    /// Page range, for example `1,3-5`. Defaults to all pages.
    pub pages: Option<String>,
    /// First number written on the first selected page.
    pub start: u32,
    /// Text before the number.
    pub prefix: String,
    /// Text after the number.
    pub suffix: String,
    /// Font size in PDF points.
    pub font_size: f32,
    /// Page number placement.
    pub position: PageNumberPosition,
}

impl Default for PageNumbersOptions {
    fn default() -> Self {
        Self {
            pages: None,
            start: 1,
            prefix: String::new(),
            suffix: String::new(),
            font_size: 12.0,
            position: PageNumberPosition::BottomCenter,
        }
    }
}

/// Page number placement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PageNumberPosition {
    /// Top-left corner.
    TopLeft,
    /// Top-center edge.
    TopCenter,
    /// Top-right corner.
    TopRight,
    /// Bottom-left corner.
    BottomLeft,
    /// Bottom-center edge.
    BottomCenter,
    /// Bottom-right corner.
    BottomRight,
}

/// Merges multiple PDF artifacts into a single PDF.
pub fn merge_pdf_artifacts(inputs: &[Artifact]) -> Result<PdfArtifact, OxideError> {
    merge_pdf_artifacts_with_limits(inputs, &ResourceLimits::default())
}

/// Merges multiple PDF artifacts into a single PDF while enforcing resource limits.
pub fn merge_pdf_artifacts_with_limits(
    inputs: &[Artifact],
    limits: &ResourceLimits,
) -> Result<PdfArtifact, OxideError> {
    if inputs.len() < 2 {
        return Err(OxideError::InvalidInput {
            reason: "merge requires at least two PDF inputs".to_owned(),
        });
    }

    let mut documents = Vec::with_capacity(inputs.len());
    let mut total_pages = 0usize;
    for input in inputs {
        let bytes = pdf_bytes(input)?;
        enforce_input_bytes(bytes.len(), limits)?;
        let document = load_pdf(bytes)?;
        total_pages = total_pages
            .checked_add(document.get_pages().len())
            .ok_or_else(|| resource_limit("max_pages"))?;
        enforce_max_pages(total_pages, limits)?;
        documents.push(document);
    }

    let bytes = merge_documents(documents)?;
    enforce_output_bytes(bytes.len(), limits)?;
    Ok(PdfArtifact {
        bytes: bytes.into(),
    })
}

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
    enforce_max_pages(document.get_pages().len(), limits)?;
    let selected_pages = parse_page_range(pages, document.get_pages().len() as u32)?;
    keep_pages(&mut document, &selected_pages)?;
    let bytes = save_pdf(document)?;
    enforce_output_bytes(bytes.len(), limits)?;
    Ok(PdfArtifact {
        bytes: bytes.into(),
    })
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
    enforce_max_pages(document.get_pages().len(), limits)?;
    let selected_pages = parse_page_range(pages, document.get_pages().len() as u32)?;
    keep_pages(&mut document, &selected_pages)?;
    let bytes = save_pdf(document)?;
    enforce_output_bytes(bytes.len(), limits)?;
    Ok(PdfArtifact {
        bytes: bytes.into(),
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

    let bytes = save_pdf(document)?;
    enforce_output_bytes(bytes.len(), limits)?;
    Ok(PdfArtifact {
        bytes: bytes.into(),
    })
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
    keep_pages(&mut document, &kept_pages)?;
    let bytes = save_pdf(document)?;
    enforce_output_bytes(bytes.len(), limits)?;
    Ok(PdfArtifact {
        bytes: bytes.into(),
    })
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
    _options: &DeleteBlankPagesOptions,
    limits: &ResourceLimits,
) -> Result<PdfArtifact, OxideError> {
    enforce_input_bytes(input.len(), limits)?;
    let mut document = load_pdf(input)?;
    let page_count = document.get_pages().len() as u32;
    enforce_max_pages(page_count as usize, limits)?;
    let page_map = document.get_pages();
    let mut blank_pages = Vec::new();
    for (page_number, page_id) in page_map {
        if page_is_structurally_blank(&document, page_id)? {
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
    keep_pages(&mut document, &kept_pages)?;
    let bytes = save_pdf(document)?;
    enforce_output_bytes(bytes.len(), limits)?;
    Ok(PdfArtifact {
        bytes: bytes.into(),
    })
}

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
    let crop_box = validated_rect(options.left, options.bottom, options.right, options.top)?;
    enforce_input_bytes(input.len(), limits)?;
    let mut document = load_pdf(input)?;
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
    let bytes = save_pdf(document)?;
    enforce_output_bytes(bytes.len(), limits)?;
    Ok(PdfArtifact {
        bytes: bytes.into(),
    })
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
    if !options.factor.is_finite() || options.factor <= 0.0 {
        return Err(OxideError::InvalidInput {
            reason: "scale factor must be greater than zero".to_owned(),
        });
    }
    enforce_input_bytes(input.len(), limits)?;
    let mut document = load_pdf(input)?;
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
        scale_page_boxes(&mut document, page_id, options.factor)?;
        prepend_page_transform(&mut document, page_id, options.factor)?;
    }
    let bytes = save_pdf(document)?;
    enforce_output_bytes(bytes.len(), limits)?;
    Ok(PdfArtifact {
        bytes: bytes.into(),
    })
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
    _options: &SinglePageOptions,
    limits: &ResourceLimits,
) -> Result<PdfArtifact, OxideError> {
    enforce_input_bytes(input.len(), limits)?;
    let mut document = load_pdf(input)?;
    let page_ids = document.get_pages().into_values().collect::<Vec<_>>();
    enforce_max_pages(page_ids.len(), limits)?;
    if page_ids.len() == 1 {
        return Err(OxideError::InvalidInput {
            reason: "single_page requires at least two pages".to_owned(),
        });
    }

    let mut max_width = 0.0f32;
    let mut total_height = 0.0f32;
    let mut page_sizes = Vec::with_capacity(page_ids.len());
    for page_id in &page_ids {
        let (width, height) = page_size(&document, *page_id)?;
        if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
            return Err(OxideError::ParsePdf);
        }
        max_width = max_width.max(width);
        total_height += height;
        page_sizes.push((width, height));
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
        merge_page_resources_into(&document, *page_id, &mut merged_resources)?;
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
    rebuild_pages_tree(&mut document, &[first_page])?;
    let bytes = save_pdf(document)?;
    enforce_output_bytes(bytes.len(), limits)?;
    Ok(PdfArtifact {
        bytes: bytes.into(),
    })
}

/// Lays multiple source pages on each output page.
pub fn nup_pdf_pages(input: &[u8], options: &NUpOptions) -> Result<PdfArtifact, OxideError> {
    nup_pdf_pages_with_limits(input, options, &ResourceLimits::default())
}

/// Lays multiple source pages on each output page while enforcing resource limits.
pub fn nup_pdf_pages_with_limits(
    input: &[u8],
    options: &NUpOptions,
    limits: &ResourceLimits,
) -> Result<PdfArtifact, OxideError> {
    validate_nup_options(options)?;
    enforce_input_bytes(input.len(), limits)?;
    let source = load_pdf(input)?;
    let page_ids = source.get_pages().into_values().collect::<Vec<_>>();
    enforce_max_pages(page_ids.len(), limits)?;
    let layout = page_layout_from_first_page(&source, &page_ids)?;
    let slots_per_page = (options.columns * options.rows) as usize;
    let output_count = page_ids.len().div_ceil(slots_per_page);
    enforce_max_pages(output_count, limits)?;
    let order = (0..page_ids.len()).collect::<Vec<_>>();

    let bytes = impose_pages(
        &source,
        &page_ids,
        &order,
        layout,
        options.columns,
        options.rows,
    )?;
    enforce_output_bytes(bytes.len(), limits)?;
    Ok(PdfArtifact {
        bytes: bytes.into(),
    })
}

/// Arranges pages for booklet printing.
pub fn booklet_pdf_pages(
    input: &[u8],
    options: &BookletOptions,
) -> Result<PdfArtifact, OxideError> {
    booklet_pdf_pages_with_limits(input, options, &ResourceLimits::default())
}

/// Arranges pages for booklet printing while enforcing resource limits.
pub fn booklet_pdf_pages_with_limits(
    input: &[u8],
    _options: &BookletOptions,
    limits: &ResourceLimits,
) -> Result<PdfArtifact, OxideError> {
    enforce_input_bytes(input.len(), limits)?;
    let source = load_pdf(input)?;
    let page_ids = source.get_pages().into_values().collect::<Vec<_>>();
    enforce_max_pages(page_ids.len(), limits)?;
    if page_ids.len() < 2 {
        return Err(OxideError::InvalidInput {
            reason: "booklet requires at least two pages".to_owned(),
        });
    }
    let layout = page_layout_from_first_page(&source, &page_ids)?;
    let sheet_count = page_ids.len().div_ceil(4);
    enforce_max_pages(sheet_count * 2, limits)?;
    let order = booklet_page_order(page_ids.len());

    let bytes = impose_pages(&source, &page_ids, &order, layout, 2, 1)?;
    enforce_output_bytes(bytes.len(), limits)?;
    Ok(PdfArtifact {
        bytes: bytes.into(),
    })
}

/// Adds page numbers to selected PDF pages.
pub fn add_pdf_page_numbers(
    input: &[u8],
    options: &PageNumbersOptions,
) -> Result<PdfArtifact, OxideError> {
    add_pdf_page_numbers_with_limits(input, options, &ResourceLimits::default())
}

/// Adds page numbers to selected PDF pages while enforcing resource limits.
pub fn add_pdf_page_numbers_with_limits(
    input: &[u8],
    options: &PageNumbersOptions,
    limits: &ResourceLimits,
) -> Result<PdfArtifact, OxideError> {
    validate_page_number_options(options)?;
    enforce_input_bytes(input.len(), limits)?;
    let mut document = load_pdf(input)?;
    let page_count = document.get_pages().len() as u32;
    enforce_max_pages(page_count as usize, limits)?;
    let selected_pages = selected_or_all_pages(options.pages.as_deref(), page_count)?;
    add_standard_font_resource(&mut document, &selected_pages, b"OxPnF1".to_vec())?;
    let page_map = document.get_pages();
    for (index, page_number) in selected_pages.iter().enumerate() {
        let page_id = *page_map
            .get(page_number)
            .ok_or_else(|| OxideError::InvalidInput {
                reason: format!("page {page_number} is out of range"),
            })?;
        let (page_width, page_height) = page_size(&document, page_id)?;
        let label = format!(
            "{}{}{}",
            options.prefix,
            options.start + index as u32,
            options.suffix
        );
        let content = page_number_content(
            &label,
            page_width,
            page_height,
            options.font_size,
            options.position,
        )?;
        document
            .add_page_contents(page_id, content)
            .map_err(|_| OxideError::WritePdf)?;
    }
    let bytes = save_pdf(document)?;
    enforce_output_bytes(bytes.len(), limits)?;
    Ok(PdfArtifact {
        bytes: bytes.into(),
    })
}

pub(crate) fn parse_page_range(pages: &str, page_count: u32) -> Result<Vec<u32>, OxideError> {
    if pages.trim().is_empty() {
        return Err(OxideError::InvalidInput {
            reason: "page range must not be empty".to_owned(),
        });
    }

    let mut selected = Vec::new();
    for part in pages.split(',') {
        let part = part.trim();
        if part.is_empty() {
            return Err(OxideError::InvalidInput {
                reason: "page range contains an empty item".to_owned(),
            });
        }

        if let Some((start, end)) = part.split_once('-') {
            let start = parse_page_number(start.trim(), page_count)?;
            let end = parse_page_number(end.trim(), page_count)?;
            if start > end {
                return Err(OxideError::InvalidInput {
                    reason: format!("page range '{part}' must be ascending"),
                });
            }
            selected.extend(start..=end);
        } else {
            selected.push(parse_page_number(part, page_count)?);
        }
    }
    let unique_pages = selected.iter().copied().collect::<BTreeSet<_>>();
    if unique_pages.len() != selected.len() {
        return Err(OxideError::InvalidInput {
            reason: "page range must not contain duplicate pages".to_owned(),
        });
    }

    Ok(selected)
}

fn parse_page_number(value: &str, page_count: u32) -> Result<u32, OxideError> {
    let page = value.parse::<u32>().map_err(|_| OxideError::InvalidInput {
        reason: format!("invalid page number '{value}'"),
    })?;
    if page == 0 || page > page_count {
        return Err(OxideError::InvalidInput {
            reason: format!("page {page} is out of range 1-{page_count}"),
        });
    }

    Ok(page)
}

fn selected_or_all_pages(pages: Option<&str>, page_count: u32) -> Result<Vec<u32>, OxideError> {
    match pages {
        Some(pages) => parse_page_range(pages, page_count),
        None => Ok((1..=page_count).collect()),
    }
}

fn validated_rect(left: f32, bottom: f32, right: f32, top: f32) -> Result<[f32; 4], OxideError> {
    if [left, bottom, right, top]
        .into_iter()
        .any(|value| !value.is_finite())
    {
        return Err(OxideError::InvalidInput {
            reason: "page box coordinates must be finite".to_owned(),
        });
    }
    if left >= right || bottom >= top {
        return Err(OxideError::InvalidInput {
            reason: "page box coordinates must satisfy left < right and bottom < top".to_owned(),
        });
    }

    Ok([left, bottom, right, top])
}

fn crop_box_object(rect: [f32; 4]) -> Object {
    Object::Array(rect.into_iter().map(Object::Real).collect())
}

fn normalize_rotation(degrees: i16) -> Result<i16, OxideError> {
    match degrees.rem_euclid(360) {
        90 => Ok(90),
        180 => Ok(180),
        270 => Ok(270),
        _ => Err(OxideError::InvalidInput {
            reason: "rotation must be 90, 180, or 270 degrees".to_owned(),
        }),
    }
}

fn keep_pages(document: &mut lopdf::Document, selected_pages: &[u32]) -> Result<(), OxideError> {
    let page_count = document.get_pages().len() as u32;
    if selected_pages.is_empty() {
        return Err(OxideError::InvalidInput {
            reason: "at least one page must be selected".to_owned(),
        });
    }
    let pages_before_delete = document.get_pages();
    let selected_page_ids = selected_pages
        .iter()
        .map(|page| {
            pages_before_delete
                .get(page)
                .copied()
                .ok_or_else(|| OxideError::InvalidInput {
                    reason: format!("page {page} is out of range"),
                })
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mut delete_pages = (1..=page_count)
        .filter(|page| !selected_pages.contains(page))
        .collect::<Vec<_>>();
    delete_pages.sort_unstable_by(|left, right| right.cmp(left));
    document.delete_pages(&delete_pages);
    rebuild_pages_tree(document, &selected_page_ids)
}

fn merge_documents(documents: Vec<lopdf::Document>) -> Result<Vec<u8>, OxideError> {
    let mut next_id = 1;
    let mut merged = lopdf::Document::with_version("1.7");
    let mut document_pages = BTreeMap::new();
    let mut document_objects = BTreeMap::new();

    for mut document in documents {
        document.renumber_objects_with(next_id);
        next_id = document.max_id + 1;

        for page_id in document.get_pages().into_values() {
            let page = document
                .get_object(page_id)
                .cloned()
                .map_err(|_| OxideError::ParsePdf)?;
            document_pages.insert(page_id, page);
        }
        document_objects.extend(document.objects);
    }

    let mut catalog_object = None;
    let mut pages_object = None;
    for (object_id, object) in document_objects {
        match object.type_name().unwrap_or(b"") {
            b"Catalog" => {
                if catalog_object.is_none() {
                    catalog_object = Some((object_id, object));
                }
            }
            b"Pages" => {
                if pages_object.is_none() {
                    pages_object = Some((object_id, object));
                }
            }
            b"Page" | b"Outlines" | b"Outline" => {}
            _ => {
                merged.objects.insert(object_id, object);
            }
        }
    }

    let (pages_id, pages_object) = pages_object.ok_or(OxideError::ParsePdf)?;
    for (page_id, page) in &document_pages {
        let dictionary = page.as_dict().map_err(|_| OxideError::ParsePdf)?;
        let mut dictionary = dictionary.clone();
        dictionary.set("Parent", pages_id);
        merged
            .objects
            .insert(*page_id, lopdf::Object::Dictionary(dictionary));
    }

    let mut pages_dictionary = pages_object
        .as_dict()
        .map_err(|_| OxideError::ParsePdf)?
        .clone();
    pages_dictionary.set("Count", document_pages.len() as u32);
    pages_dictionary.set(
        "Kids",
        document_pages
            .keys()
            .copied()
            .map(lopdf::Object::Reference)
            .collect::<Vec<_>>(),
    );
    merged
        .objects
        .insert(pages_id, lopdf::Object::Dictionary(pages_dictionary));

    let (catalog_id, catalog_object) = catalog_object.ok_or(OxideError::ParsePdf)?;
    let mut catalog_dictionary = catalog_object
        .as_dict()
        .map_err(|_| OxideError::ParsePdf)?
        .clone();
    catalog_dictionary.set("Pages", pages_id);
    catalog_dictionary.remove(b"Outlines");
    merged
        .objects
        .insert(catalog_id, lopdf::Object::Dictionary(catalog_dictionary));
    merged.trailer.set("Root", catalog_id);
    merged.max_id = merged
        .objects
        .keys()
        .map(|(id, _)| *id)
        .max()
        .unwrap_or_default();

    save_pdf(merged)
}

fn page_is_structurally_blank(
    document: &lopdf::Document,
    page_id: lopdf::ObjectId,
) -> Result<bool, OxideError> {
    let page = document
        .get_object(page_id)
        .and_then(Object::as_dict)
        .map_err(|_| OxideError::ParsePdf)?;
    let has_content = match page.get(b"Contents") {
        Ok(Object::Array(items)) => !items.is_empty(),
        Ok(Object::Stream(stream)) => !stream.content.is_empty(),
        Ok(Object::Reference(id)) => {
            let stream = document
                .get_object(*id)
                .and_then(Object::as_stream)
                .map_err(|_| OxideError::ParsePdf)?;
            !stream.content.is_empty()
        }
        Ok(Object::Null) | Err(_) => false,
        Ok(_) => true,
    };
    if has_content {
        return Ok(false);
    }
    let has_resources = match page.get(b"Resources") {
        Ok(Object::Dictionary(dictionary)) => !dictionary.is_empty(),
        Ok(Object::Reference(id)) => {
            let dictionary = document
                .get_object(*id)
                .and_then(Object::as_dict)
                .map_err(|_| OxideError::ParsePdf)?;
            !dictionary.is_empty()
        }
        Ok(_) => true,
        Err(_) => false,
    };

    Ok(!has_resources)
}

fn scale_page_boxes(
    document: &mut lopdf::Document,
    page_id: lopdf::ObjectId,
    factor: f32,
) -> Result<(), OxideError> {
    let page = document
        .get_object_mut(page_id)
        .and_then(Object::as_dict_mut)
        .map_err(|_| OxideError::ParsePdf)?;
    for key in [
        b"MediaBox".as_slice(),
        b"CropBox",
        b"BleedBox",
        b"TrimBox",
        b"ArtBox",
    ] {
        if let Ok(object) = page.get_mut(key) {
            scale_box_object(object, factor)?;
        }
    }
    Ok(())
}

fn scale_box_object(object: &mut Object, factor: f32) -> Result<(), OxideError> {
    let values = object.as_array_mut().map_err(|_| OxideError::ParsePdf)?;
    if values.len() != 4 {
        return Err(OxideError::ParsePdf);
    }
    for value in values {
        *value = Object::Real(object_to_f32(value)? * factor);
    }
    Ok(())
}

fn prepend_page_transform(
    document: &mut lopdf::Document,
    page_id: lopdf::ObjectId,
    factor: f32,
) -> Result<(), OxideError> {
    let existing = document
        .get_page_content(page_id)
        .map_err(|_| OxideError::ParsePdf)?;
    let mut operations = vec![
        lopdf::content::Operation::new("q", vec![]),
        lopdf::content::Operation::new(
            "cm",
            vec![
                Object::Real(factor),
                Object::Real(0.0),
                Object::Real(0.0),
                Object::Real(factor),
                Object::Real(0.0),
                Object::Real(0.0),
            ],
        ),
    ];
    operations.extend(
        lopdf::content::Content::decode(&existing)
            .map_err(|_| OxideError::ParsePdf)?
            .operations,
    );
    operations.push(lopdf::content::Operation::new("Q", vec![]));

    let content = lopdf::content::Content { operations }
        .encode()
        .map_err(|_| OxideError::WritePdf)?;
    let content_id = document.add_object(Stream::new(Dictionary::new(), content));
    let page = document
        .get_object_mut(page_id)
        .and_then(Object::as_dict_mut)
        .map_err(|_| OxideError::ParsePdf)?;
    page.set("Contents", Object::Reference(content_id));
    Ok(())
}

fn merge_page_resources_into(
    document: &lopdf::Document,
    page_id: lopdf::ObjectId,
    resources: &mut Dictionary,
) -> Result<(), OxideError> {
    let (direct_resources, inherited_resource_ids) = document
        .get_page_resources(page_id)
        .map_err(|_| OxideError::ParsePdf)?;
    for resource_id in inherited_resource_ids.iter().rev() {
        let inherited = document
            .get_dictionary(*resource_id)
            .map_err(|_| OxideError::ParsePdf)?;
        merge_resource_dictionary(resources, inherited);
    }
    if let Some(direct) = direct_resources {
        merge_resource_dictionary(resources, direct);
    }
    Ok(())
}

#[derive(Debug, Clone, Copy)]
struct PageLayout {
    output_width: f32,
    output_height: f32,
}

fn validate_nup_options(options: &NUpOptions) -> Result<(), OxideError> {
    if options.columns == 0 || options.rows == 0 {
        return Err(OxideError::InvalidInput {
            reason: "nup columns and rows must be greater than zero".to_owned(),
        });
    }
    if options.columns > 8 || options.rows > 8 {
        return Err(OxideError::InvalidInput {
            reason: "nup columns and rows must be 8 or less".to_owned(),
        });
    }
    Ok(())
}

fn validate_page_number_options(options: &PageNumbersOptions) -> Result<(), OxideError> {
    if options.start == 0 {
        return Err(OxideError::InvalidInput {
            reason: "page number start must be greater than zero".to_owned(),
        });
    }
    if !options.font_size.is_finite() || options.font_size <= 0.0 {
        return Err(OxideError::InvalidInput {
            reason: "page number font size must be greater than zero".to_owned(),
        });
    }
    if !options.prefix.is_ascii() || !options.suffix.is_ascii() {
        return Err(OxideError::InvalidInput {
            reason: "page number prefix and suffix must be ASCII".to_owned(),
        });
    }
    Ok(())
}

fn page_layout_from_first_page(
    document: &lopdf::Document,
    page_ids: &[lopdf::ObjectId],
) -> Result<PageLayout, OxideError> {
    let first_page = page_ids.first().copied().ok_or(OxideError::ParsePdf)?;
    let (source_width, source_height) = page_size(document, first_page)?;
    if !source_width.is_finite()
        || !source_height.is_finite()
        || source_width <= 0.0
        || source_height <= 0.0
    {
        return Err(OxideError::ParsePdf);
    }

    Ok(PageLayout {
        output_width: source_width,
        output_height: source_height,
    })
}

fn booklet_page_order(page_count: usize) -> Vec<usize> {
    let padded_count = page_count.div_ceil(4) * 4;
    let mut order = Vec::with_capacity(padded_count);
    for sheet in 0..(padded_count / 4) {
        let left_front = padded_count - sheet * 2 - 1;
        let right_front = sheet * 2;
        let left_back = sheet * 2 + 1;
        let right_back = padded_count - sheet * 2 - 2;
        order.extend([left_front, right_front, left_back, right_back]);
    }
    order
}

fn impose_pages(
    source: &lopdf::Document,
    page_ids: &[lopdf::ObjectId],
    order: &[usize],
    layout: PageLayout,
    columns: u32,
    rows: u32,
) -> Result<Vec<u8>, OxideError> {
    let mut target = lopdf::Document::with_version("1.7");
    let catalog_id = target.new_object_id();
    let pages_id = target.new_object_id();
    let mut output_page_ids = Vec::new();
    let mut imported = BTreeMap::new();
    let slots_per_page = (columns * rows) as usize;

    for chunk in order.chunks(slots_per_page) {
        let page_id = target.new_object_id();
        let content_id = target.new_object_id();
        let mut resources = Dictionary::new();
        let mut xobjects = Dictionary::new();
        let mut operations = Vec::new();

        for (slot, source_index) in chunk.iter().enumerate() {
            if *source_index >= page_ids.len() {
                continue;
            }
            let source_page_id = page_ids[*source_index];
            let (source_width, source_height) = page_size(source, source_page_id)?;
            let xobject_id =
                page_form_xobject_from_source(source, &mut target, source_page_id, &mut imported)?;
            let resource_name = format!("OxPg{slot}").into_bytes();
            xobjects.set(resource_name.clone(), Object::Reference(xobject_id));
            operations.extend(imposed_page_operations(
                &resource_name,
                slot,
                columns,
                rows,
                layout,
                source_width,
                source_height,
            )?);
        }

        resources.set("XObject", Object::Dictionary(xobjects));
        let content = lopdf::content::Content { operations }
            .encode()
            .map_err(|_| OxideError::WritePdf)?;
        target.objects.insert(
            content_id,
            Object::Stream(Stream::new(Dictionary::new(), content)),
        );
        target.objects.insert(
            page_id,
            Object::Dictionary(dictionary! {
                "Type" => "Page",
                "Parent" => pages_id,
                "MediaBox" => crop_box_object([0.0, 0.0, layout.output_width, layout.output_height]),
                "CropBox" => crop_box_object([0.0, 0.0, layout.output_width, layout.output_height]),
                "Resources" => Object::Dictionary(resources),
                "Contents" => Object::Reference(content_id),
            }),
        );
        output_page_ids.push(page_id);
    }

    target.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => Object::Array(output_page_ids.iter().copied().map(Object::Reference).collect()),
            "Count" => output_page_ids.len() as u32,
        }),
    );
    target.objects.insert(
        catalog_id,
        Object::Dictionary(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        }),
    );
    target.trailer.set("Root", catalog_id);
    save_pdf(target)
}

fn imposed_page_operations(
    resource_name: &[u8],
    slot: usize,
    columns: u32,
    rows: u32,
    layout: PageLayout,
    source_width: f32,
    source_height: f32,
) -> Result<Vec<lopdf::content::Operation>, OxideError> {
    if !source_width.is_finite()
        || !source_height.is_finite()
        || source_width <= 0.0
        || source_height <= 0.0
    {
        return Err(OxideError::ParsePdf);
    }
    let cell_width = layout.output_width / columns as f32;
    let cell_height = layout.output_height / rows as f32;
    let column = (slot as u32) % columns;
    let row_from_top = (slot as u32) / columns;
    let scale = (cell_width / source_width).min(cell_height / source_height);
    let width = source_width * scale;
    let height = source_height * scale;
    let x = column as f32 * cell_width + (cell_width - width) / 2.0;
    let y = layout.output_height - (row_from_top + 1) as f32 * cell_height
        + (cell_height - height) / 2.0;

    Ok(vec![
        lopdf::content::Operation::new("q", vec![]),
        lopdf::content::Operation::new(
            "cm",
            vec![
                Object::Real(scale),
                Object::Real(0.0),
                Object::Real(0.0),
                Object::Real(scale),
                Object::Real(x),
                Object::Real(y),
            ],
        ),
        lopdf::content::Operation::new("Do", vec![Object::Name(resource_name.to_vec())]),
        lopdf::content::Operation::new("Q", vec![]),
    ])
}

fn page_form_xobject_from_source(
    source: &lopdf::Document,
    target: &mut lopdf::Document,
    page_id: lopdf::ObjectId,
    imported: &mut BTreeMap<lopdf::ObjectId, lopdf::ObjectId>,
) -> Result<lopdf::ObjectId, OxideError> {
    let content = source
        .get_page_content(page_id)
        .map_err(|_| OxideError::ParsePdf)?;
    let resources = imported_page_resources_with_cache(source, target, page_id, imported)?;
    let (width, height) = page_size(source, page_id)?;
    if width <= 0.0 || height <= 0.0 {
        return Err(OxideError::ParsePdf);
    }
    let mut dictionary = dictionary! {
        "Type" => "XObject",
        "Subtype" => "Form",
        "BBox" => crop_box_object([0.0, 0.0, width, height]),
        "Matrix" => Object::Array(vec![
            Object::Real(1.0),
            Object::Real(0.0),
            Object::Real(0.0),
            Object::Real(1.0),
            Object::Real(0.0),
            Object::Real(0.0),
        ]),
    };
    dictionary.set("Resources", resources);
    Ok(target.add_object(Stream::new(dictionary, content)))
}

fn imported_page_resources_with_cache(
    source: &lopdf::Document,
    target: &mut lopdf::Document,
    page_id: lopdf::ObjectId,
    imported: &mut BTreeMap<lopdf::ObjectId, lopdf::ObjectId>,
) -> Result<Dictionary, OxideError> {
    let (direct_resources, inherited_resource_ids) = source
        .get_page_resources(page_id)
        .map_err(|_| OxideError::ParsePdf)?;
    let mut resources = Dictionary::new();
    for resource_id in inherited_resource_ids.iter().rev() {
        let inherited = source
            .get_dictionary(*resource_id)
            .map_err(|_| OxideError::ParsePdf)?;
        merge_resource_dictionary(&mut resources, inherited);
    }
    if let Some(direct) = direct_resources {
        merge_resource_dictionary(&mut resources, direct);
    }
    let mut resource_object = Object::Dictionary(resources);
    remap_imported_references(&mut resource_object, source, target, imported)?;
    resource_object
        .as_dict()
        .cloned()
        .map_err(|_| OxideError::ParsePdf)
}

fn add_standard_font_resource(
    document: &mut lopdf::Document,
    pages: &[u32],
    resource_name: Vec<u8>,
) -> Result<(), OxideError> {
    let font_id = document.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
        "Encoding" => "WinAnsiEncoding",
    });
    let page_map = document.get_pages();
    let page_ids = pages
        .iter()
        .map(|page| {
            page_map
                .get(page)
                .copied()
                .ok_or_else(|| OxideError::InvalidInput {
                    reason: format!("page {page} is out of range"),
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    for page_id in page_ids {
        add_resource_dict_entry(
            document,
            page_id,
            b"Font",
            resource_name.clone(),
            Object::Reference(font_id),
        )?;
    }
    Ok(())
}

fn page_number_content(
    label: &str,
    page_width: f32,
    page_height: f32,
    font_size: f32,
    position: PageNumberPosition,
) -> Result<Vec<u8>, OxideError> {
    let escaped_label = pdf_literal_ascii(label)?;
    let width = label.chars().count() as f32 * font_size * 0.5;
    let margin = 36.0;
    let (x, y) = match position {
        PageNumberPosition::TopLeft => (margin, page_height - margin),
        PageNumberPosition::TopCenter => ((page_width - width) / 2.0, page_height - margin),
        PageNumberPosition::TopRight => (page_width - margin - width, page_height - margin),
        PageNumberPosition::BottomLeft => (margin, margin),
        PageNumberPosition::BottomCenter => ((page_width - width) / 2.0, margin),
        PageNumberPosition::BottomRight => (page_width - margin - width, margin),
    };
    let content = format!("q BT /OxPnF1 {font_size} Tf {x} {y} Td ({escaped_label}) Tj ET Q\n");
    Ok(content.into_bytes())
}

fn pdf_literal_ascii(value: &str) -> Result<String, OxideError> {
    if !value.is_ascii() {
        return Err(OxideError::InvalidInput {
            reason: "page number text must be ASCII".to_owned(),
        });
    }
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '(' => escaped.push_str("\\("),
            ')' => escaped.push_str("\\)"),
            '\\' => escaped.push_str("\\\\"),
            _ => escaped.push(character),
        }
    }
    Ok(escaped)
}
