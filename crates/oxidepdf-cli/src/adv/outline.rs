pub(crate) async fn run_outline(
    command: OutlineCommand,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    match command {
        OutlineCommand::Get(args) => execute_and_write_workflow(
            one_input_workflow(
                args.input,
                args.output,
                "outline_get",
                OperatorSpec::PdfInspect(PdfInspectOptions::Outline(
                    OutlineInspectOptions::default(),
                )),
            ),
            stdin,
            args.force,
            stdout,
        )
        .await,
        OutlineCommand::Set(args) => {
            reject_shared_stdin_inputs(&args.input, &args.tree)?;
            let tree_bytes = read_path_or_stdin(&args.tree, stdin).map_err(CliError::Input)?;
            let tree: OutlineTree = serde_json::from_slice(&tree_bytes)
                .map_err(|error| CliError::Workflow(error.to_string()))?;
            execute_and_write_workflow(
                one_input_workflow(
                    args.input,
                    args.output,
                    "outline_set",
                    OperatorSpec::PdfEdit(PdfEditOptions::Outline(OutlineEditOptions {
                        action: OutlineEditAction::Set,
                        tree: Some(tree),
                    })),
                ),
                stdin,
                args.force,
                stdout,
            )
            .await
        }
        OutlineCommand::Delete(args) => execute_and_write_workflow(
            one_input_workflow(
                args.input,
                args.output,
                "outline_delete",
                OperatorSpec::PdfEdit(PdfEditOptions::Outline(OutlineEditOptions {
                    action: OutlineEditAction::Delete,
                    tree: None,
                })),
            ),
            stdin,
            args.force,
            stdout,
        )
        .await,
    }
}
