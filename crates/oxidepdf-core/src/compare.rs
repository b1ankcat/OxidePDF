use crate::{
    enforce_input_bytes, enforce_max_pages, enforce_max_pixels, enforce_output_bytes,
    extract_text_from_pdf, inspect_pdf_annotations, inspect_pdf_attachments, inspect_pdf_forms,
    inspect_pdf_metadata, inspect_pdf_outline, load_pdf, page_size, render_pdf_page,
    AnnotationInspectOptions, AttachmentInspectOptions, ExtractTextOptions, FormInspectOptions,
    ImageArtifact, MetadataInspectOptions, OutlineInspectOptions, OxideError, RenderOptions,
    ResourceLimits, TextArtifact,
};
use image::{ImageBuffer, ImageFormat, Rgba, RgbaImage};
use lopdf::Object;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::io::Cursor;

/// PDF comparison operations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "PdfCompareOptionsDef", into = "PdfCompareOptionsDef")]
pub enum PdfCompareOptions {
    /// Generate a structured JSON comparison report.
    Report(CompareOptions),
    /// Render and compare one page visually.
    VisualDiff(VisualDiffOptions),
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct PdfCompareOptionsDef {
    report: Option<CompareOptions>,
    visual_diff: Option<VisualDiffOptions>,
}

impl TryFrom<PdfCompareOptionsDef> for PdfCompareOptions {
    type Error = OxideError;

    fn try_from(value: PdfCompareOptionsDef) -> Result<Self, Self::Error> {
        let operation_count = [value.report.is_some(), value.visual_diff.is_some()]
            .into_iter()
            .filter(|present| *present)
            .count();

        if operation_count != 1 {
            return Err(OxideError::InvalidWorkflow {
                reason: "pdf_compare must contain exactly one operation".to_owned(),
            });
        }

        if let Some(options) = value.report {
            return Ok(Self::Report(options));
        }
        if let Some(options) = value.visual_diff {
            return Ok(Self::VisualDiff(options));
        }

        unreachable!("operation count was already checked");
    }
}

impl From<PdfCompareOptions> for PdfCompareOptionsDef {
    fn from(value: PdfCompareOptions) -> Self {
        match value {
            PdfCompareOptions::Report(options) => Self {
                report: Some(options),
                ..Self::default()
            },
            PdfCompareOptions::VisualDiff(options) => Self {
                visual_diff: Some(options),
                ..Self::default()
            },
        }
    }
}

/// Options for structured PDF comparison reports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CompareOptions {
    /// Include extractable text summaries in the comparison.
    pub include_text: bool,
    /// Maximum text characters retained per side in summaries.
    pub text_max_chars: usize,
}

impl Default for CompareOptions {
    fn default() -> Self {
        Self {
            include_text: true,
            text_max_chars: 1024,
        }
    }
}

/// Options for rendered page visual differences.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct VisualDiffOptions {
    /// One-based page number to render from both documents.
    pub page: u32,
    /// Render scale applied to both documents.
    pub scale: Option<f32>,
}

impl Default for VisualDiffOptions {
    fn default() -> Self {
        Self {
            page: 1,
            scale: Some(1.0),
        }
    }
}

/// Structured comparison report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompareReport {
    /// True when no differences were found.
    pub equal: bool,
    /// Summary of the left PDF.
    pub left: PdfCompareSummary,
    /// Summary of the right PDF.
    pub right: PdfCompareSummary,
    /// Stable machine-readable differences.
    pub differences: Vec<CompareDifference>,
}

/// Stable machine-readable comparison difference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompareDifference {
    /// Stable difference code.
    pub code: CompareDifferenceCode,
    /// Stable path to the differing property.
    pub path: String,
    /// Left-side value.
    pub left: Value,
    /// Right-side value.
    pub right: Value,
}

/// Stable difference codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompareDifferenceCode {
    PageCountMismatch,
    PageSizeMismatch,
    MetadataMismatch,
    OutlineMismatch,
    AttachmentsMismatch,
    AnnotationsMismatch,
    FormsMismatch,
    TextMismatch,
    ObjectStructureMismatch,
}

