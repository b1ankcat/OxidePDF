use super::CliError;
use oxidepdf_core::{
    Artifact, ArtifactRef, ArtifactStore, OperatorSpec, OxideError, PdfOperatorRunner,
    ResourceLimits, TaskId, TaskSpec, Workflow, WorkflowMetadata, WorkflowVersion,
    execute_workflow,
};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

pub(crate) fn execute_and_write_workflow(
    workflow: Workflow,
    stdin: &[u8],
    force: bool,
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    let (store, _) = load_inputs(&workflow, stdin)?;
    let runner = PdfOperatorRunner::with_limits(workflow.limits.clone());
    let result = execute_workflow(&workflow, store, &runner).map_err(CliError::Core)?;
    let _ = write_outputs_with_stats(&workflow, &result.store, force, stdout)?;
    Ok(())
}

pub(crate) fn one_input_workflow(
    input: PathBuf,
    output: PathBuf,
    task_id: &'static str,
    op: OperatorSpec,
) -> Workflow {
    workflow_from_specs(
        vec![oxidepdf_core::InputSpec {
            id: ArtifactRef::new("input"),
            path: input,
        }],
        vec![ArtifactRef::new("input")],
        output,
        task_id,
        op,
    )
}

pub(crate) fn two_input_workflow(
    first: PathBuf,
    second: PathBuf,
    output: PathBuf,
    task_id: &'static str,
    op: OperatorSpec,
) -> Workflow {
    workflow_from_specs(
        vec![
            oxidepdf_core::InputSpec {
                id: ArtifactRef::new("input"),
                path: first,
            },
            oxidepdf_core::InputSpec {
                id: ArtifactRef::new("attachment"),
                path: second,
            },
        ],
        vec![ArtifactRef::new("input"), ArtifactRef::new("attachment")],
        output,
        task_id,
        op,
    )
}

pub(crate) fn multi_input_workflow(
    inputs: Vec<PathBuf>,
    output: PathBuf,
    task_id: &'static str,
    op: OperatorSpec,
) -> Workflow {
    let input_refs = (0..inputs.len())
        .map(|index| ArtifactRef::new(format!("input_{index}")))
        .collect::<Vec<_>>();
    let inputs = inputs
        .into_iter()
        .zip(input_refs.iter())
        .map(|(path, id)| oxidepdf_core::InputSpec {
            id: id.clone(),
            path,
        })
        .collect();
    workflow_from_specs(inputs, input_refs, output, task_id, op)
}

fn workflow_from_specs(
    inputs: Vec<oxidepdf_core::InputSpec>,
    task_inputs: Vec<ArtifactRef>,
    output: PathBuf,
    task_id: &'static str,
    op: OperatorSpec,
) -> Workflow {
    Workflow {
        version: WorkflowVersion::V1,
        inputs,
        tasks: vec![TaskSpec {
            id: TaskId::new(task_id),
            op,
            inputs: task_inputs,
        }],
        outputs: vec![oxidepdf_core::OutputSpec {
            id: ArtifactRef::new("output"),
            from: ArtifactRef::new(task_id),
            path: output,
        }],
        limits: Default::default(),
        metadata: WorkflowMetadata::default(),
    }
}

pub(crate) fn reject_shared_stdin_inputs(first: &Path, second: &Path) -> Result<(), CliError> {
    if is_stdio(first) && is_stdio(second) {
        return Err(CliError::Workflow(
            "commands with two independent inputs cannot read both inputs from stdin".to_owned(),
        ));
    }
    Ok(())
}

pub(crate) fn parse_workflow(bytes: &[u8], path: &Path) -> Result<Workflow, CliError> {
    if path.extension().and_then(|extension| extension.to_str()) == Some("json") {
        serde_json::from_slice(bytes).map_err(|error| CliError::Workflow(error.to_string()))
    } else {
        serde_saphyr::from_slice(bytes).map_err(|error| CliError::Workflow(error.to_string()))
    }
}

pub(crate) fn load_inputs(
    workflow: &Workflow,
    stdin: &[u8],
) -> Result<(ArtifactStore, u64), CliError> {
    let mut store = ArtifactStore::new();
    let mut total_input_bytes = 0u64;
    for input in &workflow.inputs {
        let bytes = read_path_or_stdin(&input.path, stdin).map_err(CliError::Input)?;
        enforce_cli_input_limits(bytes.len(), &mut total_input_bytes, &workflow.limits)?;
        store.insert(
            input.id.clone(),
            Artifact::bytes(bytes).map_err(CliError::Core)?,
        );
    }

    Ok((store, total_input_bytes))
}

pub(crate) fn write_outputs_with_stats(
    workflow: &Workflow,
    store: &ArtifactStore,
    force: bool,
    stdout: &mut impl Write,
) -> Result<u64, CliError> {
    let mut total_output_bytes = 0u64;
    for output in &workflow.outputs {
        let artifact = store.get(&output.from).ok_or_else(|| {
            CliError::Core(OxideError::InvalidWorkflow {
                reason: format!(
                    "output '{}' references missing artifact '{}'",
                    output.id.as_str(),
                    output.from.as_str()
                ),
            })
        })?;
        let bytes = artifact.output_bytes().map_err(CliError::Core)?;
        enforce_cli_output_limit(bytes.len(), &workflow.limits)?;
        total_output_bytes = total_output_bytes
            .checked_add(bytes.len() as u64)
            .ok_or(CliError::Core(OxideError::Internal))?;
        if is_stdio(&output.path) {
            stdout.write_all(&bytes).map_err(CliError::Io)?;
        } else {
            if output.path.exists() && !force {
                return Err(CliError::Workflow(format!(
                    "output file already exists: {}",
                    output.path.display()
                )));
            }
            fs::write(&output.path, &bytes).map_err(CliError::Io)?;
        }
    }

    Ok(total_output_bytes)
}

pub(crate) fn read_path_or_stdin(path: &Path, stdin: &[u8]) -> io::Result<Vec<u8>> {
    if is_stdio(path) {
        Ok(stdin.to_vec())
    } else {
        fs::read(path)
    }
}

fn enforce_cli_input_limits(
    size: usize,
    total_input_bytes: &mut u64,
    limits: &ResourceLimits,
) -> Result<(), CliError> {
    if limits
        .max_input_bytes
        .is_some_and(|limit| size as u64 > limit)
    {
        return Err(CliError::Core(OxideError::ResourceLimitExceeded {
            limit: "max_input_bytes".to_owned(),
        }));
    }
    *total_input_bytes = total_input_bytes.checked_add(size as u64).ok_or_else(|| {
        CliError::Core(OxideError::ResourceLimitExceeded {
            limit: "max_total_input_bytes".to_owned(),
        })
    })?;
    if limits
        .max_total_input_bytes
        .is_some_and(|limit| *total_input_bytes > limit)
    {
        return Err(CliError::Core(OxideError::ResourceLimitExceeded {
            limit: "max_total_input_bytes".to_owned(),
        }));
    }

    Ok(())
}

fn enforce_cli_output_limit(size: usize, limits: &ResourceLimits) -> Result<(), CliError> {
    if limits
        .max_output_bytes
        .is_some_and(|limit| size as u64 > limit)
    {
        return Err(CliError::Core(OxideError::ResourceLimitExceeded {
            limit: "max_output_bytes".to_owned(),
        }));
    }

    Ok(())
}

pub(crate) fn is_stdio(path: &Path) -> bool {
    path == Path::new("-")
}
