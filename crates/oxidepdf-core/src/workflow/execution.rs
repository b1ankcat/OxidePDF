use super::runner::OperatorRunner;
use super::types::{
    ArtifactRef, ExecutionPlan, ExecutionResult, ResourceLimits, TaskId, TaskSpec, Workflow,
};
use super::{Artifact, ArtifactStore, TextArtifact, TextExtractionDiagnostic, validate_workflow};
use crate::{OxideError, enforce_input_bytes, resource_limit};
use apalis::prelude::{
    Event, Identity, ParallelizeExt, RandomId, Task, TaskBuilder, WorkerBuilder, WorkerBuilderExt,
    WorkerContext,
};
use futures_util::{StreamExt, stream::BoxStream};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};
use std::time::{Duration, Instant};

/// Executes a workflow through a single apalis in-memory worker.
///
/// The workflow backend only yields tasks whose dependencies are satisfied.
/// Each completed task commits its artifact, releases dependents, and wakes the
/// backend so apalis can immediately schedule newly ready work. This keeps the
/// whole DAG globally concurrent without layer barriers.
pub async fn execute_workflow<R>(
    workflow: &Workflow,
    store: ArtifactStore,
    runner: R,
) -> Result<ExecutionResult, OxideError>
where
    R: OperatorRunner + Clone + Send + Sync + 'static,
{
    let plan = validate_workflow(workflow)?;
    enforce_workflow_input_limits(workflow, &store)?;
    let mut store = store;
    store.rethreshold(workflow.limits.spill_threshold_bytes)?;
    if workflow.tasks.is_empty() {
        return Ok(ExecutionResult { plan, store });
    }
    let started_at = Instant::now();
    let timeout = workflow.limits.timeout_ms.map(Duration::from_millis);
    let store = run_apalis_workflow(workflow, &plan, store, Arc::new(runner), started_at).await?;
    enforce_timeout(started_at, timeout)?;
    Ok(ExecutionResult { plan, store })
}

#[derive(Clone)]
struct WorkflowJob {
    task_index: usize,
}

struct WorkflowState {
    store: ArtifactStore,
    remaining_deps: Vec<usize>,
    dependents: Vec<Vec<usize>>,
    ready: VecDeque<usize>,
    consumer_counts: BTreeMap<ArtifactRef, usize>,
    remaining: usize,
    waker: Option<Waker>,
}

#[derive(Clone)]
struct WorkflowBackend {
    state: Arc<Mutex<WorkflowState>>,
}

async fn run_apalis_workflow<R>(
    workflow: &Workflow,
    plan: &ExecutionPlan,
    store: ArtifactStore,
    runner: Arc<R>,
    started_at: Instant,
) -> Result<ArtifactStore, OxideError>
where
    R: OperatorRunner + Send + Sync + 'static,
{
    let limits = workflow.limits.clone();
    let timeout = limits.timeout_ms.map(Duration::from_millis);
    let max_parallel_tasks = limits.max_parallel_tasks.unwrap_or(workflow.tasks.len());
    let (remaining_deps, dependents) = task_dependencies(workflow);
    let state = Arc::new(Mutex::new(WorkflowState {
        store,
        remaining_deps,
        dependents,
        ready: initial_ready_tasks(plan),
        consumer_counts: artifact_consumer_counts(workflow),
        remaining: workflow.tasks.len(),
        waker: None,
    }));
    let backend = WorkflowBackend {
        state: state.clone(),
    };

    let mut worker = WorkerBuilder::new("oxidepdf-workflow")
        .backend(backend)
        .concurrency(max_parallel_tasks)
        .option_layer(rate_limit_layer(limits.rate_limit_per_second))
        .retry(apalis::layers::retry::RetryPolicy::retries(
            limits.retry_attempts.unwrap_or(0),
        ))
        .option_layer(timeout.map(apalis::layers::TimeoutLayer::new))
        .parallelize(spawn_workflow_future)
        .data(runner)
        .data(state.clone())
        .data(Arc::new(workflow.clone()))
        .data(started_at)
        .data(timeout)
        .build(run_workflow_job::<R>)
        .stream();

    while let Some(event) = worker.next().await {
        let event = match event {
            Ok(event) => event,
            Err(apalis::prelude::WorkerError::GracefulExit) => break,
            Err(_) => return Err(OxideError::Internal),
        };
        match event {
            Event::Error(error) => {
                let err = workflow_event_error(&error);
                // Classify untyped apalis errors (e.g. from TimeoutLayer) as
                // timeout when we are at or past the deadline; preserve typed
                // OxideErrors (ParsePdf, ResourceLimitExceeded, …) as-is.
                if matches!(err, OxideError::Internal)
                    && timeout.is_some_and(|t| started_at.elapsed() >= t)
                {
                    return Err(resource_limit("timeout_ms"));
                }
                return Err(err);
            }
            Event::Success
            | Event::Start
            | Event::Idle
            | Event::HeartBeat
            | Event::Stop
            | Event::Custom(_) => {}
        }
    }

    let mut state = state.lock().map_err(|_| OxideError::Internal)?;
    Ok(std::mem::take(&mut state.store))
}

