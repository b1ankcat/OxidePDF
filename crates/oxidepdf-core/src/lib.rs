#![forbid(unsafe_code)]
#![doc = "Core contracts and shared logic for OxidePDF."]

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;

/// Current workflow schema version.
///
/// Stage 1 only establishes the crate boundary. Stage 2 will add the full
/// serialized workflow contract around this version.
pub const WORKFLOW_SCHEMA_VERSION: u16 = 1;

/// Supported workflow schema versions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "u16", into = "u16")]
pub enum WorkflowVersion {
    /// Initial public workflow schema.
    V1,
}

impl TryFrom<u16> for WorkflowVersion {
    type Error = OxideError;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            WORKFLOW_SCHEMA_VERSION => Ok(Self::V1),
            version => Err(OxideError::InvalidWorkflow {
                reason: format!("unsupported workflow version {version}"),
            }),
        }
    }
}

impl From<WorkflowVersion> for u16 {
    fn from(value: WorkflowVersion) -> Self {
        match value {
            WorkflowVersion::V1 => WORKFLOW_SCHEMA_VERSION,
        }
    }
}

/// Complete workflow submitted by CLI, Web, or WASM clients.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Workflow {
    /// Schema version for this workflow document.
    pub version: WorkflowVersion,
    /// External inputs available to tasks.
    pub inputs: Vec<InputSpec>,
    /// Ordered or dependency-connected work items.
    pub tasks: Vec<TaskSpec>,
    /// Final artifacts to materialize.
    pub outputs: Vec<OutputSpec>,
    /// Resource limits applied while validating and executing the workflow.
    #[serde(default)]
    pub limits: ResourceLimits,
    /// Caller-provided metadata for diagnostics and later UI display.
    #[serde(default)]
    pub metadata: WorkflowMetadata,
}

/// External workflow input.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputSpec {
    /// Stable input identifier.
    pub id: ArtifactRef,
    /// File path or `-` for stdin.
    pub path: PathBuf,
}

/// Final workflow output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputSpec {
    /// Stable output identifier.
    pub id: ArtifactRef,
    /// Artifact to write.
    pub from: ArtifactRef,
    /// File path or `-` for stdout.
    pub path: PathBuf,
}

/// A single workflow task.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskSpec {
    /// Stable task identifier.
    pub id: TaskId,
    /// Operator and its options.
    pub op: OperatorSpec,
    /// Input artifact references consumed by this task.
    pub inputs: Vec<ArtifactRef>,
}

/// Task identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TaskId(String);

impl TaskId {
    /// Returns the underlying identifier.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Input, task, or output artifact reference.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ArtifactRef(String);

impl ArtifactRef {
    /// Returns the underlying reference.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Resource limits applied to workflow execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ResourceLimits {
    /// Maximum size of any single input, in bytes.
    pub max_input_bytes: Option<u64>,
    /// Maximum total size of all inputs, in bytes.
    pub max_total_input_bytes: Option<u64>,
    /// Maximum number of PDF pages.
    pub max_pages: Option<u32>,
    /// Maximum total image pixels.
    pub max_pixels: Option<u64>,
    /// Maximum output size, in bytes.
    pub max_output_bytes: Option<u64>,
    /// Maximum workflow runtime, in milliseconds.
    pub timeout_ms: Option<u64>,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_input_bytes: Some(512 * 1024 * 1024),
            max_total_input_bytes: Some(512 * 1024 * 1024),
            max_pages: Some(5_000),
            max_pixels: Some(160_000_000),
            max_output_bytes: None,
            timeout_ms: None,
        }
    }
}

/// Workflow metadata.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct WorkflowMetadata {
    /// Optional human-readable title.
    pub title: Option<String>,
}