/// Summary fields used by structured PDF comparison.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PdfCompareSummary {
    pub page_count: usize,
    pub page_sizes: Vec<PageSizeSummary>,
    pub metadata: Value,
    pub outline: Value,
    pub attachments: Value,
    pub annotations: Value,
    pub forms: Value,
    pub text: Option<TextSummary>,
    pub object_structure: ObjectStructureSummary,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PageSizeSummary {
    pub page: u32,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextSummary {
    pub char_count: usize,
    pub sample: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObjectStructureSummary {
    pub object_count: usize,
    pub stream_count: usize,
    pub dictionary_count: usize,
    pub array_count: usize,
    pub string_count: usize,
    pub numeric_count: usize,
    pub boolean_count: usize,
    pub null_count: usize,
    pub named_type_counts: BTreeMap<String, usize>,
}

pub fn compare_pdf_report(
    left: &[u8],
    right: &[u8],
    options: &CompareOptions,
    limits: &ResourceLimits,
) -> Result<TextArtifact, OxideError> {
    enforce_compare_inputs(left, right, limits)?;
    let left_summary = summarize_pdf(left, options, limits)?;
    let right_summary = summarize_pdf(right, options, limits)?;
    let differences = compare_summaries(&left_summary, &right_summary);
    let report = CompareReport {
        equal: differences.is_empty(),
        left: left_summary,
        right: right_summary,
        differences,
    };
    let text = serde_json::to_string_pretty(&report).map_err(|_| OxideError::Internal)?;
    enforce_output_bytes(text.len(), limits)?;
    Ok(TextArtifact {
        text,
        diagnostics: Vec::new(),
    })
}

pub fn compare_pdf_visual_diff(
    left: &[u8],
    right: &[u8],
    options: &VisualDiffOptions,
    limits: &ResourceLimits,
) -> Result<ImageArtifact, OxideError> {
    enforce_compare_inputs(left, right, limits)?;
    if options.page == 0 {
        return Err(OxideError::InvalidInput {
            reason: "page number must be one or greater".to_owned(),
        });
    }

    let render_options = RenderOptions {
        page: options.page,
        format: Some("png".to_owned()),
        scale: options.scale,
    };
    let left_render = render_pdf_page(left, &render_options, limits)?;
    let right_render = render_pdf_page(right, &render_options, limits)?;
    let left_image = image::load_from_memory(&left_render.bytes)
        .map_err(|_| OxideError::RenderPdf)?
        .to_rgba8();
    let right_image = image::load_from_memory(&right_render.bytes)
        .map_err(|_| OxideError::RenderPdf)?
        .to_rgba8();

    let width = left_image.width().max(right_image.width());
    let height = left_image.height().max(right_image.height());
    let pixels = u64::from(width) * u64::from(height);
    enforce_max_pixels(pixels, limits)?;

    let mut diff: RgbaImage = ImageBuffer::from_pixel(width, height, Rgba([255, 255, 255, 255]));
    for y in 0..height {
        for x in 0..width {
            let left_pixel = pixel_at(&left_image, x, y);
            let right_pixel = pixel_at(&right_image, x, y);
            if left_pixel != right_pixel {
                diff.put_pixel(x, y, Rgba([220, 38, 38, 255]));
            }
        }
    }

    let mut bytes = Vec::new();
    diff.write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
        .map_err(|_| OxideError::RenderPdf)?;
    if bytes.is_empty() {
        return Err(OxideError::RenderPdf);
    }
    enforce_output_bytes(bytes.len(), limits)?;
    Ok(ImageArtifact { bytes })
}

fn enforce_compare_inputs(
    left: &[u8],
    right: &[u8],
    limits: &ResourceLimits,
) -> Result<(), OxideError> {
    enforce_input_bytes(left.len(), limits)?;
    enforce_input_bytes(right.len(), limits)?;
    Ok(())
}

fn summarize_pdf(
    input: &[u8],
    options: &CompareOptions,
    limits: &ResourceLimits,
) -> Result<PdfCompareSummary, OxideError> {
    let document = load_pdf(input)?;
    let pages = document.get_pages();
    enforce_max_pages(pages.len(), limits)?;
    let mut page_sizes = Vec::with_capacity(pages.len());
    for (page_number, page_id) in pages {
        let (width, height) = page_size(&document, page_id)?;
        page_sizes.push(PageSizeSummary {
            page: page_number,
            width,
            height,
        });
    }

    Ok(PdfCompareSummary {
        page_count: page_sizes.len(),
        page_sizes,
        metadata: inspect_json(input, |bytes| {
            inspect_pdf_metadata(bytes, &MetadataInspectOptions::default())
        })?,
        outline: inspect_json(input, |bytes| {
            inspect_pdf_outline(bytes, &OutlineInspectOptions::default())
        })?,
        attachments: inspect_json(input, |bytes| {
            inspect_pdf_attachments(bytes, &AttachmentInspectOptions::default())
        })?,
        annotations: inspect_json(input, |bytes| {
            inspect_pdf_annotations(bytes, &AnnotationInspectOptions::default())
        })?,
        forms: inspect_json(input, |bytes| {
            inspect_pdf_forms(bytes, &FormInspectOptions::default())
        })?,
        text: if options.include_text {
            Some(text_summary(input, options.text_max_chars, limits))
        } else {
            None
        },
        object_structure: summarize_object_structure(&document),
    })
}

fn inspect_json(
    input: &[u8],
    inspect: impl FnOnce(&[u8]) -> Result<TextArtifact, OxideError>,
) -> Result<Value, OxideError> {
    let artifact = inspect(input)?;
    serde_json::from_str(&artifact.text).map_err(|_| OxideError::Internal)
}

fn text_summary(input: &[u8], max_chars: usize, limits: &ResourceLimits) -> TextSummary {
    let text = extract_text_from_pdf(input, &ExtractTextOptions::default(), limits)
        .map(|artifact| artifact.text)
        .unwrap_or_default();
    TextSummary {
        char_count: text.chars().count(),
        sample: text.chars().take(max_chars).collect(),
    }
}

fn summarize_object_structure(document: &lopdf::Document) -> ObjectStructureSummary {
    let mut summary = ObjectStructureSummary {
        object_count: document.objects.len(),
        stream_count: 0,
        dictionary_count: 0,
        array_count: 0,
        string_count: 0,
        numeric_count: 0,
        boolean_count: 0,
        null_count: 0,
        named_type_counts: BTreeMap::new(),
    };

    for object in document.objects.values() {
        count_object(object, &mut summary);
    }

    summary
}

fn count_object(object: &Object, summary: &mut ObjectStructureSummary) {
    match object {
        Object::Array(items) => {
            summary.array_count += 1;
            for item in items {
                count_object(item, summary);
            }
        }
        Object::Dictionary(dictionary) => {
            summary.dictionary_count += 1;
            count_named_type(dictionary, summary);
            for (_, value) in dictionary.iter() {
                count_object(value, summary);
            }
        }
        Object::Stream(stream) => {
            summary.stream_count += 1;
            count_named_type(&stream.dict, summary);
            for (_, value) in stream.dict.iter() {
                count_object(value, summary);
            }
        }
        Object::String(_, _) => summary.string_count += 1,
        Object::Integer(_) | Object::Real(_) => summary.numeric_count += 1,
        Object::Boolean(_) => summary.boolean_count += 1,
        Object::Null => summary.null_count += 1,
        Object::Name(_) | Object::Reference(_) => {}
    }
}

fn count_named_type(dictionary: &lopdf::Dictionary, summary: &mut ObjectStructureSummary) {
    if let Ok(name) = dictionary.get(b"Type").and_then(Object::as_name) {
        let key = String::from_utf8_lossy(name).into_owned();
        *summary.named_type_counts.entry(key).or_insert(0) += 1;
    }
}

fn compare_summaries(
    left: &PdfCompareSummary,
    right: &PdfCompareSummary,
) -> Vec<CompareDifference> {
    let mut differences = Vec::new();
    push_difference(
        &mut differences,
        CompareDifferenceCode::PageCountMismatch,
        "pages.count",
        left.page_count,
        right.page_count,
    );

    for index in 0..left.page_sizes.len().min(right.page_sizes.len()) {
        push_difference(
            &mut differences,
            CompareDifferenceCode::PageSizeMismatch,
            format!("pages[{}].size", index + 1),
            &left.page_sizes[index],
            &right.page_sizes[index],
        );
    }
    push_difference(
        &mut differences,
        CompareDifferenceCode::MetadataMismatch,
        "metadata",
        &left.metadata,
        &right.metadata,
    );
    push_difference(
        &mut differences,
        CompareDifferenceCode::OutlineMismatch,
        "outline",
        &left.outline,
        &right.outline,
    );
    push_difference(
        &mut differences,
        CompareDifferenceCode::AttachmentsMismatch,
        "attachments",
        &left.attachments,
        &right.attachments,
    );
    push_difference(
        &mut differences,
        CompareDifferenceCode::AnnotationsMismatch,
        "annotations",
        &left.annotations,
        &right.annotations,
    );
    push_difference(
        &mut differences,
        CompareDifferenceCode::FormsMismatch,
        "forms",
        &left.forms,
        &right.forms,
    );
    push_difference(
        &mut differences,
        CompareDifferenceCode::TextMismatch,
        "text",
        &left.text,
        &right.text,
    );
    push_difference(
        &mut differences,
        CompareDifferenceCode::ObjectStructureMismatch,
        "objects",
        &left.object_structure,
        &right.object_structure,
    );

    differences
}

fn push_difference(
    differences: &mut Vec<CompareDifference>,
    code: CompareDifferenceCode,
    path: impl Into<String>,
    left: impl Serialize,
    right: impl Serialize,
) {
    let left = serde_json::to_value(left).unwrap_or_else(|_| json!(null));
    let right = serde_json::to_value(right).unwrap_or_else(|_| json!(null));
    if left != right {
        differences.push(CompareDifference {
            code,
            path: path.into(),
            left,
            right,
        });
    }
}

fn pixel_at(image: &RgbaImage, x: u32, y: u32) -> Option<Rgba<u8>> {
    if x < image.width() && y < image.height() {
        Some(*image.get_pixel(x, y))
    } else {
        None
    }
}
