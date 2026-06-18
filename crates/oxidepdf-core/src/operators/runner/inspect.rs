pub(crate) fn run_pdf_inspect(
    options: &PdfInspectOptions,
    inputs: &[Artifact],
    limits: &ResourceLimits,
) -> Result<Artifact, OxideError> {
    match options {
        PdfInspectOptions::Render(options) => {
            let input = single_pdf_input_bytes(inputs, limits)?;
            render_pdf_page(&input, options, limits).map(Artifact::Image)
        }
        PdfInspectOptions::ExtractText(options) => {
            let input = single_pdf_input_bytes(inputs, limits)?;
            extract_text_from_pdf(&input, options, limits).map(Artifact::Text)
        }
        PdfInspectOptions::Metadata(options) => {
            let document = single_pdf_document_for_inspect(inputs, limits)?;
            let _ = options;
            crate::metadata::inspect_metadata_on_document(&document).map(Artifact::Text)
        }
        PdfInspectOptions::Outline(options) => {
            let document = single_pdf_document_for_inspect(inputs, limits)?;
            let _ = options;
            crate::outlines::inspect_outline_on_document(&document).map(Artifact::Text)
        }
        PdfInspectOptions::Attachments(options) => {
            let document = single_pdf_document_for_inspect(inputs, limits)?;
            let _ = options;
            crate::attachments::inspect_attachments_on_document(&document).map(Artifact::Text)
        }
        PdfInspectOptions::AttachmentExtract(options) => {
            let input = single_pdf_input_bytes(inputs, limits)?;
            extract_pdf_attachment(&input, &options.name, limits).map(Artifact::Bytes)
        }
        PdfInspectOptions::Annotations(options) => {
            let document = single_pdf_document_for_inspect(inputs, limits)?;
            let _ = options;
            crate::annotations::inspect_annotations_on_document(&document).map(Artifact::Text)
        }
        PdfInspectOptions::Forms(options) => {
            let document = single_pdf_document_for_inspect(inputs, limits)?;
            let _ = options;
            crate::forms::inspect_forms_on_document(&document).map(Artifact::Text)
        }
        PdfInspectOptions::Images(options) => {
            let document = single_pdf_document_for_inspect(inputs, limits)?;
            let _ = options;
            crate::overlay::inspect_images_on_document(&document).map(Artifact::Text)
        }
        PdfInspectOptions::ImageExtract(options) => {
            let input = single_pdf_input_bytes(inputs, limits)?;
            extract_pdf_image(&input, options, limits).map(Artifact::Bytes)
        }
    }
}