/// Supported workflow operators.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "OperatorSpecDef", into = "OperatorSpecDef")]
pub enum OperatorSpec {
    /// Merge multiple PDFs.
    Merge(MergeOptions),
    /// Split a PDF by page range.
    Split(SplitOptions),
    /// Reorder pages in a PDF.
    Reorder(ReorderOptions),
    /// Rotate selected pages.
    Rotate(RotateOptions),
    /// Convert images to PDF pages.
    ImageToPdf(ImageToPdfOptions),
    /// Convert SVG to PDF.
    SvgToPdf(SvgToPdfOptions),
    /// Extract text from a PDF.
    ExtractText(ExtractTextOptions),
    /// Add a watermark to a PDF.
    Watermark(WatermarkOptions),
    /// Render PDF pages to images.
    Render(RenderOptions),
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct OperatorSpecDef {
    merge: Option<MergeOptions>,
    split: Option<SplitOptions>,
    reorder: Option<ReorderOptions>,
    rotate: Option<RotateOptions>,
    image_to_pdf: Option<ImageToPdfOptions>,
    svg_to_pdf: Option<SvgToPdfOptions>,
    extract_text: Option<ExtractTextOptions>,
    watermark: Option<WatermarkOptions>,
    render: Option<RenderOptions>,
}

impl TryFrom<OperatorSpecDef> for OperatorSpec {
    type Error = OxideError;

    fn try_from(value: OperatorSpecDef) -> Result<Self, Self::Error> {
        let operator_count = [
            value.merge.is_some(),
            value.split.is_some(),
            value.reorder.is_some(),
            value.rotate.is_some(),
            value.image_to_pdf.is_some(),
            value.svg_to_pdf.is_some(),
            value.extract_text.is_some(),
            value.watermark.is_some(),
            value.render.is_some(),
        ]
        .into_iter()
        .filter(|present| *present)
        .count();

        if operator_count != 1 {
            return Err(OxideError::InvalidWorkflow {
                reason: "operator spec must contain exactly one operator".to_owned(),
            });
        }

        if let Some(options) = value.merge {
            return Ok(Self::Merge(options));
        }
        if let Some(options) = value.split {
            return Ok(Self::Split(options));
        }
        if let Some(options) = value.reorder {
            return Ok(Self::Reorder(options));
        }
        if let Some(options) = value.rotate {
            return Ok(Self::Rotate(options));
        }
        if let Some(options) = value.image_to_pdf {
            return Ok(Self::ImageToPdf(options));
        }
        if let Some(options) = value.svg_to_pdf {
            return Ok(Self::SvgToPdf(options));
        }
        if let Some(options) = value.extract_text {
            return Ok(Self::ExtractText(options));
        }
        if let Some(options) = value.watermark {
            return Ok(Self::Watermark(options));
        }
        if let Some(options) = value.render {
            return Ok(Self::Render(options));
        }

        unreachable!("operator count was already checked");
    }
}

impl From<OperatorSpec> for OperatorSpecDef {
    fn from(value: OperatorSpec) -> Self {
        match value {
            OperatorSpec::Merge(options) => Self {
                merge: Some(options),
                ..Self::default()
            },
            OperatorSpec::Split(options) => Self {
                split: Some(options),
                ..Self::default()
            },
            OperatorSpec::Reorder(options) => Self {
                reorder: Some(options),
                ..Self::default()
            },
            OperatorSpec::Rotate(options) => Self {
                rotate: Some(options),
                ..Self::default()
            },
            OperatorSpec::ImageToPdf(options) => Self {
                image_to_pdf: Some(options),
                ..Self::default()
            },
            OperatorSpec::SvgToPdf(options) => Self {
                svg_to_pdf: Some(options),
                ..Self::default()
            },
            OperatorSpec::ExtractText(options) => Self {
                extract_text: Some(options),
                ..Self::default()
            },
            OperatorSpec::Watermark(options) => Self {
                watermark: Some(options),
                ..Self::default()
            },
            OperatorSpec::Render(options) => Self {
                render: Some(options),
                ..Self::default()
            },
        }
    }
}

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

/// Options for image-to-PDF conversion.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ImageToPdfOptions {
    /// Layout mode such as `fit`, `fill`, or `original_size`.
    pub layout: Option<String>,
}

