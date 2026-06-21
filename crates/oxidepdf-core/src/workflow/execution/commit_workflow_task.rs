
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
