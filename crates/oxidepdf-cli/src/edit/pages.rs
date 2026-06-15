pub(crate) fn run_merge(
    args: MergeArgs,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    let workflow = multi_input_workflow(
        args.inputs,
        args.output,
        "merge",
        OperatorSpec::PdfEdit(PdfEditOptions::Merge(MergeOptions {})),
    );
    execute_and_write_workflow(workflow, stdin, args.force, stdout)
}

pub(crate) fn run_page_selection(
    args: PageSelectionArgs,
    stdin: &[u8],
    stdout: &mut impl Write,
    command: PageCommand,
) -> Result<(), CliError> {
    let task_id = match command {
        PageCommand::Keep => "keep_pages",
        PageCommand::Extract => "extract_pages",
        PageCommand::Reorder => "reorder_pages",
    };
    let op = match command {
        PageCommand::Keep => OperatorSpec::PdfEdit(PdfEditOptions::KeepPages(SplitOptions {
            pages: args.pages,
        })),
        PageCommand::Extract => {
            OperatorSpec::PdfEdit(PdfEditOptions::ExtractPages(PageSelectionOptions {
                pages: args.pages,
            }))
        }
        PageCommand::Reorder => {
            OperatorSpec::PdfEdit(PdfEditOptions::ReorderPages(ReorderOptions {
                pages: args.pages,
            }))
        }
    };
    let workflow = one_input_workflow(args.input, args.output, task_id, op);

    execute_and_write_workflow(workflow, stdin, args.force, stdout)
}

pub(crate) fn run_rotate(
    args: RotateArgs,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    let workflow = one_input_workflow(
        args.input,
        args.output,
        "rotate_pages",
        OperatorSpec::PdfEdit(PdfEditOptions::RotatePages(RotateOptions {
            pages: args.pages,
            degrees: args.degrees,
        })),
    );

    execute_and_write_workflow(workflow, stdin, args.force, stdout)
}

pub(crate) fn run_delete_pages(
    args: PageSelectionArgs,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    let workflow = one_input_workflow(
        args.input,
        args.output,
        "delete_pages",
        OperatorSpec::PdfEdit(PdfEditOptions::DeletePages(PageSelectionOptions {
            pages: args.pages,
        })),
    );

    execute_and_write_workflow(workflow, stdin, args.force, stdout)
}

pub(crate) fn run_delete_blank_pages(
    args: DeleteBlankPagesArgs,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    let workflow = one_input_workflow(
        args.input,
        args.output,
        "delete_blank_pages",
        OperatorSpec::PdfEdit(PdfEditOptions::DeleteBlankPages(
            DeleteBlankPagesOptions::default(),
        )),
    );

    execute_and_write_workflow(workflow, stdin, args.force, stdout)
}
