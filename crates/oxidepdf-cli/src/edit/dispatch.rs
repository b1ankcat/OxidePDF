pub(crate) async fn run_pdf_edit(
    command: PdfEditCommand,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    match command {
        PdfEditCommand::Merge(args) => run_merge(args, stdin, stdout).await,
        PdfEditCommand::KeepPages(args) => {
            run_page_selection(args, stdin, stdout, PageCommand::Keep).await
        }
        PdfEditCommand::ExtractPages(args) => {
            run_page_selection(args, stdin, stdout, PageCommand::Extract).await
        }
        PdfEditCommand::ReorderPages(args) => {
            run_page_selection(args, stdin, stdout, PageCommand::Reorder).await
        }
        PdfEditCommand::RotatePages(args) => run_rotate(args, stdin, stdout).await,
        PdfEditCommand::DeletePages(args) => run_delete_pages(args, stdin, stdout).await,
        PdfEditCommand::DeleteBlankPages(args) => run_delete_blank_pages(args, stdin, stdout).await,
        PdfEditCommand::CropPages(args) => run_crop_pages(args, stdin, stdout).await,
        PdfEditCommand::ScalePages(args) => run_scale_pages(args, stdin, stdout).await,
        PdfEditCommand::SinglePage(args) => run_single_page(args, stdin, stdout).await,
        PdfEditCommand::NUp(args) => run_nup(args, stdin, stdout).await,
        PdfEditCommand::Booklet(args) => run_booklet(args, stdin, stdout).await,
        PdfEditCommand::PageNumbers(args) => run_page_numbers(args, stdin, stdout).await,
        PdfEditCommand::Img2pdf(args) => run_img2pdf(args, stdin, stdout).await,
        PdfEditCommand::Svg2pdf(args) => run_svg2pdf(args, stdin, stdout).await,
        PdfEditCommand::Watermark(args) => run_watermark(args, stdin, stdout).await,
        PdfEditCommand::Compress(args) => run_compress(args, stdin, stdout).await,
        PdfEditCommand::Stamp(args) => run_stamp(args, stdin, stdout).await,
        PdfEditCommand::OverlayPdf(args) => run_overlay_pdf(args, stdin, stdout).await,
        PdfEditCommand::Color(command) => run_color(command, stdin, stdout).await,
        PdfEditCommand::InteractiveRemove(args) => run_interactive_remove(args, stdin, stdout).await,
    }
}
