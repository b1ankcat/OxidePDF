use crate::{
    AnnotationEditOptions, AttachmentEditOptions, BookletOptions, ColorEditOptions,
    CompressionOptions, CropPagesOptions, DeleteBlankPagesOptions, FormFillOptions,
    ImageEditOptions, ImageToPdfOptions, InteractiveRemovalOptions, MergeOptions,
    MetadataEditOptions, NUpOptions, OutlineEditOptions, OverlayOptions, OxideError,
    PageNumbersOptions, PageSelectionOptions, ReorderOptions, RotateOptions, ScalePagesOptions,
    SinglePageOptions, SplitOptions, SvgToPdfOptions, WatermarkOptions,
};
use serde::{Deserialize, Serialize};

/// PDF edit and creation operations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "PdfEditOptionsDef", into = "PdfEditOptionsDef")]
pub enum PdfEditOptions {
    /// Merge multiple PDFs.
    Merge(MergeOptions),
    /// Keep selected pages from a PDF.
    KeepPages(SplitOptions),
    /// Extract selected pages from a PDF.
    ExtractPages(PageSelectionOptions),
    /// Reorder pages in a PDF.
    ReorderPages(ReorderOptions),
    /// Rotate selected pages.
    RotatePages(RotateOptions),
    /// Delete selected pages.
    DeletePages(PageSelectionOptions),
    /// Delete pages with no content streams and no page resources.
    DeleteBlankPages(DeleteBlankPagesOptions),
    /// Crop selected pages.
    CropPages(CropPagesOptions),
    /// Scale selected pages.
    ScalePages(ScalePagesOptions),
    /// Combine all pages into one tall page.
    SinglePage(SinglePageOptions),
    /// Lay multiple source pages on each output page.
    NUp(NUpOptions),
    /// Arrange pages for booklet printing.
    Booklet(BookletOptions),
    /// Add page numbers to pages.
    PageNumbers(PageNumbersOptions),
    /// Convert images to PDF pages.
    ImageToPdf(ImageToPdfOptions),
    /// Convert SVG to PDF.
    SvgToPdf(SvgToPdfOptions),
    /// Add a watermark to a PDF.
    Watermark(WatermarkOptions),
    /// Add text, image, SVG, stamp, signature appearance, or PDF page overlay.
    Overlay(OverlayOptions),
    /// Add, replace, or delete image XObject resources.
    ImageEdit(ImageEditOptions),
    /// Edit simple RGB color operators.
    Color(ColorEditOptions),
    /// Edit document metadata.
    Metadata(MetadataEditOptions),
    /// Edit outline tree.
    Outline(OutlineEditOptions),
    /// Add or delete embedded file attachments.
    Attachment(AttachmentEditOptions),
    /// Add or delete annotations.
    Annotation(AnnotationEditOptions),
    /// Fill AcroForm fields.
    FormFill(FormFillOptions),
    /// Clear AcroForm read-only flags.
    FormUnlockReadonly,
    /// Remove AcroForm and form widgets.
    FormRemove,
    /// Remove selected interactive document elements.
    InteractiveRemove(InteractiveRemovalOptions),
    /// Compress and optimize a PDF without implicit quality loss.
    Compression(CompressionOptions),
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct PdfEditOptionsDef {
    merge: Option<MergeOptions>,
    keep_pages: Option<SplitOptions>,
    extract_pages: Option<PageSelectionOptions>,
    reorder_pages: Option<ReorderOptions>,
    rotate_pages: Option<RotateOptions>,
    delete_pages: Option<PageSelectionOptions>,
    delete_blank_pages: Option<DeleteBlankPagesOptions>,
    crop_pages: Option<CropPagesOptions>,
    scale_pages: Option<ScalePagesOptions>,
    single_page: Option<SinglePageOptions>,
    nup: Option<NUpOptions>,
    booklet: Option<BookletOptions>,
    page_numbers: Option<PageNumbersOptions>,
    image_to_pdf: Option<ImageToPdfOptions>,
    svg_to_pdf: Option<SvgToPdfOptions>,
    watermark: Option<WatermarkOptions>,
    overlay: Option<OverlayOptions>,
    image_edit: Option<ImageEditOptions>,
    color: Option<ColorEditOptions>,
    metadata: Option<MetadataEditOptions>,
    outline: Option<OutlineEditOptions>,
    attachment: Option<AttachmentEditOptions>,
    annotation: Option<AnnotationEditOptions>,
    form_fill: Option<FormFillOptions>,
    form_unlock_readonly: Option<()>,
    form_remove: Option<()>,
    interactive_remove: Option<InteractiveRemovalOptions>,
    compression: Option<CompressionOptions>,
}

impl TryFrom<PdfEditOptionsDef> for PdfEditOptions {
    type Error = OxideError;

