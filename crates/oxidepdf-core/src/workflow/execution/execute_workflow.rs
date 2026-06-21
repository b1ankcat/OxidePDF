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
