use crate::operators::{
    run_pdf_compare, run_pdf_edit, run_pdf_inspect, run_pdf_security, run_pdf_sign,
};
use crate::{
    enforce_input_bytes, resource_limit, save_pdf, OxideError, PdfCompareOptions, PdfEditOptions,
    PdfInspectOptions, PdfSecurityOptions, PdfSignOptions,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use rayon::prelude::*;

/// Current workflow schema version.
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
    /// Payloads larger than this are spilled to a memory-mapped temp file
    /// instead of the heap. `None` keeps every payload inline. Lets deployments
    /// tune the heap/spill tradeoff (e.g. a small CI container vs. a workstation).
    pub spill_threshold_bytes: Option<u64>,
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
            spill_threshold_bytes: Some(DEFAULT_SPILL_THRESHOLD_BYTES as u64),
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
    /// Edit or create PDF artifacts.
    PdfEdit(PdfEditOptions),
    /// Inspect or render PDF artifacts.
    PdfInspect(PdfInspectOptions),
    /// Apply password, encryption, or permission operations.
    PdfSecurity(PdfSecurityOptions),
    /// Compare two PDF artifacts.
    PdfCompare(PdfCompareOptions),
    /// Sign or verify PDF signature material.
    PdfSign(PdfSignOptions),
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct OperatorSpecDef {
    pdf_edit: Option<PdfEditOptions>,
    pdf_inspect: Option<PdfInspectOptions>,
    pdf_security: Option<PdfSecurityOptions>,
    pdf_compare: Option<PdfCompareOptions>,
    pdf_sign: Option<PdfSignOptions>,
}

impl TryFrom<OperatorSpecDef> for OperatorSpec {
    type Error = OxideError;

    fn try_from(value: OperatorSpecDef) -> Result<Self, Self::Error> {
        let operator_count = [
            value.pdf_edit.is_some(),
            value.pdf_inspect.is_some(),
            value.pdf_security.is_some(),
            value.pdf_compare.is_some(),
            value.pdf_sign.is_some(),
        ]
        .into_iter()
        .filter(|present| *present)
        .count();

        if operator_count != 1 {
            return Err(OxideError::InvalidWorkflow {
                reason: "operator spec must contain exactly one operator".to_owned(),
            });
        }

        if let Some(options) = value.pdf_edit {
            return Ok(Self::PdfEdit(options));
        }
        if let Some(options) = value.pdf_inspect {
            return Ok(Self::PdfInspect(options));
        }
        if let Some(options) = value.pdf_security {
            return Ok(Self::PdfSecurity(options));
        }
        if let Some(options) = value.pdf_compare {
            return Ok(Self::PdfCompare(options));
        }
        if let Some(options) = value.pdf_sign {
            return Ok(Self::PdfSign(options));
        }

        unreachable!("operator count was already checked");
    }
}

impl From<OperatorSpec> for OperatorSpecDef {
    fn from(value: OperatorSpec) -> Self {
        match value {
            OperatorSpec::PdfEdit(options) => Self {
                pdf_edit: Some(options),
                ..Self::default()
            },
            OperatorSpec::PdfInspect(options) => Self {
                pdf_inspect: Some(options),
                ..Self::default()
            },
            OperatorSpec::PdfSecurity(options) => Self {
                pdf_security: Some(options),
                ..Self::default()
            },
            OperatorSpec::PdfCompare(options) => Self {
                pdf_compare: Some(options),
                ..Self::default()
            },
            OperatorSpec::PdfSign(options) => Self {
                pdf_sign: Some(options),
                ..Self::default()
            },
        }
    }
}

/// Artifact produced or consumed by workflow tasks.
///
/// `Eq` is intentionally not derived: the `PdfObject` variant wraps an
/// `Arc<lopdf::Document>`, which implements neither `PartialEq` nor `Eq`.
/// Equality is provided by a hand-written `PartialEq` that compares parsed
/// documents by shared identity (`Arc::ptr_eq`).
#[derive(Debug, Clone)]
pub enum Artifact {
    /// PDF artifact placeholder.
    Pdf(PdfArtifact),
    /// Parsed PDF object tree, shared across chained operators without
    /// re-serializing between steps.
    PdfObject(PdfObjectArtifact),
    /// Image artifact placeholder.
    Image(ImageArtifact),
    /// Text artifact.
    Text(TextArtifact),
    /// SVG artifact.
    Svg(SvgArtifact),
    /// Raw bytes.
    Bytes(BytesArtifact),
}

impl PartialEq for Artifact {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Pdf(left), Self::Pdf(right)) => left == right,
            // Parsed documents are not structurally comparable; two object
            // artifacts are equal only when they share the same backing tree.
            (Self::PdfObject(left), Self::PdfObject(right)) => {
                Arc::ptr_eq(&left.document, &right.document)
            }
            (Self::Image(left), Self::Image(right)) => left == right,
            (Self::Text(left), Self::Text(right)) => left == right,
            (Self::Svg(left), Self::Svg(right)) => left == right,
            (Self::Bytes(left), Self::Bytes(right)) => left == right,
            _ => false,
        }
    }
}

