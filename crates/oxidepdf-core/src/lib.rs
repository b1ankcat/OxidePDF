#![forbid(unsafe_code)]
#![doc = "Core contracts and shared logic for OxidePDF."]

use pdf_writer::Finish;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::PathBuf;
use thiserror::Error;

const A4_WIDTH: f32 = 595.0;
const A4_HEIGHT: f32 = 842.0;

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
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TaskId(String);

impl TaskId {
    /// Creates a task identifier.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the underlying identifier.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Input, task, or output artifact reference.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ArtifactRef(String);

impl ArtifactRef {
    /// Creates an artifact reference.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

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
#[derive(Debug, Clone, Error, PartialEq, Eq)]
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

/// Artifact produced or consumed by workflow tasks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Artifact {
    /// PDF artifact placeholder.
    Pdf(PdfArtifact),
    /// Image artifact placeholder.
    Image(ImageArtifact),
    /// Text artifact.
    Text(TextArtifact),
    /// SVG artifact.
    Svg(SvgArtifact),
    /// Raw bytes.
    Bytes(BytesArtifact),
}

impl Artifact {
    /// Creates a raw byte artifact.
    pub fn bytes(bytes: impl AsRef<[u8]>) -> Self {
        Self::Bytes(BytesArtifact {
            bytes: bytes.as_ref().to_vec(),
        })
    }

    /// Creates a PDF artifact.
    pub fn pdf(bytes: impl AsRef<[u8]>) -> Self {
        Self::Pdf(PdfArtifact {
            bytes: bytes.as_ref().to_vec(),
        })
    }

    /// Creates an image artifact.
    pub fn image(bytes: impl AsRef<[u8]>) -> Self {
        Self::Image(ImageArtifact {
            bytes: bytes.as_ref().to_vec(),
        })
    }

    /// Creates an SVG artifact.
    pub fn svg(bytes: impl AsRef<[u8]>) -> Self {
        Self::Svg(SvgArtifact {
            bytes: bytes.as_ref().to_vec(),
        })
    }
}

/// PDF artifact placeholder for later operators.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdfArtifact {
    /// Serialized bytes until object-level artifacts are added.
    pub bytes: Vec<u8>,
}

/// Image artifact placeholder for later operators.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageArtifact {
    /// Encoded image bytes.
    pub bytes: Vec<u8>,
}

/// Text artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextArtifact {
    /// Extracted or generated text.
    pub text: String,
}

/// SVG artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SvgArtifact {
    /// SVG document bytes.
    pub bytes: Vec<u8>,
}

/// Byte artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BytesArtifact {
    /// Raw bytes.
    pub bytes: Vec<u8>,
}

/// In-memory artifact store used by the serial executor.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ArtifactStore {
    artifacts: BTreeMap<ArtifactRef, Artifact>,
}

impl ArtifactStore {
    /// Creates an empty store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts or replaces an artifact.
    pub fn insert(&mut self, id: ArtifactRef, artifact: Artifact) -> Option<Artifact> {
        self.artifacts.insert(id, artifact)
    }

    /// Returns an artifact by id.
    pub fn get(&self, id: &ArtifactRef) -> Option<&Artifact> {
        self.artifacts.get(id)
    }
}

/// Validated workflow execution plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionPlan {
    /// Task ids in topological execution order.
    pub task_order: Vec<TaskId>,
}

/// Result of a successful workflow execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionResult {
    /// Validated execution plan used for this run.
    pub plan: ExecutionPlan,
    /// Artifact store containing inputs and task outputs.
    pub store: ArtifactStore,
}

/// Operator implementation boundary used by the serial executor.
pub trait OperatorRunner {
    /// Runs a task against resolved input artifacts.
    fn run(&mut self, task: &TaskSpec, inputs: &[Artifact]) -> Result<Artifact, OxideError>;
}

/// Operator runner for object-level PDF page editing.
#[derive(Debug, Clone, Default)]
pub struct PdfOperatorRunner {
    limits: ResourceLimits,
}

impl PdfOperatorRunner {
    /// Creates a runner using explicit workflow resource limits.
    pub fn with_limits(limits: ResourceLimits) -> Self {
        Self { limits }
    }
}

