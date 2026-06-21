
impl From<PdfEditOptions> for PdfEditOptionsDef {
    fn from(value: PdfEditOptions) -> Self {
        match value {
            PdfEditOptions::Merge(options) => Self {
                merge: Some(options),
                ..Self::default()
            },
            PdfEditOptions::KeepPages(options) => Self {
                keep_pages: Some(options),
                ..Self::default()
            },
            PdfEditOptions::ExtractPages(options) => Self {
                extract_pages: Some(options),
                ..Self::default()
            },
            PdfEditOptions::ReorderPages(options) => Self {
                reorder_pages: Some(options),
                ..Self::default()
            },
            PdfEditOptions::RotatePages(options) => Self {
                rotate_pages: Some(options),
                ..Self::default()
            },
            PdfEditOptions::DeletePages(options) => Self {
                delete_pages: Some(options),
                ..Self::default()
            },
            PdfEditOptions::DeleteBlankPages(options) => Self {
                delete_blank_pages: Some(options),
                ..Self::default()
            },
            PdfEditOptions::CropPages(options) => Self {
                crop_pages: Some(options),
                ..Self::default()
            },
            PdfEditOptions::ScalePages(options) => Self {
                scale_pages: Some(options),
                ..Self::default()
            },
            PdfEditOptions::SinglePage(options) => Self {
                single_page: Some(options),
                ..Self::default()
            },
            PdfEditOptions::NUp(options) => Self {
                nup: Some(options),
                ..Self::default()
            },
            PdfEditOptions::Booklet(options) => Self {
                booklet: Some(options),
                ..Self::default()
            },
            PdfEditOptions::PageNumbers(options) => Self {
                page_numbers: Some(options),
                ..Self::default()
            },
            PdfEditOptions::ImageToPdf(options) => Self {
                image_to_pdf: Some(options),
                ..Self::default()
            },
            PdfEditOptions::SvgToPdf(options) => Self {
                svg_to_pdf: Some(options),
                ..Self::default()
            },
            PdfEditOptions::Watermark(options) => Self {
                watermark: Some(options),
                ..Self::default()
            },
            PdfEditOptions::Overlay(options) => Self {
                overlay: Some(options),
                ..Self::default()
            },
            PdfEditOptions::ImageEdit(options) => Self {
                image_edit: Some(options),
                ..Self::default()
            },
            PdfEditOptions::Color(options) => Self {
                color: Some(options),
                ..Self::default()
            },
            PdfEditOptions::Metadata(options) => Self {
                metadata: Some(options),
                ..Self::default()
            },
            PdfEditOptions::Outline(options) => Self {
                outline: Some(options),
                ..Self::default()
            },
            PdfEditOptions::Attachment(options) => Self {
                attachment: Some(options),
                ..Self::default()
            },
            PdfEditOptions::Annotation(options) => Self {
                annotation: Some(options),
                ..Self::default()
            },
            PdfEditOptions::FormFill(options) => Self {
                form_fill: Some(options),
                ..Self::default()
            },
            PdfEditOptions::FormUnlockReadonly => Self {
                form_unlock_readonly: Some(()),
                ..Self::default()
            },
            PdfEditOptions::FormRemove => Self {
                form_remove: Some(()),
                ..Self::default()
            },
            PdfEditOptions::InteractiveRemove(options) => Self {
                interactive_remove: Some(options),
                ..Self::default()
            },
            PdfEditOptions::Compression(options) => Self {
                compression: Some(options),
                ..Self::default()
            },
        }
    }
}