/// Options for SVG-to-PDF conversion.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SvgToPdfOptions {
    /// User-selected rasterization mode. Defaults to vector output when false.
    pub rasterize: bool,
}

/// Options for text extraction.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ExtractTextOptions {
    /// Output format, initially `plain`.
    pub format: Option<String>,
}

/// Options for watermarking.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WatermarkOptions {
    /// Watermark kind.
    pub kind: WatermarkKind,
    /// Text for text watermarks.
    pub text: Option<String>,
    /// Opacity from 0.0 to 1.0.
    pub opacity: Option<f32>,
    /// Position such as `center`.
    pub position: Option<String>,
}

/// Watermark content kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WatermarkKind {
    /// Text watermark.
    Text,
    /// Image watermark.
    Image,
    /// SVG watermark.
    Svg,
}

/// Options for rendering.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RenderOptions {
    /// One-based page number.
    pub page: u32,
    /// Optional output format such as `png`.
    pub format: Option<String>,
    /// Optional render scale.
    pub scale: Option<f32>,
}

/// Structured core error.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum OxideError {
    /// Workflow is invalid.
    #[error("invalid workflow: {reason}")]
    InvalidWorkflow {
        /// Non-sensitive reason.
        reason: String,
    },
    /// Input is invalid.
    #[error("invalid input: {reason}")]
    InvalidInput {
        /// Non-sensitive reason.
        reason: String,
    },
    /// PDF feature is not supported.
    #[error("unsupported PDF feature: {feature}")]
    UnsupportedPdfFeature {
        /// Non-sensitive feature name.
        feature: String,
    },
    /// PDF is encrypted and no usable password was provided.
    #[error("encrypted PDF")]
    EncryptedPdf,
    /// Provided password is incorrect.
    #[error("incorrect password")]
    IncorrectPassword,
    /// PDF parsing failed.
    #[error("PDF parse error")]
    ParsePdf,
    /// PDF writing failed.
    #[error("PDF write error")]
    WritePdf,
    /// PDF rendering failed.
    #[error("PDF render error")]
    RenderPdf,
    /// Text extraction failed.
    #[error("text extraction error")]
    ExtractText,
    /// Font resolution failed.
    #[error("font resolution error")]
    FontResolution,
    /// SVG parsing failed.
    #[error("SVG parse error")]
    SvgParse,
    /// Image decoding failed.
    #[error("image decode error")]
    ImageDecode,
    /// A resource limit was exceeded.
    #[error("resource limit exceeded: {limit}")]
    ResourceLimitExceeded {
        /// Non-sensitive limit name.
        limit: String,
    },
    /// I/O failed.
    #[error("I/O error")]
    Io,
    /// Internal invariant failed.
    #[error("internal error")]
    Internal,
}

impl OxideError {
    /// Stable machine-readable error code for CLI and Web mappings.
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidWorkflow { .. } => "invalid_workflow",
            Self::InvalidInput { .. } => "invalid_input",
            Self::UnsupportedPdfFeature { .. } => "unsupported_pdf_feature",
            Self::EncryptedPdf => "encrypted_pdf",
            Self::IncorrectPassword => "incorrect_password",
            Self::ParsePdf => "parse_pdf",
            Self::WritePdf => "write_pdf",
            Self::RenderPdf => "render_pdf",
            Self::ExtractText => "extract_text",
            Self::FontResolution => "font_resolution",
            Self::SvgParse => "svg_parse",
            Self::ImageDecode => "image_decode",
            Self::ResourceLimitExceeded { .. } => "resource_limit_exceeded",
            Self::Io => "io",
            Self::Internal => "internal",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workflow_schema_version_starts_at_one() {
        assert_eq!(WORKFLOW_SCHEMA_VERSION, 1);
    }

