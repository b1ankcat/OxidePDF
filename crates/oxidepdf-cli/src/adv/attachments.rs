pub(crate) fn run_attach(
    command: AttachCommand,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    match command {
        AttachCommand::Add(args) => {
            reject_shared_stdin_inputs(&args.input, &args.file)?;
            let name = match args.name {
                Some(name) => Some(name),
                None => Some(
                    args.file
                        .file_name()
                        .and_then(|name| name.to_str())
                        .ok_or_else(|| {
                            CliError::Workflow(
                                "attachment name must be explicit for this path".to_owned(),
                            )
                        })?
                        .to_owned(),
                ),
            };
            let workflow = two_input_workflow(
                args.input,
                args.file,
                args.output,
                "attach_add",
                OperatorSpec::PdfEdit(PdfEditOptions::Attachment(AttachmentEditOptions {
                    action: AttachmentEditAction::Add,
                    name,
                    description: args.description,
                })),
            );
            execute_and_write_workflow(workflow, stdin, args.force, stdout)
        }
        AttachCommand::List(args) => execute_and_write_workflow(
            one_input_workflow(
                args.input,
                args.output,
                "attach_list",
                OperatorSpec::PdfInspect(PdfInspectOptions::Attachments(
                    AttachmentInspectOptions::default(),
                )),
            ),
            stdin,
            args.force,
            stdout,
        ),
        AttachCommand::Extract(args) => execute_and_write_workflow(
            one_input_workflow(
                args.input,
                args.output,
                "attach_extract",
                OperatorSpec::PdfInspect(PdfInspectOptions::AttachmentExtract(
                    AttachmentExtractOptions { name: args.name },
                )),
            ),
            stdin,
            args.force,
            stdout,
        ),
        AttachCommand::Delete(args) => execute_and_write_workflow(
            one_input_workflow(
                args.input,
                args.output,
                "attach_delete",
                OperatorSpec::PdfEdit(PdfEditOptions::Attachment(AttachmentEditOptions {
                    action: AttachmentEditAction::Delete,
                    name: Some(args.name),
                    description: None,
                })),
            ),
            stdin,
            args.force,
            stdout,
        ),
    }
}