fn rate_limit_layer(
    rate_limit_per_second: Option<u64>,
) -> Option<apalis::layers::limit::RateLimitLayer> {
    rate_limit_per_second.map(|rate| {
        apalis::layers::limit::RateLimitLayer::new(1, Duration::from_secs_f64(1.0 / rate as f64))
    })
}

fn spawn_workflow_future<F>(future: F) -> tokio::task::JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    // Run on a blocking thread so that futures containing std::thread::sleep
    // (or other blocking calls) don't stall the shared async executor — a
    // single current_thread runtime (e.g. #[tokio::test]) would serialize all
    // tasks if tokio::spawn were used here.
    let handle = tokio::runtime::Handle::current();
    tokio::task::spawn_blocking(move || handle.block_on(future))
}

impl apalis::prelude::Backend for WorkflowBackend {
    type Args = WorkflowJob;
    type IdType = RandomId;
    type Context = apalis::prelude::Extensions;
    type Error = OxideError;
    type Stream = Self;
    type Layer = Identity;
    type Beat = BoxStream<'static, Result<(), Self::Error>>;

    fn heartbeat(&self, _worker: &WorkerContext) -> Self::Beat {
        futures_util::stream::once(async { Ok(()) }).boxed()
    }

    fn middleware(&self) -> Self::Layer {
        Identity::new()
    }

    fn poll(self, _worker: &WorkerContext) -> Self::Stream {
        self
    }
}

impl futures_util::Stream for WorkflowBackend {
    type Item =
        Result<Option<Task<WorkflowJob, apalis::prelude::Extensions, RandomId>>, OxideError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut state = match self.state.lock() {
            Ok(state) => state,
            Err(_) => return Poll::Ready(Some(Err(OxideError::Internal))),
        };
        if let Some(task_index) = state.ready.pop_front() {
            drop(state);
            let task = TaskBuilder::new(WorkflowJob { task_index })
                .with_idempotency_key(task_index.to_string())
                .build();
            return Poll::Ready(Some(Ok(Some(task))));
        }
        if state.remaining == 0 {
            return Poll::Ready(None);
        }
        state.waker = Some(cx.waker().clone());
        Poll::Pending
    }
}

