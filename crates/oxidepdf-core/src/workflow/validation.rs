use super::execution::{
    build_execution_graph, check_resource_limit_entrypoint, collect_ids, enforce_task_count_limit,
    invalid_workflow, validate_output_references, validate_task_references,
};
use super::types::{ExecutionPlan, Workflow};
use crate::OxideError;
use std::collections::{BTreeMap, BTreeSet};

/// Validates a workflow and returns a topological execution plan.
pub fn validate_workflow(workflow: &Workflow) -> Result<ExecutionPlan, OxideError> {
    check_resource_limit_entrypoint(&workflow.limits)?;
    enforce_task_count_limit(workflow.tasks.len(), &workflow.limits)?;
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
