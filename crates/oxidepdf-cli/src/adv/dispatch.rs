pub(crate) fn run_pdf_adv(
    command: PdfAdvCommand,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    match command {
        PdfAdvCommand::Metadata(command) => run_metadata(command, stdin, stdout),
        PdfAdvCommand::Outline(command) => run_outline(command, stdin, stdout),
        PdfAdvCommand::Attach(command) => run_attach(command, stdin, stdout),
        PdfAdvCommand::Annot(command) => run_annot(command, stdin, stdout),
        PdfAdvCommand::Form(command) => run_form(command, stdin, stdout),
        PdfAdvCommand::Image(command) => run_image(command, stdin, stdout),
    }
}
