pub(crate) fn run_metadata(
    command: MetadataCommand,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    match command {
        MetadataCommand::Get(args) => execute_and_write_workflow(
            one_input_workflow(
                args.input,
                args.output,
                "metadata_get",
                OperatorSpec::PdfInspect(PdfInspectOptions::Metadata(
                    MetadataInspectOptions::default(),
                )),
            ),
            stdin,
            args.force,
            stdout,
        ),
        MetadataCommand::Set(args) => execute_and_write_workflow(
            one_input_workflow(
                args.input,
                args.output,
                "metadata_set",
                OperatorSpec::PdfEdit(PdfEditOptions::Metadata(MetadataEditOptions {
                    action: MetadataEditAction::Set,
                    entries: parse_metadata_entries(args.entries)?,
                    keys: Vec::new(),
                })),
            ),
            stdin,
            args.force,
            stdout,
        ),
        MetadataCommand::Delete(args) => execute_and_write_workflow(
            one_input_workflow(
                args.input,
                args.output,
                "metadata_delete",
                OperatorSpec::PdfEdit(PdfEditOptions::Metadata(MetadataEditOptions {
                    action: MetadataEditAction::Delete,
                    entries: Vec::new(),
                    keys: args.keys,
                })),
            ),
            stdin,
            args.force,
            stdout,
        ),
        MetadataCommand::Validate(args) => execute_and_write_workflow(
            one_input_workflow(
                args.input,
                args.output,
                "metadata_validate",
                OperatorSpec::PdfInspect(PdfInspectOptions::Metadata(
                    MetadataInspectOptions::default(),
                )),
            ),
            stdin,
            args.force,
            stdout,
        ),
    }
}