    fn try_from(value: PdfEditOptionsDef) -> Result<Self, Self::Error> {
        let operation_count = [
            value.merge.is_some(),
            value.keep_pages.is_some(),
            value.extract_pages.is_some(),
            value.reorder_pages.is_some(),
            value.rotate_pages.is_some(),
            value.delete_pages.is_some(),
            value.delete_blank_pages.is_some(),
            value.crop_pages.is_some(),
            value.scale_pages.is_some(),
            value.single_page.is_some(),
            value.nup.is_some(),
            value.booklet.is_some(),
            value.page_numbers.is_some(),
            value.image_to_pdf.is_some(),
            value.svg_to_pdf.is_some(),
            value.watermark.is_some(),
            value.overlay.is_some(),
            value.image_edit.is_some(),
            value.color.is_some(),
            value.metadata.is_some(),
            value.outline.is_some(),
            value.attachment.is_some(),
            value.annotation.is_some(),
            value.form_fill.is_some(),
            value.form_unlock_readonly.is_some(),
            value.form_remove.is_some(),
            value.interactive_remove.is_some(),
            value.compression.is_some(),
        ]
        .into_iter()
        .filter(|present| *present)
        .count();

        if operation_count != 1 {
            return Err(OxideError::InvalidWorkflow {
                reason: "pdf_edit must contain exactly one operation".to_owned(),
            });
        }

        if let Some(options) = value.merge {
            return Ok(Self::Merge(options));
        }
        if let Some(options) = value.keep_pages {
            return Ok(Self::KeepPages(options));
        }
        if let Some(options) = value.extract_pages {
            return Ok(Self::ExtractPages(options));
        }
        if let Some(options) = value.reorder_pages {
            return Ok(Self::ReorderPages(options));
        }
        if let Some(options) = value.rotate_pages {
            return Ok(Self::RotatePages(options));
        }
        if let Some(options) = value.delete_pages {
            return Ok(Self::DeletePages(options));
        }
        if let Some(options) = value.delete_blank_pages {
            return Ok(Self::DeleteBlankPages(options));
        }
        if let Some(options) = value.crop_pages {
            return Ok(Self::CropPages(options));
        }
        if let Some(options) = value.scale_pages {
            return Ok(Self::ScalePages(options));
        }
        if let Some(options) = value.single_page {
            return Ok(Self::SinglePage(options));
        }
        if let Some(options) = value.nup {
            return Ok(Self::NUp(options));
        }
        if let Some(options) = value.booklet {
            return Ok(Self::Booklet(options));
        }
        if let Some(options) = value.page_numbers {
            return Ok(Self::PageNumbers(options));
        }
        if let Some(options) = value.image_to_pdf {
            return Ok(Self::ImageToPdf(options));
        }
        if let Some(options) = value.svg_to_pdf {
            return Ok(Self::SvgToPdf(options));
        }
        if let Some(options) = value.watermark {
            return Ok(Self::Watermark(options));
        }
        if let Some(options) = value.overlay {
            return Ok(Self::Overlay(options));
        }
        if let Some(options) = value.image_edit {
            return Ok(Self::ImageEdit(options));
        }
        if let Some(options) = value.color {
            return Ok(Self::Color(options));
        }
        if let Some(options) = value.metadata {
            return Ok(Self::Metadata(options));
        }
        if let Some(options) = value.outline {
            return Ok(Self::Outline(options));
        }
        if let Some(options) = value.attachment {
            return Ok(Self::Attachment(options));
        }
        if let Some(options) = value.annotation {
            return Ok(Self::Annotation(options));
        }
        if let Some(options) = value.form_fill {
            return Ok(Self::FormFill(options));
        }
        if value.form_unlock_readonly.is_some() {
            return Ok(Self::FormUnlockReadonly);
        }
        if value.form_remove.is_some() {
            return Ok(Self::FormRemove);
        }
        if let Some(options) = value.interactive_remove {
            return Ok(Self::InteractiveRemove(options));
        }
        if let Some(options) = value.compression {
            return Ok(Self::Compression(options));
        }

        unreachable!("operation count was already checked");
    }
}