impl Artifact {
    /// Creates a raw byte artifact.
    pub fn bytes(bytes: impl AsRef<[u8]>) -> Self {
        Self::Bytes(BytesArtifact {
            bytes: ArtifactBytes::from(bytes.as_ref()),
        })
    }

    /// Creates a PDF artifact.
    pub fn pdf(bytes: impl AsRef<[u8]>) -> Self {
        Self::Pdf(PdfArtifact {
            bytes: ArtifactBytes::from(bytes.as_ref()),
        })
    }

    /// Creates a parsed-PDF object artifact from an owned document.
    pub fn pdf_object(document: lopdf::Document) -> Self {
        Self::PdfObject(PdfObjectArtifact {
            document: Arc::new(document),
        })
    }

    /// Returns the artifact's bytes for materialization at an output boundary.
    ///
    /// Byte-backed artifacts borrow their buffer; a parsed object tree is
    /// serialized here (the single serialization point for the object-level
    /// path). Cloning the document out of the shared `Arc` is required because
    /// serialization renumbers and prunes objects, which needs ownership.
    pub fn output_bytes(&self) -> Result<std::borrow::Cow<'_, [u8]>, OxideError> {
        match self {
            Self::Pdf(pdf) => Ok(std::borrow::Cow::Borrowed(pdf.bytes.as_slice())),
            Self::Image(image) => Ok(std::borrow::Cow::Borrowed(image.bytes.as_slice())),
            Self::Svg(svg) => Ok(std::borrow::Cow::Borrowed(svg.bytes.as_slice())),
            Self::Bytes(bytes) => Ok(std::borrow::Cow::Borrowed(bytes.bytes.as_slice())),
            Self::Text(text) => Ok(std::borrow::Cow::Borrowed(text.text.as_bytes())),
            Self::PdfObject(artifact) => {
                let document = (*artifact.document).clone();
                Ok(std::borrow::Cow::Owned(save_pdf(document)?))
            }
        }
    }

    /// Creates an image artifact.
    pub fn image(bytes: impl AsRef<[u8]>) -> Self {
        Self::Image(ImageArtifact {
            bytes: ArtifactBytes::from(bytes.as_ref()),
        })
    }

    /// Creates an SVG artifact.
    pub fn svg(bytes: impl AsRef<[u8]>) -> Self {
        Self::Svg(SvgArtifact {
            bytes: ArtifactBytes::from(bytes.as_ref()),
        })
    }

    /// Applies a workflow's configured spill threshold to this artifact's
    /// payload. Byte-backed artifacts produced during execution are built with
    /// the default threshold; this re-evaluates them against the workflow's
    /// threshold. Text and parsed-object artifacts carry no `ArtifactBytes` and
    /// pass through.
    fn spilled_to_threshold(self, threshold: Option<u64>) -> Self {
        match self {
            Self::Pdf(artifact) => Self::Pdf(PdfArtifact {
                bytes: artifact.bytes.spilled_to_threshold(threshold),
            }),
            Self::Image(artifact) => Self::Image(ImageArtifact {
                bytes: artifact.bytes.spilled_to_threshold(threshold),
            }),
            Self::Svg(artifact) => Self::Svg(SvgArtifact {
                bytes: artifact.bytes.spilled_to_threshold(threshold),
            }),
            Self::Bytes(artifact) => Self::Bytes(BytesArtifact {
                bytes: artifact.bytes.spilled_to_threshold(threshold),
            }),
            // The parsed object tree is already an in-memory structure, not a
            // byte buffer, so spill thresholds do not apply.
            Self::PdfObject(artifact) => Self::PdfObject(artifact),
            Self::Text(artifact) => Self::Text(artifact),
        }
    }
}

