use super::*;

pub(crate) fn run_workflow(
    args: RunArgs,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    let workflow_bytes = read_path_or_stdin(&args.workflow, stdin).map_err(CliError::Input)?;
    let workflow = parse_workflow(&workflow_bytes, &args.workflow)?;
    let store = load_inputs(&workflow, stdin)?;
    let runner = PdfOperatorRunner::with_limits(workflow.limits.clone());
    let result = execute_workflow(&workflow, store, &runner).map_err(CliError::Core)?;
    write_outputs(&workflow, &result.store, args.force, stdout)?;

    Ok(())
}
