pub(crate) fn run_merge(
    args: MergeArgs,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    let input_refs = (0..args.inputs.len())
        .map(|index| ArtifactRef::new(format!("input_{index}")))
        .collect::<Vec<_>>();
    let workflow = Workflow {
        version: WorkflowVersion::V1,
        inputs: args
            .inputs
            .into_iter()
            .zip(input_refs.iter())
            .map(|(path, id)| oxidepdf_core::InputSpec {
                id: id.clone(),
                path,
            })
            .collect(),
        tasks: vec![TaskSpec {
            id: TaskId::new("merge"),
            op: OperatorSpec::PdfEdit(PdfEditOptions::Merge(MergeOptions {})),
            inputs: input_refs,
        }],
        outputs: vec![oxidepdf_core::OutputSpec {
            id: ArtifactRef::new("output"),
            from: ArtifactRef::new("merge"),
            path: args.output,
        }],
        limits: Default::default(),
        metadata: WorkflowMetadata::default(),
    };
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

