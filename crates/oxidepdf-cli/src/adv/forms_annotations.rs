pub(crate) async fn run_annot(
    command: AnnotCommand,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    match command {
        AnnotCommand::List(args) => execute_and_write_workflow(
            one_input_workflow(
                args.input,
                args.output,
                "annot_list",
                OperatorSpec::PdfInspect(PdfInspectOptions::Annotations(
                    AnnotationInspectOptions::default(),
                )),
            ),
            stdin,
            args.force,
            stdout,
        )
        .await,
        AnnotCommand::Add(args) => execute_and_write_workflow(
            one_input_workflow(
                args.input,
                args.output,
                "annot_add",
                OperatorSpec::PdfEdit(PdfEditOptions::Annotation(AnnotationEditOptions {
                    action: AnnotationEditAction::AddText,
                    page: Some(args.page),
                    id: Some(args.id),
                    text: Some(args.text),
                })),
            ),
            stdin,
            args.force,
            stdout,
        )
        .await,
        AnnotCommand::Delete(args) => execute_and_write_workflow(
            one_input_workflow(
                args.input,
                args.output,
                "annot_delete",
                OperatorSpec::PdfEdit(PdfEditOptions::Annotation(AnnotationEditOptions {
                    action: AnnotationEditAction::Delete,
                    page: None,
                    id: Some(args.id),
                    text: None,
                })),
            ),
            stdin,
            args.force,
            stdout,
        )
        .await,
    }
}

pub(crate) async fn run_form(
    command: FormCommand,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    match command {
        FormCommand::Inspect(args) => execute_and_write_workflow(
            one_input_workflow(
                args.input,
                args.output,
                "form_inspect",
                OperatorSpec::PdfInspect(PdfInspectOptions::Forms(FormInspectOptions::default())),
            ),
            stdin,
            args.force,
            stdout,
        )
        .await,
        FormCommand::Fill(args) => execute_and_write_workflow(
            one_input_workflow(
                args.input,
                args.output,
                "form_fill",
                OperatorSpec::PdfEdit(PdfEditOptions::FormFill(FormFillOptions {
                    fields: parse_form_fields(args.fields)?,
                })),
            ),
            stdin,
            args.force,
            stdout,
        )
        .await,
        FormCommand::UnlockReadonly(args) => execute_and_write_workflow(
            one_input_workflow(
                args.input,
                args.output,
                "form_unlock_readonly",
                OperatorSpec::PdfEdit(PdfEditOptions::FormUnlockReadonly),
            ),
            stdin,
            args.force,
            stdout,
        )
        .await,
        FormCommand::Remove(args) => execute_and_write_workflow(
            one_input_workflow(
                args.input,
                args.output,
                "form_remove",
                OperatorSpec::PdfEdit(PdfEditOptions::FormRemove),
            ),
            stdin,
            args.force,
            stdout,
        )
        .await,
    }
}

pub(crate) async fn run_interactive_remove(
    args: InteractiveRemoveArgs,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    execute_and_write_workflow(
        one_input_workflow(
            args.input,
            args.output,
            "interactive_remove",
            OperatorSpec::PdfEdit(PdfEditOptions::InteractiveRemove(
                InteractiveRemovalOptions {
                    annotations: args.annotations,
                    forms: args.forms,
                    actions: args.actions,
                    javascript: args.javascript,
                    embedded_files: args.embedded_files,
                },
            )),
        ),
        stdin,
        args.force,
        stdout,
    )
    .await
}