/// Default spill threshold when `ResourceLimits::spill_threshold_bytes` is left
/// at its default. Artifacts larger than this are spilled to a memory-mapped
/// temp file instead of the heap. 64 MiB keeps typical PDFs inline while capping
/// the resident heap footprint of very large inputs and outputs.
const DEFAULT_SPILL_THRESHOLD_BYTES: usize = 64 * 1024 * 1024;

/// Reference-counted artifact payload.
///
/// Cloning an `ArtifactBytes` only bumps an atomic reference count; no bytes are
/// copied. This lets the executor hand the same large PDF to several tasks, and
/// keep inputs in the store, without duplicating multi-hundred-megabyte buffers.
///
/// Payloads above the spill threshold are spilled to a memory-mapped temp
/// file so they do not occupy heap; smaller payloads stay inline. Both forms
/// expose the same `&[u8]` view, so consumers never observe the difference.
#[derive(Clone)]
pub struct ArtifactBytes {
    storage: Storage,
}

#[derive(Clone)]
enum Storage {
    /// Heap-resident payload.
    Inline(Arc<[u8]>),
    /// Payload backed by a memory-mapped temporary file.
    Mapped(Arc<MappedTemp>),
}

/// A temp file plus its read-only memory mapping. The file is kept alive
/// alongside the mapping and removed when the last reference is dropped.
///
/// The stored mapping is read-only so `ArtifactBytes` stays `Sync` and can be
/// shared across the parallel executor; the payload is written once through a
/// transient writable mapping during [`spill_to_mmap`].
struct MappedTemp {
    // Held so the backing file outlives the mapping and is cleaned up on drop.
    _file: tempfile::NamedTempFile,
    mmap: tiverse_mmap::Mmap<tiverse_mmap::ReadOnly>,
}

impl MappedTemp {
    fn as_slice(&self) -> &[u8] {
        &self.mmap
    }
}

impl ArtifactBytes {
    /// Returns the payload length in bytes.
    pub fn len(&self) -> usize {
        self.as_slice().len()
    }

    /// Returns whether the payload is empty.
    pub fn is_empty(&self) -> bool {
        self.as_slice().is_empty()
    }

    /// Returns the payload as a byte slice.
    pub fn as_slice(&self) -> &[u8] {
        match &self.storage {
            Storage::Inline(bytes) => bytes,
            Storage::Mapped(mapped) => mapped.as_slice(),
        }
    }

    /// Returns whether the payload is currently spilled to a temp file.
    /// Primarily intended for tests and diagnostics.
    pub fn is_spilled(&self) -> bool {
        matches!(self.storage, Storage::Mapped(_))
    }

    /// Builds a payload from owned bytes using the default spill threshold,
    /// spilling to a memory-mapped temp file when the size exceeds it. Spilling
    /// is best-effort: if a temp file cannot be created or mapped, the payload
    /// stays inline.
    fn from_vec(bytes: Vec<u8>) -> Self {
        Self::from_vec_with_threshold(bytes, Some(DEFAULT_SPILL_THRESHOLD_BYTES as u64))
    }