    #[test]
    fn parses_example_json_workflow() {
        let workflow: Workflow = serde_json::from_str(
            r#"
            {
              "version": 1,
              "inputs": [
                { "id": "source", "path": "./input.pdf" }
              ],
              "tasks": [
                {
                  "id": "rotate_pages",
                  "op": {
                    "rotate": {
                      "pages": "1,3-5",
                      "degrees": 90
                    }
                  },
                  "inputs": ["source"]
                }
              ],
              "outputs": [
                { "id": "final", "from": "rotate_pages", "path": "./output.pdf" }
              ],
              "limits": {
                "max_input_bytes": 524288000,
                "max_pages": 5000,
                "max_pixels": 160000000
              },
              "metadata": {
                "title": "Example workflow"
              }
            }
            "#,
        )
        .unwrap();

        assert_eq!(workflow.version, WorkflowVersion::V1);
        assert_eq!(workflow.inputs[0].id.as_str(), "source");
        assert_eq!(workflow.tasks[0].id.as_str(), "rotate_pages");
        assert!(matches!(
            workflow.tasks[0].op,
            OperatorSpec::Rotate(RotateOptions { degrees: 90, .. })
        ));
        assert_eq!(workflow.outputs[0].from.as_str(), "rotate_pages");
    }

    #[test]
    fn parses_example_yaml_workflow() {
        let workflow: Workflow = serde_yaml::from_str(
            r#"
            version: 1
            inputs:
              - id: source
                path: ./input.pdf
            tasks:
              - id: rotate_pages
                op:
                  rotate:
                    pages: "1,3-5"
                    degrees: 90
                inputs: [source]
              - id: stamp
                op:
                  watermark:
                    kind: text
                    text: Confidential
                    opacity: 0.18
                    position: center
                inputs: [rotate_pages]
            outputs:
              - id: final
                from: stamp
                path: ./output.pdf
            limits:
              max_input_bytes: 524288000
              max_pages: 5000
              max_pixels: 160000000
            metadata:
              title: Example workflow
            "#,
        )
        .unwrap();

        assert_eq!(workflow.version, WorkflowVersion::V1);
        assert_eq!(workflow.tasks.len(), 2);
        assert!(matches!(
            workflow.tasks[1].op,
            OperatorSpec::Watermark(WatermarkOptions {
                kind: WatermarkKind::Text,
                ..
            })
        ));
    }

    #[test]
    fn missing_required_workflow_field_fails() {
        let err = serde_json::from_str::<Workflow>(
            r#"
            {
              "version": 1,
              "inputs": [],
              "tasks": [],
              "limits": {},
              "metadata": {}
            }
            "#,
        )
        .unwrap_err();

        assert!(err.to_string().contains("outputs"));
    }

    #[test]
    fn operator_spec_rejects_multiple_operator_keys() {
        let err = serde_json::from_str::<OperatorSpec>(
            r#"
            {
              "rotate": { "pages": "1", "degrees": 90 },
              "split": { "pages": "1" }
            }
            "#,
        )
        .unwrap_err();

        assert!(err
            .to_string()
            .contains("operator spec must contain exactly one operator"));
    }

    #[test]
    fn error_codes_are_stable_machine_readable_values() {
        assert_eq!(
            OxideError::UnsupportedPdfFeature {
                feature: "object stream".to_owned()
            }
            .code(),
            "unsupported_pdf_feature"
        );
        assert_eq!(OxideError::EncryptedPdf.code(), "encrypted_pdf");
        assert_eq!(OxideError::IncorrectPassword.code(), "incorrect_password");
        assert_eq!(OxideError::FontResolution.code(), "font_resolution");
        assert_eq!(
            OxideError::ResourceLimitExceeded {
                limit: "max_pages".to_owned()
            }
            .code(),
            "resource_limit_exceeded"
        );
    }
}
