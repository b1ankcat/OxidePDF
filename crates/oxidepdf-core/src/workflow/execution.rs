use super::runner::OperatorRunner;
use super::types::{
    ArtifactRef, ExecutionPlan, ExecutionResult, ResourceLimits, TaskId, TaskSpec, Workflow,
};
use super::{Artifact, ArtifactStore, TextArtifact, TextExtractionDiagnostic, validate_workflow};
use crate::{OxideError, enforce_input_bytes, resource_limit};
use rayon::prelude::*;
use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration, Instant};

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
            .map(|(task, inputs)| {
                enforce_timeout(started_at, timeout)?;
                runner.run(task, inputs).map(|artifact| (*task, artifact))
            })
            .collect::<Result<Vec<_>, OxideError>>()?;

        enforce_timeout(started_at, timeout)?;

        // Barrier passed: commit results and evict consumed artifacts serially.
        // Re-evaluate each produced payload against the workflow's spill
        // threshold (operators build artifacts with the default threshold).
        let spill_threshold = workflow.limits.spill_threshold_bytes;
        for (task, artifact) in outputs {
            store.insert(
                ArtifactRef::new(task.id.as_str()),
                artifact.spilled_to_threshold(spill_threshold)?,
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
    ];

    if numeric_limits.into_iter().flatten().any(|limit| limit == 0) || limits.max_pages == Some(0) {
        return Err(OxideError::ResourceLimitExceeded {
            limit: "resource limit must be greater than zero".to_owned(),
        });
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
    if timeout.is_some_and(|timeout| started_at.elapsed() > timeout) {
        return Err(resource_limit("timeout_ms"));
    }

    Ok(())
}

pub(super) fn invalid_workflow(reason: impl Into<String>) -> OxideError {
    OxideError::InvalidWorkflow {
        reason: reason.into(),
    }
}