    /// Builds a payload from owned bytes, spilling to a memory-mapped temp file
    /// when the size exceeds `threshold`. A `threshold` of `None` keeps the
    /// payload inline regardless of size. Spilling is best-effort: if a temp
    /// file cannot be created or mapped, the payload stays inline.
    fn from_vec_with_threshold(bytes: Vec<u8>, threshold: Option<u64>) -> Self {
        if threshold.is_some_and(|threshold| bytes.len() as u64 > threshold) {
            if let Some(mapped) = spill_to_mmap(&bytes) {
                return Self {
                    storage: Storage::Mapped(Arc::new(mapped)),
                };
            }
        }
        Self {
            storage: Storage::Inline(Arc::from(bytes.into_boxed_slice())),
        }
    }

    /// Builds a payload from an existing `Arc<[u8]>` without copying or spilling.
    ///
    /// Use this when the caller already owns a reference-counted buffer (for
    /// example one shared from another `ArtifactBytes` or a custom source): the
    /// `Arc` is adopted directly, so no allocation or copy occurs. Because the
    /// buffer is already heap-resident and shared, it is kept inline rather than
    /// spilled — spilling an existing `Arc` would copy the very bytes this path
    /// exists to avoid copying.
    pub fn from_arc(bytes: Arc<[u8]>) -> Self {
        Self {
            storage: Storage::Inline(bytes),
        }
    }

    /// Reconciles the payload's storage with `threshold`: spills an oversized
    /// inline payload to a memory-mapped temp file, and pulls a spilled payload
    /// back inline when it now fits within the threshold. This is where a
    /// workflow's configured spill threshold is applied to artifacts produced
    /// during execution, which operators build with the default threshold — so
    /// raising or lowering the threshold both take effect. A `threshold` of
    /// `None` keeps everything inline.
    fn spilled_to_threshold(self, threshold: Option<u64>) -> Self {
        let over_threshold = |len: usize| threshold.is_some_and(|threshold| len as u64 > threshold);
        match &self.storage {
            // Inline but now over the threshold: spill it. Best-effort — a spill
            // failure leaves it inline (a placement choice, not a behavior).
            Storage::Inline(bytes) if over_threshold(bytes.len()) => match spill_to_mmap(bytes) {
                Some(mapped) => Self {
                    storage: Storage::Mapped(Arc::new(mapped)),
                },
                None => self,
            },
            // Spilled but now within the threshold: pull it back to the heap.
            Storage::Mapped(mapped) if !over_threshold(mapped.as_slice().len()) => Self {
                storage: Storage::Inline(Arc::from(mapped.as_slice().to_vec().into_boxed_slice())),
            },
            _ => self,
        }
    }
}

/// Spills `bytes` to a temp file, writing through a single memory mapping.
///
/// The file is pre-sized with `set_len`, mapped `MAP_SHARED` read-write, and the
/// payload is copied straight into the mapped region — so the bytes are placed
/// once, in the pages backing the file, instead of being staged in the kernel
/// write cache by a `write()` and then mapped back in. The writable mapping is
/// then dropped and the populated file is re-mapped read-only for storage, which
/// keeps `ArtifactBytes` `Sync` (a read-write mapping is not) so it can be shared
/// across the parallel executor.
///
/// Returns `None` on any IO or mapping failure; this is a memory-placement
/// optimization, not a behavioral path, so the caller keeps the payload inline
/// instead.
fn spill_to_mmap(bytes: &[u8]) -> Option<MappedTemp> {
    let file = tempfile::NamedTempFile::new().ok()?;
    file.as_file().set_len(bytes.len() as u64).ok()?;
    {
        let mut writable = tiverse_mmap::MmapOptions::new()
            .path(file.path())
            .shared()
            .map_readwrite()
            .ok()?;
        writable.as_mut_slice().copy_from_slice(bytes);
    }
    let mmap = tiverse_mmap::MmapOptions::new()
        .path(file.path())
        .map_readonly()
        .ok()?;
    Some(MappedTemp { _file: file, mmap })
}

impl std::fmt::Debug for ArtifactBytes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ArtifactBytes")
            .field("len", &self.len())
            .field("spilled", &self.is_spilled())
            .finish()
    }
}

impl PartialEq for ArtifactBytes {
    fn eq(&self, other: &Self) -> bool {
        self.as_slice() == other.as_slice()
    }
}

