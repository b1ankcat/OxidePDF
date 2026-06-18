use super::types::{
    CompareDifference, CompareDifferenceCode, CompareOptions, CompareReport,
    ObjectStructureSummary, PageSizeSummary, PdfCompareSummary, TextSummary,
};
use crate::{
    AnnotationInspectOptions, AttachmentInspectOptions, ExtractTextOptions, FormInspectOptions,
    MetadataInspectOptions, OutlineInspectOptions, OxideError, ResourceLimits, TextArtifact,
    enforce_input_bytes, enforce_max_pages, enforce_output_bytes, extract_text_from_pdf,
    inspect_pdf_annotations_with_limits, inspect_pdf_attachments_with_limits,
    inspect_pdf_forms_with_limits, inspect_pdf_metadata_with_limits,
    inspect_pdf_outline_with_limits, load_pdf, page_size,
};
use lopdf::Object;
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;

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

pub(super) fn enforce_compare_inputs(
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
            inspect_pdf_metadata_with_limits(bytes, &MetadataInspectOptions::default(), limits)
        })?,
        outline: inspect_json(input, |bytes| {
            inspect_pdf_outline_with_limits(bytes, &OutlineInspectOptions::default(), limits)
        })?,
        attachments: inspect_json(input, |bytes| {
            inspect_pdf_attachments_with_limits(bytes, &AttachmentInspectOptions::default(), limits)
        })?,
        annotations: inspect_json(input, |bytes| {
            inspect_pdf_annotations_with_limits(bytes, &AnnotationInspectOptions::default(), limits)
        })?,
        forms: inspect_json(input, |bytes| {
            inspect_pdf_forms_with_limits(bytes, &FormInspectOptions::default(), limits)
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
    let left = serde_json::to_value(left).unwrap_or(json!(null));
    let right = serde_json::to_value(right).unwrap_or(json!(null));
    if left != right {
        differences.push(CompareDifference {
            code,
            path: path.into(),
            left,
            right,
        });
    }
}
