use crate::OxideError;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

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