impl Eq for ArtifactBytes {}

impl std::ops::Deref for ArtifactBytes {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl AsRef<[u8]> for ArtifactBytes {
    fn as_ref(&self) -> &[u8] {
        self.as_slice()
    }
}

impl From<Vec<u8>> for ArtifactBytes {
    fn from(bytes: Vec<u8>) -> Self {
        Self::from_vec(bytes)
    }
}

impl From<&[u8]> for ArtifactBytes {
    fn from(bytes: &[u8]) -> Self {
        Self::from_vec(bytes.to_vec())
    }
}

impl From<Arc<[u8]>> for ArtifactBytes {
    fn from(bytes: Arc<[u8]>) -> Self {
        Self::from_arc(bytes)
    }
}

/// PDF artifact placeholder for later operators.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdfArtifact {
    /// Serialized PDF bytes. Produced by operators that have not yet migrated to
    /// the object-level path, and at the workflow output boundary.
    pub bytes: ArtifactBytes,
}

/// Parsed PDF object tree shared across chained operators.
///
/// Holding the document as an `Arc<lopdf::Document>` lets several tasks read the
/// same parsed tree without re-parsing, and lets a chain of object-level
/// operators pass the document along without serializing to bytes and parsing
/// again between every step. Serialization happens once, at the output boundary.
#[derive(Debug, Clone)]
pub struct PdfObjectArtifact {
    /// The shared, parsed PDF document.
    pub document: Arc<lopdf::Document>,
}

/// Image artifact placeholder for later operators.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageArtifact {
    /// Encoded image bytes.
    pub bytes: ArtifactBytes,
}

/// Text artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextArtifact {
    /// Extracted or generated text.
    pub text: String,
    /// Page-level extraction diagnostics reserved for structured output.
    pub diagnostics: Vec<TextExtractionDiagnostic>,
}

/// Page-level diagnostic emitted by text extraction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextExtractionDiagnostic {
    /// One-based page number.
    pub page: u32,
    /// Stable diagnostic code.
    pub code: TextExtractionDiagnosticCode,
    /// Non-sensitive diagnostic message.
    pub message: String,
}

/// Stable text extraction diagnostic code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextExtractionDiagnosticCode {
    /// Page has no extractable text layer.
    NoTextLayer,
}

/// SVG artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SvgArtifact {
    /// SVG document bytes.
    pub bytes: ArtifactBytes,
}

/// Byte artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BytesArtifact {
    /// Raw bytes.
    pub bytes: ArtifactBytes,
}

/// In-memory artifact store used by the executor.
///
/// Lookups are keyed by `ArtifactRef` with no ordering requirement, so a
/// `HashMap` gives O(1) amortized insert/get/remove.
///
/// `Eq` is not derived because `Artifact` is only `PartialEq` (its `PdfObject`
/// variant wraps a non-`Eq` parsed document).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ArtifactStore {
    artifacts: HashMap<ArtifactRef, Artifact>,
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

    /// Removes an artifact, returning it if present.
    ///
    /// The executor calls this to evict an artifact once its last consumer has
    /// run and no output references it, keeping peak memory close to the working
    /// set rather than the full set of every artifact ever produced.
    pub fn remove(&mut self, id: &ArtifactRef) -> Option<Artifact> {
        self.artifacts.remove(id)
    }
}

/// Validated workflow execution plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionPlan {
    /// Task ids in topological execution order.
    pub task_order: Vec<TaskId>,
    /// Dependency layers as task indices into `workflow.tasks`. Layer 0 holds
    /// tasks with no task dependency; each later layer depends only on earlier
    /// ones. Tasks within a layer are mutually independent and run concurrently.
    /// Built once during validation so execution does not re-walk the graph.
    pub layers: Vec<Vec<usize>>,
    /// Index of each task id into `workflow.tasks`, precomputed during
    /// validation so execution does not rebuild a lookup map on every run.
    pub task_index: BTreeMap<TaskId, usize>,
    /// For each artifact, the last task in `task_order` that consumes it as an
    /// input. After that task runs, the artifact can be evicted unless an output
    /// references it.
    pub last_consumer: BTreeMap<ArtifactRef, TaskId>,
    /// Artifacts referenced by an output spec; these are never evicted.
    pub output_refs: BTreeSet<ArtifactRef>,
}

