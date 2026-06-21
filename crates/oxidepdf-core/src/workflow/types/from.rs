
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
