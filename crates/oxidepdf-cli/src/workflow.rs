use super::*;

pub(crate) fn run_workflow(
    args: RunArgs,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    let workflow_from_stdin = is_stdio(&args.workflow);
    let workflow_bytes = read_path_or_stdin(&args.workflow, stdin).map_err(CliError::Input)?;
    let workflow = parse_workflow(&workflow_bytes, &args.workflow)?;

    let stdin_inputs = workflow
        .inputs
        .iter()
        .filter(|input| is_stdio(&input.path))
        .count();
    if workflow_from_stdin && stdin_inputs > 0 {
        return Err(CliError::Workflow(
            "workflow read from stdin cannot also declare a stdin ('-') input".to_owned(),
        ));
    }
    if stdin_inputs > 1 {
        return Err(CliError::Workflow(
            "workflow cannot read more than one input from stdin ('-')".to_owned(),
        ));
    }

    let store = load_inputs(&workflow, stdin)?;
    let runner = PdfOperatorRunner::with_limits(workflow.limits.clone());
    let result = execute_workflow(&workflow, store, &runner).map_err(CliError::Core)?;
    write_outputs(&workflow, &result.store, args.force, stdout)?;

    Ok(())
}
