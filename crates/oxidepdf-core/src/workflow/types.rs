use super::ArtifactStore;
use super::artifact::DEFAULT_SPILL_THRESHOLD_BYTES;
use crate::{
    OxideError, PdfCompareOptions, PdfEditOptions, PdfInspectOptions, PdfSecurityOptions,
    PdfSignOptions,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

pub const WORKFLOW_SCHEMA_VERSION: u16 = 1;
/// Default maximum number of tasks accepted by a workflow.
pub const DEFAULT_MAX_WORKFLOW_TASKS: usize = 256;
/// Default maximum number of workflow tasks that may run concurrently.
pub const DEFAULT_MAX_PARALLEL_TASKS: usize = 8;

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
    /// Number of retries for a failed task. `None` disables retries.
    pub retry_attempts: Option<usize>,
    /// Maximum number of tasks started per second. `None` disables rate
    /// limiting.
    pub rate_limit_per_second: Option<u64>,
    /// Maximum number of tasks accepted by a workflow.
    pub max_tasks: Option<usize>,
    /// Maximum number of workflow tasks running at once.
    pub max_parallel_tasks: Option<usize>,
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
            retry_attempts: None,
            rate_limit_per_second: None,
            max_tasks: Some(DEFAULT_MAX_WORKFLOW_TASKS),
            max_parallel_tasks: Some(DEFAULT_MAX_PARALLEL_TASKS),
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