impl OperatorRunner for PdfOperatorRunner {
    fn run(&mut self, task: &TaskSpec, inputs: &[Artifact]) -> Result<Artifact, OxideError> {
        match &task.op {
            OperatorSpec::Merge(_) => merge_pdf_artifacts(inputs).map(Artifact::Pdf),
            OperatorSpec::Split(options) => {
                let input = single_pdf_input(inputs)?;
                split_pdf(input, &options.pages).map(Artifact::Pdf)
            }
            OperatorSpec::Reorder(options) => {
                let input = single_pdf_input(inputs)?;
                reorder_pdf(input, &options.pages).map(Artifact::Pdf)
            }
            OperatorSpec::Rotate(options) => {
                let input = single_pdf_input(inputs)?;
                rotate_pdf(input, &options.pages, options.degrees).map(Artifact::Pdf)
            }
            OperatorSpec::ImageToPdf(options) => {
                image_artifacts_to_pdf(inputs, options, &self.limits).map(Artifact::Pdf)
            }
            OperatorSpec::SvgToPdf(options) => {
                let input = single_svg_input(inputs)?;
                svg_to_pdf(input, options, &self.limits).map(Artifact::Pdf)
            }
            other => Err(OxideError::UnsupportedPdfFeature {
                feature: format!("{other:?}"),
            }),
        }
    }
}

/// Merges multiple PDF artifacts into a single PDF.
pub fn merge_pdf_artifacts(inputs: &[Artifact]) -> Result<PdfArtifact, OxideError> {
    if inputs.len() < 2 {
        return Err(OxideError::InvalidInput {
            reason: "merge requires at least two PDF inputs".to_owned(),
        });
    }

    let mut documents = Vec::with_capacity(inputs.len());
    for input in inputs {
        documents.push(load_pdf(pdf_bytes(input)?)?);
    }

    merge_documents(documents).map(|bytes| PdfArtifact { bytes })
}

/// Splits a PDF by keeping the specified one-based pages.
pub fn split_pdf(input: &[u8], pages: &str) -> Result<PdfArtifact, OxideError> {
    let mut document = load_pdf(input)?;
    let selected_pages = parse_page_range(pages, document.get_pages().len() as u32)?;
    keep_pages(&mut document, &selected_pages)?;
    save_pdf(document).map(|bytes| PdfArtifact { bytes })
}

/// Reorders a PDF by an explicit one-based page sequence.
pub fn reorder_pdf(input: &[u8], pages: &str) -> Result<PdfArtifact, OxideError> {
    let mut document = load_pdf(input)?;
    let selected_pages = parse_page_range(pages, document.get_pages().len() as u32)?;
    keep_pages(&mut document, &selected_pages)?;
    save_pdf(document).map(|bytes| PdfArtifact { bytes })
}

/// Rotates selected PDF pages by 90, 180, or 270 degrees.
pub fn rotate_pdf(input: &[u8], pages: &str, degrees: i16) -> Result<PdfArtifact, OxideError> {
    let mut document = load_pdf(input)?;
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

    save_pdf(document).map(|bytes| PdfArtifact { bytes })
}

/// Converts image artifacts into a PDF with one image per page.
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
        enforce_input_bytes(bytes.len(), limits)?;
        let decoded = decode_image(bytes)?;
        let pixels = u64::from(decoded.width) * u64::from(decoded.height);
        total_pixels = total_pixels
            .checked_add(pixels)
            .ok_or_else(|| resource_limit("max_pixels"))?;
        enforce_max_pixels(total_pixels, limits)?;
        images.push(decoded);
    }

    let layout = ImageLayout::from_options(options)?;
    write_images_pdf(&images, layout).map(|bytes| PdfArtifact { bytes })
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

    Ok(PdfArtifact { bytes })
}

/// Validates a workflow and returns a topological execution plan.
pub fn validate_workflow(workflow: &Workflow) -> Result<ExecutionPlan, OxideError> {
    check_resource_limit_entrypoint(&workflow.limits)?;
    let ids = collect_ids(workflow)?;
    validate_task_references(workflow, &ids)?;
    validate_output_references(workflow, &ids)?;
    let task_order = topological_sort(workflow)?;

    Ok(ExecutionPlan { task_order })
}

