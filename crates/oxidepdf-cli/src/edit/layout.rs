pub(crate) async fn run_crop_pages(
    args: CropPagesArgs,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    let workflow = one_input_workflow(
        args.input,
        args.output,
        "crop_pages",
        OperatorSpec::PdfEdit(PdfEditOptions::CropPages(CropPagesOptions {
            pages: args.pages,
            left: args.left,
            bottom: args.bottom,
            right: args.right,
            top: args.top,
        })),
    );

    execute_and_write_workflow(workflow, stdin, args.force, stdout).await
}

pub(crate) async fn run_scale_pages(
    args: ScalePagesArgs,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    let workflow = one_input_workflow(
        args.input,
        args.output,
        "scale_pages",
        OperatorSpec::PdfEdit(PdfEditOptions::ScalePages(ScalePagesOptions {
            pages: args.pages,
            factor: args.factor,
        })),
    );

    execute_and_write_workflow(workflow, stdin, args.force, stdout).await
}

pub(crate) async fn run_single_page(
    args: SinglePageArgs,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    let workflow = one_input_workflow(
        args.input,
        args.output,
        "single_page",
        OperatorSpec::PdfEdit(PdfEditOptions::SinglePage(SinglePageOptions::default())),
    );

    execute_and_write_workflow(workflow, stdin, args.force, stdout).await
}

pub(crate) async fn run_nup(
    args: NUpArgs,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    let workflow = one_input_workflow(
        args.input,
        args.output,
        "nup",
        OperatorSpec::PdfEdit(PdfEditOptions::NUp(NUpOptions {
            columns: args.columns,
            rows: args.rows,
        })),
    );

    execute_and_write_workflow(workflow, stdin, args.force, stdout).await
}

pub(crate) async fn run_booklet(
    args: BookletArgs,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    let workflow = one_input_workflow(
        args.input,
        args.output,
        "booklet",
        OperatorSpec::PdfEdit(PdfEditOptions::Booklet(BookletOptions::default())),
    );

    execute_and_write_workflow(workflow, stdin, args.force, stdout).await
}

pub(crate) async fn run_page_numbers(
    args: PageNumbersArgs,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    let workflow = one_input_workflow(
        args.input,
        args.output,
        "page_numbers",
        OperatorSpec::PdfEdit(PdfEditOptions::PageNumbers(PageNumbersOptions {
            pages: args.pages,
            start: args.start,
            prefix: args.prefix,
            suffix: args.suffix,
            font_size: args.font_size,
            position: args.position.into(),
        })),
    );

    execute_and_write_workflow(workflow, stdin, args.force, stdout).await
}

