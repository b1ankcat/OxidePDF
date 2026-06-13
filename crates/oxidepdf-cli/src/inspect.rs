use super::*;

pub(crate) fn run_pdf_inspect(
    command: PdfInspectCommand,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    match command {
        PdfInspectCommand::Render(args) => run_render(args, stdin, stdout),
        PdfInspectCommand::ExtractText(args) => run_extract_text(args, stdin, stdout),
    }
}

pub(crate) fn run_render(
    args: RenderArgs,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    let workflow = one_input_workflow(
        args.input,
        args.output,
        "render",
        OperatorSpec::PdfInspect(PdfInspectOptions::Render(RenderOptions {
            page: args.page,
            format: Some("png".to_owned()),
            scale: args.scale,
        })),
    );

    execute_and_write_workflow(workflow, stdin, args.force, stdout)
}

pub(crate) fn run_extract_text(
    args: ExtractTextArgs,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    let workflow = one_input_workflow(
        args.input,
        args.output,
        "extract_text",
        OperatorSpec::PdfInspect(PdfInspectOptions::ExtractText(ExtractTextOptions {
            format: Some("plain".to_owned()),
        })),
    );

    execute_and_write_workflow(workflow, stdin, args.force, stdout)
}
