pub(crate) fn run_pdf_edit(
    command: PdfEditCommand,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    match command {
        PdfEditCommand::Merge(args) => run_merge(args, stdin, stdout),
        PdfEditCommand::KeepPages(args) => {
            run_page_selection(args, stdin, stdout, PageCommand::Keep)
        }
        PdfEditCommand::ExtractPages(args) => {
            run_page_selection(args, stdin, stdout, PageCommand::Extract)
        }
        PdfEditCommand::ReorderPages(args) => {
            run_page_selection(args, stdin, stdout, PageCommand::Reorder)
        }
        PdfEditCommand::RotatePages(args) => run_rotate(args, stdin, stdout),
        PdfEditCommand::DeletePages(args) => run_delete_pages(args, stdin, stdout),
        PdfEditCommand::DeleteBlankPages(args) => run_delete_blank_pages(args, stdin, stdout),
        PdfEditCommand::CropPages(args) => run_crop_pages(args, stdin, stdout),
        PdfEditCommand::ScalePages(args) => run_scale_pages(args, stdin, stdout),
        PdfEditCommand::SinglePage(args) => run_single_page(args, stdin, stdout),
        PdfEditCommand::NUp(args) => run_nup(args, stdin, stdout),
        PdfEditCommand::Booklet(args) => run_booklet(args, stdin, stdout),
        PdfEditCommand::PageNumbers(args) => run_page_numbers(args, stdin, stdout),
        PdfEditCommand::Img2pdf(args) => run_img2pdf(args, stdin, stdout),
        PdfEditCommand::Svg2pdf(args) => run_svg2pdf(args, stdin, stdout),
        PdfEditCommand::Watermark(args) => run_watermark(args, stdin, stdout),
        PdfEditCommand::Compress(args) => run_compress(args, stdin, stdout),
        PdfEditCommand::Stamp(args) => run_stamp(args, stdin, stdout),
        PdfEditCommand::OverlayPdf(args) => run_overlay_pdf(args, stdin, stdout),
        PdfEditCommand::Color(command) => run_color(command, stdin, stdout),
        PdfEditCommand::InteractiveRemove(args) => run_interactive_remove(args, stdin, stdout),
    }
}