async fn run_workflow_job<R>(
    job: WorkflowJob,
    runner: apalis::prelude::Data<Arc<R>>,
    state: apalis::prelude::Data<Arc<Mutex<WorkflowState>>>,
    workflow: apalis::prelude::Data<Arc<Workflow>>,
    started_at: apalis::prelude::Data<Instant>,
    timeout: apalis::prelude::Data<Option<Duration>>,
    worker: WorkerContext,
) -> Result<(), OxideError>
where
    R: OperatorRunner + Send + Sync + 'static,
{
    enforce_timeout(*started_at, *timeout)?;
    let (task, inputs) = {
        let state = state.lock().map_err(|_| OxideError::Internal)?;
        let task = workflow.tasks[job.task_index].clone();
        let inputs = task
            .inputs
            .iter()
            .map(|input| {
                state.store.get(input).cloned().ok_or_else(|| {
                    invalid_workflow(format!(
                        "artifact '{}' is missing at execution time",
                        input.as_str()
                    ))
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        (task, inputs)
    };
    let artifact = runner.run(task.clone(), inputs).await?;
    enforce_timeout(*started_at, *timeout)?;
    let done = {
        let mut state = state.lock().map_err(|_| OxideError::Internal)?;
        let ready = commit_workflow_task(&workflow, &mut state, job.task_index, task, artifact)?;
        state.ready.extend(ready);
        if let Some(waker) = state.waker.take() {
            waker.wake();
        }
        state.remaining == 0
    };
    if done {
        worker.stop().map_err(|_| OxideError::Internal)?;
    }
    Ok(())
}

fn commit_workflow_task(
    workflow: &Workflow,
    state: &mut WorkflowState,
    task_index: usize,
    task: TaskSpec,
    artifact: Artifact,
) -> Result<Vec<usize>, OxideError> {
    let artifact = artifact.spilled_to_threshold(workflow.limits.spill_threshold_bytes)?;
    state
        .store
        .insert(ArtifactRef::new(task.id.as_str()), artifact);
    evict_consumed_artifacts(workflow, state, &task);
    state.remaining -= 1;

    let mut ready = Vec::new();
    for &dependent in &state.dependents[task_index] {
        state.remaining_deps[dependent] -= 1;
        if state.remaining_deps[dependent] == 0 {
            ready.push(dependent);
        }
    }
    Ok(ready)
}

fn initial_ready_tasks(plan: &ExecutionPlan) -> VecDeque<usize> {
    plan.layers.first().into_iter().flatten().copied().collect()
}

fn artifact_consumer_counts(workflow: &Workflow) -> BTreeMap<ArtifactRef, usize> {
    let mut counts = BTreeMap::new();
    for task in &workflow.tasks {
        for input in &task.inputs {
            *counts.entry(input.clone()).or_default() += 1;
        }
    }
    counts
}

fn task_dependencies(workflow: &Workflow) -> (Vec<usize>, Vec<Vec<usize>>) {
    let task_ids = workflow
        .tasks
        .iter()
        .enumerate()
        .map(|(index, task)| (task.id.clone(), index))
        .collect::<BTreeMap<_, _>>();
    let mut remaining_deps = vec![0usize; workflow.tasks.len()];
    let mut dependents = vec![Vec::new(); workflow.tasks.len()];
    for (index, task) in workflow.tasks.iter().enumerate() {
        for input in &task.inputs {
            let dependency = TaskId::new(input.as_str());
            if let Some(&dependency_index) = task_ids.get(&dependency) {
                remaining_deps[index] += 1;
                dependents[dependency_index].push(index);
            }
        }
    }
    (remaining_deps, dependents)
}

fn workflow_event_error(error: &apalis::prelude::BoxDynError) -> OxideError {
    if let Some(error) = error.downcast_ref::<OxideError>() {
        return error.clone();
    }
    OxideError::Internal
}

fn evict_consumed_artifacts(workflow: &Workflow, state: &mut WorkflowState, task: &TaskSpec) {
    for input in &task.inputs {
        let Some(count) = state.consumer_counts.get_mut(input) else {
            continue;
        };
        *count -= 1;
        if *count == 0 && !workflow.outputs.iter().any(|output| output.from == *input) {
            state.store.remove(input);
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

pub(super) fn collect_ids(workflow: &Workflow) -> Result<BTreeSet<ArtifactRef>, OxideError> {
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

pub(super) fn validate_task_references(
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

pub(super) fn validate_output_references(
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
pub(super) fn build_execution_graph(
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

pub(super) fn check_resource_limit_entrypoint(limits: &ResourceLimits) -> Result<(), OxideError> {
    let numeric_limits = [
        limits.max_input_bytes,
        limits.max_total_input_bytes,
        limits.max_pixels,
        limits.max_output_bytes,
        limits.timeout_ms,
        limits.retry_attempts.map(|value| value as u64),
        limits.rate_limit_per_second,
        limits.max_tasks.map(|value| value as u64),
        limits.max_parallel_tasks.map(|value| value as u64),
    ];

    if numeric_limits.into_iter().flatten().any(|limit| limit == 0) || limits.max_pages == Some(0) {
        return Err(OxideError::ResourceLimitExceeded {
            limit: "resource limit must be greater than zero".to_owned(),
        });
    }

    Ok(())
}

pub(super) fn enforce_task_count_limit(
    task_count: usize,
    limits: &ResourceLimits,
) -> Result<(), OxideError> {
    if limits.max_tasks.is_some_and(|limit| task_count > limit) {
        return Err(resource_limit("max_tasks"));
    }

    Ok(())
}

pub(super) fn enforce_artifact_output_bytes(
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

pub(super) fn artifact_size(artifact: &Artifact) -> usize {
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
    if timeout.is_some_and(|timeout| started_at.elapsed() >= timeout) {
        return Err(resource_limit("timeout_ms"));
    }

    Ok(())
}

pub(super) fn invalid_workflow(reason: impl Into<String>) -> OxideError {
    OxideError::InvalidWorkflow {
        reason: reason.into(),
    }
}