/// Executes a workflow serially.
pub fn execute_workflow(
    workflow: &Workflow,
    mut store: ArtifactStore,
    runner: &mut impl OperatorRunner,
) -> Result<ExecutionResult, OxideError> {
    let plan = validate_workflow(workflow)?;
    let tasks_by_id = workflow
        .tasks
        .iter()
        .map(|task| (task.id.clone(), task))
        .collect::<BTreeMap<_, _>>();

    for task_id in &plan.task_order {
        let task = tasks_by_id.get(task_id).ok_or_else(|| {
            invalid_workflow(format!(
                "task '{}' disappeared after validation",
                task_id.as_str()
            ))
        })?;
        let inputs = task
            .inputs
            .iter()
            .map(|input| {
                store.get(input).cloned().ok_or_else(|| {
                    invalid_workflow(format!(
                        "artifact '{}' is missing at execution time",
                        input.as_str()
                    ))
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let artifact = runner.run(task, &inputs)?;
        store.insert(ArtifactRef(task.id.as_str().to_owned()), artifact);
    }

    Ok(ExecutionResult { plan, store })
}

fn collect_ids(workflow: &Workflow) -> Result<BTreeSet<ArtifactRef>, OxideError> {
    let mut ids = BTreeSet::new();
    for input in &workflow.inputs {
        insert_unique_id(&mut ids, &input.id)?;
    }
    for task in &workflow.tasks {
        insert_unique_id(&mut ids, &ArtifactRef(task.id.as_str().to_owned()))?;
    }
    for output in &workflow.outputs {
        insert_unique_id(&mut ids, &output.id)?;
    }

    Ok(ids)
}

fn insert_unique_id(ids: &mut BTreeSet<ArtifactRef>, id: &ArtifactRef) -> Result<(), OxideError> {
    if id.as_str().is_empty() {
        return Err(invalid_workflow("artifact id must not be empty"));
    }
    if !ids.insert(id.clone()) {
        return Err(invalid_workflow(format!(
            "duplicate artifact id '{}'",
            id.as_str()
        )));
    }

    Ok(())
}

fn validate_task_references(
    workflow: &Workflow,
    ids: &BTreeSet<ArtifactRef>,
) -> Result<(), OxideError> {
    for task in &workflow.tasks {
        if task.inputs.is_empty() {
            return Err(invalid_workflow(format!(
                "task '{}' must declare at least one input",
                task.id.as_str()
            )));
        }
        for input in &task.inputs {
            if !ids.contains(input) {
                return Err(invalid_workflow(format!(
                    "task '{}' references missing artifact '{}'",
                    task.id.as_str(),
                    input.as_str()
                )));
            }
        }
    }

    Ok(())
}

fn validate_output_references(
    workflow: &Workflow,
    ids: &BTreeSet<ArtifactRef>,
) -> Result<(), OxideError> {
    for output in &workflow.outputs {
        if !ids.contains(&output.from) {
            return Err(invalid_workflow(format!(
                "output '{}' references missing artifact '{}'",
                output.id.as_str(),
                output.from.as_str()
            )));
        }
    }

    Ok(())
}

fn topological_sort(workflow: &Workflow) -> Result<Vec<TaskId>, OxideError> {
    let task_ids = workflow
        .tasks
        .iter()
        .map(|task| task.id.clone())
        .collect::<BTreeSet<_>>();
    let mut incoming_count = workflow
        .tasks
        .iter()
        .map(|task| (task.id.clone(), 0usize))
        .collect::<BTreeMap<_, _>>();
    let mut dependents = BTreeMap::<TaskId, Vec<TaskId>>::new();

    for task in &workflow.tasks {
        for input in &task.inputs {
            let dependency = TaskId(input.as_str().to_owned());
            if task_ids.contains(&dependency) {
                *incoming_count.get_mut(&task.id).ok_or_else(|| {
                    invalid_workflow(format!("task '{}' is missing", task.id.as_str()))
                })? += 1;
                dependents
                    .entry(dependency)
                    .or_default()
                    .push(task.id.clone());
            }
        }
    }

    let mut ready = incoming_count
        .iter()
        .filter_map(|(id, count)| (*count == 0).then_some(id.clone()))
        .collect::<VecDeque<_>>();
    let mut task_order = Vec::with_capacity(workflow.tasks.len());

    while let Some(task_id) = ready.pop_front() {
        task_order.push(task_id.clone());
        if let Some(children) = dependents.get(&task_id) {
            for child in children {
                let child_count = incoming_count.get_mut(child).ok_or_else(|| {
                    invalid_workflow(format!("task '{}' is missing", child.as_str()))
                })?;
                *child_count -= 1;
                if *child_count == 0 {
                    ready.push_back(child.clone());
                }
            }
        }
    }

    if task_order.len() != workflow.tasks.len() {
        return Err(invalid_workflow("workflow task graph contains a cycle"));
    }

    Ok(task_order)
}

fn check_resource_limit_entrypoint(limits: &ResourceLimits) -> Result<(), OxideError> {
    let numeric_limits = [
        limits.max_input_bytes,
        limits.max_total_input_bytes,
        limits.max_pixels,
        limits.max_output_bytes,
        limits.timeout_ms,
    ];

    if numeric_limits.into_iter().flatten().any(|limit| limit == 0) || limits.max_pages == Some(0) {
        return Err(OxideError::ResourceLimitExceeded {
            limit: "resource limit must be greater than zero".to_owned(),
        });
    }

    Ok(())
}

fn single_pdf_input(inputs: &[Artifact]) -> Result<&[u8], OxideError> {
    if inputs.len() != 1 {
        return Err(OxideError::InvalidInput {
            reason: "operator requires exactly one PDF input".to_owned(),
        });
    }

    pdf_bytes(&inputs[0])
}

fn single_svg_input(inputs: &[Artifact]) -> Result<&[u8], OxideError> {
    if inputs.len() != 1 {
        return Err(OxideError::InvalidInput {
            reason: "svg2pdf requires exactly one SVG input".to_owned(),
        });
    }

    match &inputs[0] {
        Artifact::Svg(svg) => Ok(&svg.bytes),
        Artifact::Bytes(bytes) => Ok(&bytes.bytes),
        _ => Err(OxideError::InvalidInput {
            reason: "expected SVG input artifact".to_owned(),
        }),
    }
}

fn pdf_bytes(artifact: &Artifact) -> Result<&[u8], OxideError> {
    match artifact {
        Artifact::Pdf(pdf) => Ok(&pdf.bytes),
        Artifact::Bytes(bytes) => Ok(&bytes.bytes),
        _ => Err(OxideError::InvalidInput {
            reason: "expected PDF input artifact".to_owned(),
        }),
    }
}

fn image_bytes(artifact: &Artifact) -> Result<&[u8], OxideError> {
    match artifact {
        Artifact::Image(image) => Ok(&image.bytes),
        Artifact::Bytes(bytes) => Ok(&bytes.bytes),
        _ => Err(OxideError::InvalidInput {
            reason: "expected image input artifact".to_owned(),
        }),
    }
}

fn load_pdf(input: &[u8]) -> Result<lopdf::Document, OxideError> {
    let document = lopdf::Document::load_mem(input).map_err(map_lopdf_read_error)?;
    if document.is_encrypted() {
        return Err(OxideError::EncryptedPdf);
    }
    if document.get_pages().is_empty() {
        return Err(OxideError::InvalidInput {
            reason: "PDF contains no pages".to_owned(),
        });
    }

    Ok(document)
}

fn save_pdf(mut document: lopdf::Document) -> Result<Vec<u8>, OxideError> {
    let mut output = Vec::new();
    document.prune_objects();
    document.renumber_objects();
    document
        .save_to(&mut output)
        .map_err(|_| OxideError::WritePdf)?;

    Ok(output)
}

fn map_lopdf_read_error(error: lopdf::Error) -> OxideError {
    match error {
        lopdf::Error::Decryption(_) | lopdf::Error::UnsupportedSecurityHandler(_) => {
            OxideError::EncryptedPdf
        }
        _ => OxideError::ParsePdf,
    }
}

fn parse_page_range(pages: &str, page_count: u32) -> Result<Vec<u32>, OxideError> {
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

#[derive(Debug, Clone)]
struct DecodedImage {
    width: u32,
    height: u32,
    rgb: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ImageLayout {
    Fit,
    OriginalSize,
}

impl ImageLayout {
    fn from_options(options: &ImageToPdfOptions) -> Result<Self, OxideError> {
        match options.layout.as_deref().unwrap_or("fit") {
            "fit" => Ok(Self::Fit),
            "original_size" => Ok(Self::OriginalSize),
            other => Err(OxideError::InvalidInput {
                reason: format!("unsupported image layout '{other}'"),
            }),
        }
    }
}

fn decode_image(input: &[u8]) -> Result<DecodedImage, OxideError> {
    let format = image::guess_format(input).map_err(|_| OxideError::ImageDecode)?;
    match format {
        image::ImageFormat::Jpeg | image::ImageFormat::Png | image::ImageFormat::WebP => {}
        _ => return Err(OxideError::ImageDecode),
    }
    let image = image::load_from_memory_with_format(input, format)
        .map_err(|_| OxideError::ImageDecode)?
        .to_rgb8();

    Ok(DecodedImage {
        width: image.width(),
        height: image.height(),
        rgb: image.into_raw(),
    })
}

fn write_images_pdf(images: &[DecodedImage], layout: ImageLayout) -> Result<Vec<u8>, OxideError> {
    let mut next_ref = 1;
    let mut alloc_ref = || {
        let reference = pdf_writer::Ref::new(next_ref);
        next_ref += 1;
        reference
    };
    let catalog_id = alloc_ref();
    let pages_id = alloc_ref();
    let page_ids = (0..images.len()).map(|_| alloc_ref()).collect::<Vec<_>>();
    let image_ids = (0..images.len()).map(|_| alloc_ref()).collect::<Vec<_>>();
    let content_ids = (0..images.len()).map(|_| alloc_ref()).collect::<Vec<_>>();

    let mut pdf = pdf_writer::Pdf::new();
    pdf.catalog(catalog_id).pages(pages_id);
    pdf.pages(pages_id)
        .kids(page_ids.iter().copied())
        .count(images.len() as i32);

    for (((page_id, image_id), content_id), image) in page_ids
        .iter()
        .zip(image_ids.iter())
        .zip(content_ids.iter())
        .zip(images.iter())
    {
        let image_name = pdf_writer::Name(b"Im1");
        let (page_width, page_height, image_width, image_height, x, y) =
            image_placement(image, layout);

        let mut page = pdf.page(*page_id);
        page.media_box(pdf_writer::Rect::new(0.0, 0.0, page_width, page_height));
        page.parent(pages_id);
        page.contents(*content_id);
        page.resources().x_objects().pair(image_name, *image_id);
        page.finish();

        let mut image_object = pdf.image_xobject(*image_id, &image.rgb);
        image_object.width(image.width as i32);
        image_object.height(image.height as i32);
        image_object.color_space().device_rgb();
        image_object.bits_per_component(8);
        image_object.finish();

        let mut content = pdf_writer::Content::new();
        content.save_state();
        content.transform([image_width, 0.0, 0.0, image_height, x, y]);
        content.x_object(image_name);
        content.restore_state();
        pdf.stream(*content_id, &content.finish());
    }

    Ok(pdf.finish())
}

fn image_placement(image: &DecodedImage, layout: ImageLayout) -> (f32, f32, f32, f32, f32, f32) {
    let original_width = image.width as f32;
    let original_height = image.height as f32;
    match layout {
        ImageLayout::OriginalSize => (
            original_width,
            original_height,
            original_width,
            original_height,
            0.0,
            0.0,
        ),
        ImageLayout::Fit => {
            let scale = (A4_WIDTH / original_width)
                .min(A4_HEIGHT / original_height)
                .min(1.0);
            let image_width = original_width * scale;
            let image_height = original_height * scale;
            let x = (A4_WIDTH - image_width) / 2.0;
            let y = (A4_HEIGHT - image_height) / 2.0;
            (A4_WIDTH, A4_HEIGHT, image_width, image_height, x, y)
        }
    }
}

fn parse_svg(input: &[u8]) -> Result<svg2pdf::usvg::Tree, OxideError> {
    let options = svg2pdf::usvg::Options::default();
    svg2pdf::usvg::Tree::from_data(input, &options).map_err(|_| OxideError::SvgParse)
}

fn svg_pixel_count(tree: &svg2pdf::usvg::Tree) -> Result<u64, OxideError> {
    let size = tree.size().to_int_size();
    Ok(u64::from(size.width()) * u64::from(size.height()))
}

fn rasterize_svg(tree: &svg2pdf::usvg::Tree) -> Result<DecodedImage, OxideError> {
    let size = tree.size().to_int_size();
    let mut pixmap =
        resvg::tiny_skia::Pixmap::new(size.width(), size.height()).ok_or(OxideError::RenderPdf)?;
    resvg::render(
        tree,
        resvg::tiny_skia::Transform::identity(),
        &mut pixmap.as_mut(),
    );
    let mut rgb =
        Vec::with_capacity((u64::from(size.width()) * u64::from(size.height()) * 3) as usize);
    for pixel in pixmap.data().chunks_exact(4) {
        rgb.extend_from_slice(&[pixel[0], pixel[1], pixel[2]]);
    }

    Ok(DecodedImage {
        width: size.width(),
        height: size.height(),
        rgb,
    })
}

fn enforce_input_bytes(size: usize, limits: &ResourceLimits) -> Result<(), OxideError> {
    if let Some(limit) = limits.max_input_bytes {
        if size as u64 > limit {
            return Err(resource_limit("max_input_bytes"));
        }
    }

    Ok(())
}

fn enforce_max_pages(pages: usize, limits: &ResourceLimits) -> Result<(), OxideError> {
    if let Some(limit) = limits.max_pages {
        if pages as u32 > limit {
            return Err(resource_limit("max_pages"));
        }
    }

    Ok(())
}

fn enforce_max_pixels(pixels: u64, limits: &ResourceLimits) -> Result<(), OxideError> {
    if let Some(limit) = limits.max_pixels {
        if pixels > limit {
            return Err(resource_limit("max_pixels"));
        }
    }

    Ok(())
}

fn resource_limit(limit: impl Into<String>) -> OxideError {
    OxideError::ResourceLimitExceeded {
        limit: limit.into(),
    }
}

fn rebuild_pages_tree(
    document: &mut lopdf::Document,
    page_ids: &[lopdf::ObjectId],
) -> Result<(), OxideError> {
    let catalog = document.catalog().map_err(|_| OxideError::ParsePdf)?;
    let pages_id = catalog
        .get(b"Pages")
        .and_then(lopdf::Object::as_reference)
        .map_err(|_| OxideError::ParsePdf)?;
    {
        let pages_dictionary = document
            .get_object_mut(pages_id)
            .and_then(lopdf::Object::as_dict_mut)
            .map_err(|_| OxideError::ParsePdf)?;
        pages_dictionary.set("Count", page_ids.len() as u32);
        pages_dictionary.set(
            "Kids",
            page_ids
                .iter()
                .copied()
                .map(lopdf::Object::Reference)
                .collect::<Vec<_>>(),
        );
    }
    for page_id in page_ids {
        let page_dictionary = document
            .get_object_mut(*page_id)
            .and_then(lopdf::Object::as_dict_mut)
            .map_err(|_| OxideError::ParsePdf)?;
        page_dictionary.set("Parent", pages_id);
    }

    Ok(())
}

fn invalid_workflow(reason: impl Into<String>) -> OxideError {
    OxideError::InvalidWorkflow {
        reason: reason.into(),
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

    #[test]
    fn linear_workflow_executes_tasks_in_dependency_order() {
        let workflow = workflow_from_json(
            r#"
            {
              "version": 1,
              "inputs": [{ "id": "source", "path": "input.pdf" }],
              "tasks": [
                {
                  "id": "rotate",
                  "op": { "rotate": { "pages": "1", "degrees": 90 } },
                  "inputs": ["source"]
                },
                {
                  "id": "render",
                  "op": { "render": { "page": 1, "format": "png", "scale": 1.0 } },
                  "inputs": ["rotate"]
                }
              ],
              "outputs": [{ "id": "final", "from": "render", "path": "out.png" }]
            }
            "#,
        );
        let mut store = ArtifactStore::new();
        store.insert(artifact_ref("source"), Artifact::bytes(b"input"));
        let mut runner = RecordingRunner::default();

        let result = execute_workflow(&workflow, store, &mut runner).unwrap();

        assert_eq!(runner.executed, ["rotate", "render"]);
        assert_eq!(
            result.store.get(&artifact_ref("render")),
            Some(&Artifact::bytes(b"render"))
        );
        assert_eq!(result.plan.task_order[0].as_str(), "rotate");
        assert_eq!(result.plan.task_order[1].as_str(), "render");
    }

    #[test]
    fn dag_workflow_topologically_sorts_before_execution() {
        let workflow = workflow_from_json(
            r#"
            {
              "version": 1,
              "inputs": [{ "id": "source", "path": "input.pdf" }],
              "tasks": [
                {
                  "id": "left",
                  "op": { "rotate": { "pages": "1", "degrees": 90 } },
                  "inputs": ["source"]
                },
                {
                  "id": "right",
                  "op": { "rotate": { "pages": "1", "degrees": 180 } },
                  "inputs": ["source"]
                },
                {
                  "id": "join",
                  "op": { "merge": {} },
                  "inputs": ["left", "right"]
                }
              ],
              "outputs": [{ "id": "final", "from": "join", "path": "out.pdf" }]
            }
            "#,
        );

        let plan = validate_workflow(&workflow).unwrap();
        let left = plan
            .task_order
            .iter()
            .position(|id| id.as_str() == "left")
            .unwrap();
        let right = plan
            .task_order
            .iter()
            .position(|id| id.as_str() == "right")
            .unwrap();
        let join = plan
            .task_order
            .iter()
            .position(|id| id.as_str() == "join")
            .unwrap();

        assert!(left < join);
        assert!(right < join);
    }

    #[test]
    fn cyclic_workflow_fails_validation() {
        let workflow = workflow_from_json(
            r#"
            {
              "version": 1,
              "inputs": [],
              "tasks": [
                {
                  "id": "a",
                  "op": { "merge": {} },
                  "inputs": ["b"]
                },
                {
                  "id": "b",
                  "op": { "merge": {} },
                  "inputs": ["a"]
                }
              ],
              "outputs": [{ "id": "final", "from": "b", "path": "out.pdf" }]
            }
            "#,
        );

        let err = validate_workflow(&workflow).unwrap_err();

        assert!(matches!(err, OxideError::InvalidWorkflow { .. }));
        assert!(err.to_string().contains("cycle"));
    }

    #[test]
    fn missing_artifact_reference_fails_validation() {
        let workflow = workflow_from_json(
            r#"
            {
              "version": 1,
              "inputs": [{ "id": "source", "path": "input.pdf" }],
              "tasks": [
                {
                  "id": "rotate",
                  "op": { "rotate": { "pages": "1", "degrees": 90 } },
                  "inputs": ["missing"]
                }
              ],
              "outputs": [{ "id": "final", "from": "rotate", "path": "out.pdf" }]
            }
            "#,
        );

        let err = validate_workflow(&workflow).unwrap_err();

        assert!(matches!(err, OxideError::InvalidWorkflow { .. }));
        assert!(err.to_string().contains("missing"));
    }

    #[test]
    fn duplicate_artifact_identifiers_fail_validation() {
        let workflow = workflow_from_json(
            r#"
            {
              "version": 1,
              "inputs": [{ "id": "source", "path": "input.pdf" }],
              "tasks": [
                {
                  "id": "source",
                  "op": { "rotate": { "pages": "1", "degrees": 90 } },
                  "inputs": ["source"]
                }
              ],
              "outputs": [{ "id": "final", "from": "source", "path": "out.pdf" }]
            }
            "#,
        );

        let err = validate_workflow(&workflow).unwrap_err();

        assert!(matches!(err, OxideError::InvalidWorkflow { .. }));
        assert!(err.to_string().contains("duplicate"));
    }

    #[test]
    fn task_failure_stops_downstream_execution() {
        let workflow = workflow_from_json(
            r#"
            {
              "version": 1,
              "inputs": [{ "id": "source", "path": "input.pdf" }],
              "tasks": [
                {
                  "id": "fail",
                  "op": { "rotate": { "pages": "1", "degrees": 90 } },
                  "inputs": ["source"]
                },
                {
                  "id": "after",
                  "op": { "render": { "page": 1, "format": "png", "scale": 1.0 } },
                  "inputs": ["fail"]
                }
              ],
              "outputs": [{ "id": "final", "from": "after", "path": "out.png" }]
            }
            "#,
        );
        let mut store = ArtifactStore::new();
        store.insert(artifact_ref("source"), Artifact::bytes(b"input"));
        let expected = OxideError::InvalidInput {
            reason: "runner failed".to_owned(),
        };
        let mut runner = RecordingRunner {
            fail_on: Some("fail"),
            error: Some(expected.clone()),
            ..RecordingRunner::default()
        };

        let err = execute_workflow(&workflow, store, &mut runner).unwrap_err();

        assert_eq!(err, expected);
        assert_eq!(runner.executed, ["fail"]);
    }

    #[test]
    fn merge_pdf_artifacts_combines_pages() {
        let pdf = include_bytes!("../../../tests/test.pdf");

        let merged = merge_pdf_artifacts(&[Artifact::pdf(pdf), Artifact::pdf(pdf)]).unwrap();
        let document = lopdf::Document::load_mem(&merged.bytes).unwrap();

        assert_eq!(document.get_pages().len(), 6);
    }

    #[test]
    fn split_pdf_keeps_only_selected_pages() {
        let pdf = include_bytes!("../../../tests/test.pdf");

        let split = split_pdf(pdf, "2-3").unwrap();
        let document = lopdf::Document::load_mem(&split.bytes).unwrap();

        assert_eq!(document.get_pages().len(), 2);
        assert_page_numbers(&document, &[1, 2]);
    }

    #[test]
    fn reorder_pdf_rearranges_pages() {
        let pdf = include_bytes!("../../../tests/test.pdf");

        let reordered = reorder_pdf(pdf, "3,1,2").unwrap();
        let document = lopdf::Document::load_mem(&reordered.bytes).unwrap();

        assert_eq!(document.get_pages().len(), 3);
        assert_page_numbers(&document, &[1, 2, 3]);
    }

    #[test]
    fn rotate_pdf_updates_page_rotation() {
        let pdf = include_bytes!("../../../tests/test.pdf");

        let rotated = rotate_pdf(pdf, "1-2", 90).unwrap();
        let document = lopdf::Document::load_mem(&rotated.bytes).unwrap();

        assert_eq!(page_rotation(&document, 1), 90);
        assert_eq!(page_rotation(&document, 2), 90);
        assert_eq!(page_rotation(&document, 3), 0);
    }

    #[test]
    fn pdf_operator_runner_handles_page_editing_tasks() {
        let pdf = include_bytes!("../../../tests/test.pdf");
        let mut runner = PdfOperatorRunner::default();

        let merged = runner
            .run(
                &TaskSpec {
                    id: TaskId::new("merge"),
                    op: OperatorSpec::Merge(MergeOptions {}),
                    inputs: vec![artifact_ref("a"), artifact_ref("b")],
                },
                &[Artifact::pdf(pdf), Artifact::pdf(pdf)],
            )
            .unwrap();

        assert!(matches!(merged, Artifact::Pdf(_)));
    }

    #[test]
    fn image_artifacts_to_pdf_converts_real_jpeg() {
        let image = include_bytes!("../../../tests/test.jpg");

        let pdf = image_artifacts_to_pdf(
            &[Artifact::image(image)],
            &ImageToPdfOptions::default(),
            &ResourceLimits::default(),
        )
        .unwrap();
        let document = lopdf::Document::load_mem(&pdf.bytes).unwrap();

        assert_eq!(document.get_pages().len(), 1);
    }

    #[test]
    fn image_artifacts_to_pdf_writes_one_page_per_image() {
        let image = include_bytes!("../../../tests/test.jpg");

        let pdf = image_artifacts_to_pdf(
            &[Artifact::image(image), Artifact::image(image)],
            &ImageToPdfOptions::default(),
            &ResourceLimits::default(),
        )
        .unwrap();
        let document = lopdf::Document::load_mem(&pdf.bytes).unwrap();

        assert_eq!(document.get_pages().len(), 2);
    }

    #[test]
    fn image_artifacts_to_pdf_enforces_pixel_limit() {
        let image = include_bytes!("../../../tests/test.jpg");
        let limits = ResourceLimits {
            max_pixels: Some(1),
            ..ResourceLimits::default()
        };

        let err = image_artifacts_to_pdf(
            &[Artifact::image(image)],
            &ImageToPdfOptions::default(),
            &limits,
        )
        .unwrap_err();

        assert_eq!(
            err,
            OxideError::ResourceLimitExceeded {
                limit: "max_pixels".to_owned()
            }
        );
    }

    #[test]
    fn image_artifacts_to_pdf_rejects_unknown_image_format() {
        let err = image_artifacts_to_pdf(
            &[Artifact::image(b"not an image")],
            &ImageToPdfOptions::default(),
            &ResourceLimits::default(),
        )
        .unwrap_err();

        assert_eq!(err, OxideError::ImageDecode);
    }

    #[test]
    fn svg_to_pdf_converts_vector_svg_to_parseable_pdf() {
        let svg = br##"<svg xmlns="http://www.w3.org/2000/svg" width="120" height="80">
            <rect x="10" y="10" width="100" height="60" fill="#0077cc"/>
        </svg>"##;

        let pdf = svg_to_pdf(svg, &SvgToPdfOptions::default(), &ResourceLimits::default()).unwrap();
        let document = lopdf::Document::load_mem(&pdf.bytes).unwrap();

        assert_eq!(document.get_pages().len(), 1);
    }

    #[test]
    fn svg_to_pdf_rasterizes_only_when_requested() {
        let svg = br##"<svg xmlns="http://www.w3.org/2000/svg" width="120" height="80">
            <circle cx="60" cy="40" r="30" fill="#ef4444"/>
        </svg>"##;

        let pdf = svg_to_pdf(
            svg,
            &SvgToPdfOptions { rasterize: true },
            &ResourceLimits::default(),
        )
        .unwrap();
        let document = lopdf::Document::load_mem(&pdf.bytes).unwrap();

        assert_eq!(document.get_pages().len(), 1);
    }

    #[test]
    fn svg_to_pdf_rejects_invalid_svg() {
        let err = svg_to_pdf(
            b"<svg><broken>",
            &SvgToPdfOptions::default(),
            &ResourceLimits::default(),
        )
        .unwrap_err();

        assert_eq!(err, OxideError::SvgParse);
    }

    fn workflow_from_json(json: &str) -> Workflow {
        serde_json::from_str(json).unwrap()
    }

    fn artifact_ref(value: &str) -> ArtifactRef {
        serde_json::from_str(&format!("{value:?}")).unwrap()
    }

    fn assert_page_numbers(document: &lopdf::Document, expected: &[u32]) {
        let pages = document.get_pages();
        let actual = pages.keys().copied().collect::<Vec<_>>();
        assert_eq!(actual, expected);
    }

    fn page_rotation(document: &lopdf::Document, page_number: u32) -> i64 {
        let page_id = document.get_pages().get(&page_number).copied().unwrap();
        let page = document.get_object(page_id).unwrap().as_dict().unwrap();
        page.get(b"Rotate")
            .and_then(lopdf::Object::as_i64)
            .unwrap_or(0)
    }

    #[derive(Default)]
    struct RecordingRunner {
        executed: Vec<String>,
        fail_on: Option<&'static str>,
        error: Option<OxideError>,
    }

    impl OperatorRunner for RecordingRunner {
        fn run(&mut self, task: &TaskSpec, _inputs: &[Artifact]) -> Result<Artifact, OxideError> {
            self.executed.push(task.id.as_str().to_owned());
            if self.fail_on == Some(task.id.as_str()) {
                return Err(self.error.take().unwrap());
            }

            Ok(Artifact::bytes(task.id.as_str().as_bytes()))
        }
    }
}