/// Result of a successful workflow execution.
///
/// `Eq` is not derived because `ArtifactStore` is only `PartialEq` (artifacts
/// may hold a non-`Eq` parsed document).
#[derive(Debug, Clone, PartialEq)]
pub struct ExecutionResult {
    /// Validated execution plan used for this run.
    pub plan: ExecutionPlan,
    /// Artifact store containing inputs and task outputs.
    pub store: ArtifactStore,
}

/// Operator implementation boundary used by the executor.
///
/// `run` takes `&self` and the trait requires `Sync` so the executor can invoke
/// it concurrently across the tasks of a single dependency layer.
pub trait OperatorRunner: Sync {
    /// Runs a task against resolved input artifacts.
    fn run(&self, task: &TaskSpec, inputs: &[Artifact]) -> Result<Artifact, OxideError>;
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
    fn run(&self, task: &TaskSpec, inputs: &[Artifact]) -> Result<Artifact, OxideError> {
        let artifact = match &task.op {
            OperatorSpec::PdfEdit(options) => run_pdf_edit(options, inputs, &self.limits),
            OperatorSpec::PdfInspect(options) => run_pdf_inspect(options, inputs, &self.limits),
            OperatorSpec::PdfSecurity(options) => run_pdf_security(options, inputs, &self.limits),
            OperatorSpec::PdfCompare(options) => run_pdf_compare(options, inputs, &self.limits),
            OperatorSpec::PdfSign(options) => run_pdf_sign(options, inputs, &self.limits),
        }?;
        enforce_artifact_output_bytes(&artifact, &self.limits)?;
        Ok(artifact)
    }
}

/// Validates a workflow and returns a topological execution plan.
pub fn validate_workflow(workflow: &Workflow) -> Result<ExecutionPlan, OxideError> {
    check_resource_limit_entrypoint(&workflow.limits)?;
    let ids = collect_ids(workflow)?;
    validate_task_references(workflow, &ids)?;
    validate_output_references(workflow, &ids)?;
    let (task_order, layers) = build_execution_graph(workflow)?;

    let task_index = workflow
        .tasks
        .iter()
        .enumerate()
        .map(|(index, task)| (task.id.clone(), index))
        .collect::<BTreeMap<_, _>>();

    let output_refs = workflow
        .outputs
        .iter()
        .map(|output| output.from.clone())
        .collect::<BTreeSet<_>>();

    // Walk tasks in execution order so the last write wins: the final entry for
    // each artifact names the task after which it is safe to evict.
    let mut last_consumer = BTreeMap::new();
    for task_id in &task_order {
        let index = task_index
            .get(task_id)
            .copied()
            .ok_or_else(|| invalid_workflow(format!("task '{}' is missing", task_id.as_str())))?;
        for input in &workflow.tasks[index].inputs {
            last_consumer.insert(input.clone(), task_id.clone());
        }
    }

    Ok(ExecutionPlan {
        task_order,
        layers,
        task_index,
        last_consumer,
        output_refs,
    })
}

