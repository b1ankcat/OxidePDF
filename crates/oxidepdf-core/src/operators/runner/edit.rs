pub(crate) fn run_pdf_edit(
    options: &PdfEditOptions,
    inputs: &[Artifact],
    limits: &ResourceLimits,
) -> Result<Artifact, OxideError> {
    match options {
        PdfEditOptions::Merge(_) => {
            let document = crate::page_ops::merge_artifacts_to_document(inputs, limits)?;
            Ok(Artifact::pdf_object(document))
        }
        PdfEditOptions::KeepPages(options) => {
            let mut document = single_pdf_document(inputs, limits)?;
            crate::page_ops::split_on_document(&mut document, &options.pages, limits)?;
            Ok(Artifact::pdf_object(document))
        }
        PdfEditOptions::ExtractPages(options) => {
            let mut document = single_pdf_document(inputs, limits)?;
            crate::page_ops::split_on_document(&mut document, &options.pages, limits)?;
            Ok(Artifact::pdf_object(document))
        }
        PdfEditOptions::ReorderPages(options) => {
            // Reorder reuses the order-preserving page selection primitive.
            let mut document = single_pdf_document(inputs, limits)?;
            crate::page_ops::split_on_document(&mut document, &options.pages, limits)?;
            Ok(Artifact::pdf_object(document))
        }
        PdfEditOptions::RotatePages(options) => {
            let mut document = single_pdf_document(inputs, limits)?;
            crate::page_ops::rotate_on_document(
                &mut document,
                &options.pages,
                options.degrees,
                limits,
            )?;
            Ok(Artifact::pdf_object(document))
        }
        PdfEditOptions::DeletePages(options) => {
            let mut document = single_pdf_document(inputs, limits)?;
            crate::page_ops::delete_pages_on_document(&mut document, &options.pages, limits)?;
            Ok(Artifact::pdf_object(document))
        }
        PdfEditOptions::DeleteBlankPages(options) => {
            let mut document = single_pdf_document(inputs, limits)?;
            crate::page_ops::delete_blank_pages_on_document(&mut document, options, limits)?;
            Ok(Artifact::pdf_object(document))
        }
        PdfEditOptions::CropPages(options) => {
            let mut document = single_pdf_document(inputs, limits)?;
            crate::page_ops::crop_pages_on_document(&mut document, options, limits)?;
            Ok(Artifact::pdf_object(document))
        }
        PdfEditOptions::ScalePages(options) => {
            let mut document = single_pdf_document(inputs, limits)?;
            crate::page_ops::scale_pages_on_document(&mut document, options, limits)?;
            Ok(Artifact::pdf_object(document))
        }
        PdfEditOptions::SinglePage(options) => {
            let mut document = single_pdf_document(inputs, limits)?;
            crate::page_ops::single_page_on_document(&mut document, options, limits)?;
            Ok(Artifact::pdf_object(document))
        }
        PdfEditOptions::NUp(options) => {
            let input = single_pdf_input_bytes(inputs, limits)?;
            nup_pdf_pages_with_limits(&input, options, limits).map(Artifact::Pdf)
        }
        PdfEditOptions::Booklet(options) => {
            let input = single_pdf_input_bytes(inputs, limits)?;
            booklet_pdf_pages_with_limits(&input, options, limits).map(Artifact::Pdf)
        }
        PdfEditOptions::PageNumbers(options) => {
            let mut document = single_pdf_document(inputs, limits)?;
            crate::page_ops::add_page_numbers_on_document(&mut document, options, limits)?;
            Ok(Artifact::pdf_object(document))
        }
        PdfEditOptions::ImageToPdf(options) => {
            image_artifacts_to_pdf(inputs, options, limits).map(Artifact::Pdf)
        }
        PdfEditOptions::SvgToPdf(options) => {
            let input = single_svg_input(inputs, limits)?;
            svg_to_pdf(input, options, limits).map(Artifact::Pdf)
        }
        PdfEditOptions::Watermark(options) => {
            let inputs = materialize_object_inputs(inputs, limits)?;
            watermark_pdf_artifacts(&inputs, options, limits).map(Artifact::Pdf)
        }
        PdfEditOptions::Overlay(options) => {
            let inputs = materialize_object_inputs(inputs, limits)?;
            overlay_pdf_artifacts(&inputs, options, limits).map(Artifact::Pdf)
        }
        PdfEditOptions::ImageEdit(options) => {
            let inputs = materialize_object_inputs(inputs, limits)?;
            edit_pdf_images_artifacts(&inputs, options, limits).map(Artifact::Pdf)
        }
        PdfEditOptions::Color(options) => {
            let mut document = single_pdf_document(inputs, limits)?;
            crate::overlay::edit_colors_on_document(&mut document, options, limits)?;
            Ok(Artifact::pdf_object(document))
        }
        PdfEditOptions::Metadata(options) => {
            let mut document = single_pdf_document(inputs, limits)?;
            crate::metadata::edit_metadata_on_document(&mut document, options, limits)?;
            Ok(Artifact::pdf_object(document))
        }
        PdfEditOptions::Outline(options) => {
            let mut document = single_pdf_document(inputs, limits)?;
            crate::outlines::edit_outline_on_document(&mut document, options, limits)?;
            Ok(Artifact::pdf_object(document))
        }
        PdfEditOptions::Attachment(options) => {
            let inputs = materialize_object_inputs(inputs, limits)?;
            edit_pdf_attachment_artifacts(&inputs, options, limits).map(Artifact::Pdf)
        }
        PdfEditOptions::Annotation(options) => {
            let mut document = single_pdf_document(inputs, limits)?;
            crate::annotations::edit_annotations_on_document(&mut document, options, limits)?;
            Ok(Artifact::pdf_object(document))
        }
        PdfEditOptions::FormFill(options) => {
            let mut document = single_pdf_document(inputs, limits)?;
            crate::forms::fill_form_on_document(&mut document, options, limits)?;
            Ok(Artifact::pdf_object(document))
        }
        PdfEditOptions::FormUnlockReadonly => {
            let mut document = single_pdf_document(inputs, limits)?;
            crate::forms::unlock_form_readonly_on_document(&mut document, limits)?;
            Ok(Artifact::pdf_object(document))
        }
        PdfEditOptions::FormRemove => {
            let mut document = single_pdf_document(inputs, limits)?;
            crate::forms::remove_forms_on_document(&mut document, limits)?;
            Ok(Artifact::pdf_object(document))
        }
        PdfEditOptions::InteractiveRemove(options) => {
            let mut document = single_pdf_document(inputs, limits)?;
            crate::interactive::remove_interactive_on_document(&mut document, options, limits)?;
            Ok(Artifact::pdf_object(document))
        }
        PdfEditOptions::Compression(options) => {
            let mut document = single_pdf_document(inputs, limits)?;
            crate::compression::compress_on_document(&mut document, options, limits)?;
            Ok(Artifact::pdf_object(document))
        }
    }
}
