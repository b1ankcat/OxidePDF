#![forbid(unsafe_code)]
#![doc = "Core contracts and shared logic for OxidePDF."]

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
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
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TaskId(String);

impl TaskId {
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

    fn workflow_from_json(json: &str) -> Workflow {
        serde_json::from_str(json).unwrap()
    }

    fn artifact_ref(value: &str) -> ArtifactRef {
        serde_json::from_str(&format!("{value:?}")).unwrap()
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