/// Executes a workflow, running independent tasks of each dependency layer in
/// parallel.
///
/// Tasks are grouped into layers by dependency depth. Within a layer every task
/// is independent, so they run concurrently on a rayon thread pool, each reading
/// the shared store immutably and cloning its inputs (an `Arc` refcount bump).
/// The layer acts as a barrier: once all its tasks finish, results are written
/// back to the store serially and consumed artifacts are evicted. This keeps the
/// store lock-free — parallel tasks never mutate it.
pub fn execute_workflow(
    workflow: &Workflow,
    mut store: ArtifactStore,
    runner: &impl OperatorRunner,
) -> Result<ExecutionResult, OxideError> {
    let plan = validate_workflow(workflow)?;
    enforce_workflow_input_limits(workflow, &store)?;
    let started_at = Instant::now();
    let timeout = workflow.limits.timeout_ms.map(Duration::from_millis);

    for layer in &plan.layers {
        enforce_timeout(started_at, timeout)?;

        // Resolve every task's inputs against the read-only store first, so the
        // parallel section borrows nothing mutable.
        let resolved = layer
            .iter()
            .map(|&index| {
                let task = &workflow.tasks[index];
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
                Ok((task, inputs))
            })
            .collect::<Result<Vec<_>, OxideError>>()?;

        // Run the layer's tasks concurrently; the first error short-circuits.
        let outputs = resolved
            .par_iter()
            .map(|(task, inputs)| runner.run(task, inputs).map(|artifact| (*task, artifact)))
            .collect::<Result<Vec<_>, OxideError>>()?;

        enforce_timeout(started_at, timeout)?;

        // Barrier passed: commit results and evict consumed artifacts serially.
        // Re-evaluate each produced payload against the workflow's spill
        // threshold (operators build artifacts with the default threshold).
        let spill_threshold = workflow.limits.spill_threshold_bytes;
        for (task, artifact) in outputs {
            store.insert(
                ArtifactRef::new(task.id.as_str()),
                artifact.spilled_to_threshold(spill_threshold),
            );
        }
        for &index in layer {
            evict_consumed_artifacts(&mut store, &plan, &workflow.tasks[index]);
        }
    }

    Ok(ExecutionResult { plan, store })
}

/// Evicts any input artifact whose last consumer is the task that just ran,
/// unless an output references it. This bounds peak memory to the live working
/// set instead of accumulating every artifact for the whole run.
fn evict_consumed_artifacts(store: &mut ArtifactStore, plan: &ExecutionPlan, task: &TaskSpec) {
    for input in &task.inputs {
        if plan.output_refs.contains(input) {
            continue;
        }
        if plan.last_consumer.get(input) == Some(&task.id) {
            store.remove(input);
        }
    }
}

fn enforce_workflow_input_limits(
    workflow: &Workflow,
    store: &ArtifactStore,
) -> Result<(), OxideError> {
    let mut total_input_bytes = 0u64;
    for input in &workflow.inputs {
        let artifact = store.get(&input.id).ok_or_else(|| {
            invalid_workflow(format!(
                "input artifact '{}' is missing at execution time",
                input.id.as_str()
            ))
        })?;
        let size = artifact_size(artifact);
        enforce_input_bytes(size, &workflow.limits)?;
        total_input_bytes = total_input_bytes
            .checked_add(size as u64)
            .ok_or_else(|| resource_limit("max_total_input_bytes"))?;
        if workflow
            .limits
            .max_total_input_bytes
            .is_some_and(|limit| total_input_bytes > limit)
        {
            return Err(resource_limit("max_total_input_bytes"));
        }
    }

    Ok(())
}

fn collect_ids(workflow: &Workflow) -> Result<BTreeSet<ArtifactRef>, OxideError> {
    let mut ids = BTreeSet::new();
    for input in &workflow.inputs {
        insert_unique_id(&mut ids, &input.id)?;
    }
    for task in &workflow.tasks {
        insert_unique_id(&mut ids, &ArtifactRef::new(task.id.as_str()))?;
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

/// Builds the dependency graph once, returning both the topological task order
/// and the parallel execution layers.
///
/// A single Kahn pass advances layer by layer: each round's ready set (tasks
/// with no remaining task dependency) forms one layer, and flattening the layers
/// in order yields a valid topological order. Tasks within a layer are mutually
/// independent and may run concurrently. A graph that does not emit every task
/// contains a cycle and is rejected.
fn build_execution_graph(
    workflow: &Workflow,
) -> Result<(Vec<TaskId>, Vec<Vec<usize>>), OxideError> {
    let task_ids = workflow
        .tasks
        .iter()
        .map(|task| task.id.clone())
        .collect::<BTreeSet<_>>();

    // Per task: how many inputs are produced by other tasks, and which task
    // indices depend on it.
    let mut remaining_deps = vec![0usize; workflow.tasks.len()];
    let mut dependents: BTreeMap<TaskId, Vec<usize>> = BTreeMap::new();
    for (index, task) in workflow.tasks.iter().enumerate() {
        for input in &task.inputs {
            let dependency = TaskId::new(input.as_str());
            if task_ids.contains(&dependency) {
                remaining_deps[index] += 1;
                dependents.entry(dependency).or_default().push(index);
            }
        }
    }

    let mut current = remaining_deps
        .iter()
        .enumerate()
        .filter_map(|(index, count)| (*count == 0).then_some(index))
        .collect::<Vec<_>>();

    let mut layers = Vec::new();
    let mut task_order = Vec::with_capacity(workflow.tasks.len());
    while !current.is_empty() {
        let mut next = Vec::new();
        for &index in &current {
            task_order.push(workflow.tasks[index].id.clone());
            if let Some(children) = dependents.get(&workflow.tasks[index].id) {
                for &child in children {
                    remaining_deps[child] -= 1;
                    if remaining_deps[child] == 0 {
                        next.push(child);
                    }
                }
            }
        }
        layers.push(current);
        current = next;
    }

    if task_order.len() != workflow.tasks.len() {
        return Err(invalid_workflow("workflow task graph contains a cycle"));
    }

    Ok((task_order, layers))
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

fn enforce_artifact_output_bytes(
    artifact: &Artifact,
    limits: &ResourceLimits,
) -> Result<(), OxideError> {
    // A parsed object tree has no byte length until serialized. When a
    // max_output_bytes limit is in force, serialize to measure the true size so
    // the limit still bounds object-level outputs; when no limit is set, skip
    // the serialization entirely and keep the chain parse/serialize-free.
    if let Artifact::PdfObject(_) = artifact {
        if limits.max_output_bytes.is_some() {
            let size = artifact.output_bytes()?.len();
            return crate::enforce_output_bytes(size, limits);
        }
        return Ok(());
    }
    crate::enforce_output_bytes(artifact_size(artifact), limits)
}

pub(crate) fn artifact_size(artifact: &Artifact) -> usize {
    match artifact {
        Artifact::Pdf(pdf) => pdf.bytes.len(),
        // A parsed object tree has no serialized byte length until it is
        // written. Intermediate object artifacts are not subject to output-byte
        // limits; the precise check runs at the output boundary after
        // serialization (see the CLI output path).
        Artifact::PdfObject(_) => 0,
        Artifact::Image(image) => image.bytes.len(),
        Artifact::Text(text) => text_artifact_size(text),
        Artifact::Svg(svg) => svg.bytes.len(),
        Artifact::Bytes(bytes) => bytes.bytes.len(),
    }
}

/// Estimates the in-memory footprint of a text artifact, including the
/// page-level diagnostics that `text.text.len()` alone omits. Undercounting
/// here would let a diagnostics-heavy artifact slip past `max_output_bytes`.
fn text_artifact_size(text: &TextArtifact) -> usize {
    let diagnostics_size = text
        .diagnostics
        .iter()
        .map(|diagnostic| {
            std::mem::size_of::<TextExtractionDiagnostic>() + diagnostic.message.len()
        })
        .sum::<usize>();
    text.text.len() + diagnostics_size
}

fn enforce_timeout(started_at: Instant, timeout: Option<Duration>) -> Result<(), OxideError> {
    if timeout.is_some_and(|timeout| started_at.elapsed() > timeout) {
        return Err(resource_limit("timeout_ms"));
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
    fn text_artifact_size_includes_diagnostics() {
        let text = TextArtifact {
            text: "abc".to_owned(),
            diagnostics: vec![TextExtractionDiagnostic {
                page: 1,
                code: TextExtractionDiagnosticCode::NoTextLayer,
                message: "no text layer".to_owned(),
            }],
        };

        // Size must account for the diagnostics, not just the text body, so the
        // resource-limit check cannot be bypassed with diagnostics-heavy output.
        assert!(artifact_size(&Artifact::Text(text)) > "abc".len());
    }
}
