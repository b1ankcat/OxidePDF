use crate::workflow::ResourceLimits;
use crate::{
    add_pdf_page_numbers_with_limits, booklet_pdf_pages_with_limits, crop_pdf_pages_with_limits,
    delete_blank_pdf_pages_with_limits, delete_pdf_pages_with_limits,
    extract_pdf_pages_with_limits, extract_text_from_pdf, image_artifacts_to_pdf,
    merge_pdf_artifacts_with_limits, nup_pdf_pages_with_limits, pdf_bytes,
    pdf_to_single_page_with_limits, render_pdf_page, reorder_pdf_with_limits,
    rotate_pdf_with_limits, scale_pdf_pages_with_limits, split_pdf_with_limits, svg_to_pdf,
    verify_pdf_signatures, watermark_pdf_artifacts, Artifact, BookletOptions, CropPagesOptions,
    DeleteBlankPagesOptions, ExtractTextOptions, ImageToPdfOptions, MergeOptions, NUpOptions,
    OxideError, PageNumbersOptions, PageSelectionOptions, PdfCompareOptions, PdfSecurityOptions,
    RenderOptions, ReorderOptions, RotateOptions, ScalePagesOptions, SignatureOptions,
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

        unreachable!("operation count was already checked");
    }
}

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
        }
    }
}

/// PDF inspection operations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "PdfInspectOptionsDef", into = "PdfInspectOptionsDef")]
pub enum PdfInspectOptions {
    /// Render PDF pages to images.
    Render(RenderOptions),
    /// Extract text from a PDF.
    ExtractText(ExtractTextOptions),
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct PdfInspectOptionsDef {
    render: Option<RenderOptions>,
    extract_text: Option<ExtractTextOptions>,
}

impl TryFrom<PdfInspectOptionsDef> for PdfInspectOptions {
    type Error = OxideError;

    fn try_from(value: PdfInspectOptionsDef) -> Result<Self, Self::Error> {
        let operation_count = [value.render.is_some(), value.extract_text.is_some()]
            .into_iter()
            .filter(|present| *present)
            .count();

        if operation_count != 1 {
            return Err(OxideError::InvalidWorkflow {
                reason: "pdf_inspect must contain exactly one operation".to_owned(),
            });
        }

        if let Some(options) = value.render {
            return Ok(Self::Render(options));
        }
        if let Some(options) = value.extract_text {
            return Ok(Self::ExtractText(options));
        }

        unreachable!("operation count was already checked");
    }
}

impl From<PdfInspectOptions> for PdfInspectOptionsDef {
    fn from(value: PdfInspectOptions) -> Self {
        match value {
            PdfInspectOptions::Render(options) => Self {
                render: Some(options),
                ..Self::default()
            },
            PdfInspectOptions::ExtractText(options) => Self {
                extract_text: Some(options),
                ..Self::default()
            },
        }
    }
}

/// PDF signing and signature verification operations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "PdfSignOptionsDef", into = "PdfSignOptionsDef")]
pub enum PdfSignOptions {
    /// Verify PDF signatures and certificate material.
    Verify(SignatureOptions),
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct PdfSignOptionsDef {
    verify: Option<SignatureOptions>,
}

impl TryFrom<PdfSignOptionsDef> for PdfSignOptions {
    type Error = OxideError;

    fn try_from(value: PdfSignOptionsDef) -> Result<Self, Self::Error> {
        if let Some(options) = value.verify {
            return Ok(Self::Verify(options));
        }

        Err(OxideError::InvalidWorkflow {
            reason: "pdf_sign must contain exactly one operation".to_owned(),
        })
    }
}

impl From<PdfSignOptions> for PdfSignOptionsDef {
    fn from(value: PdfSignOptions) -> Self {
        match value {
            PdfSignOptions::Verify(options) => Self {
                verify: Some(options),
            },
        }
    }
}

pub(crate) fn run_pdf_edit(
    options: &PdfEditOptions,
    inputs: &[Artifact],
    limits: &ResourceLimits,
) -> Result<Artifact, OxideError> {
    match options {
        PdfEditOptions::Merge(_) => {
            merge_pdf_artifacts_with_limits(inputs, limits).map(Artifact::Pdf)
        }
        PdfEditOptions::KeepPages(options) => {
            let input = single_pdf_input(inputs)?;
            split_pdf_with_limits(input, &options.pages, limits).map(Artifact::Pdf)
        }
        PdfEditOptions::ExtractPages(options) => {
            let input = single_pdf_input(inputs)?;
            extract_pdf_pages_with_limits(input, &options.pages, limits).map(Artifact::Pdf)
        }
        PdfEditOptions::ReorderPages(options) => {
            let input = single_pdf_input(inputs)?;
            reorder_pdf_with_limits(input, &options.pages, limits).map(Artifact::Pdf)
        }
        PdfEditOptions::RotatePages(options) => {
            let input = single_pdf_input(inputs)?;
            rotate_pdf_with_limits(input, &options.pages, options.degrees, limits)
                .map(Artifact::Pdf)
        }
        PdfEditOptions::DeletePages(options) => {
            let input = single_pdf_input(inputs)?;
            delete_pdf_pages_with_limits(input, &options.pages, limits).map(Artifact::Pdf)
        }
        PdfEditOptions::DeleteBlankPages(options) => {
            let input = single_pdf_input(inputs)?;
            delete_blank_pdf_pages_with_limits(input, options, limits).map(Artifact::Pdf)
        }
        PdfEditOptions::CropPages(options) => {
            let input = single_pdf_input(inputs)?;
            crop_pdf_pages_with_limits(input, options, limits).map(Artifact::Pdf)
        }
        PdfEditOptions::ScalePages(options) => {
            let input = single_pdf_input(inputs)?;
            scale_pdf_pages_with_limits(input, options, limits).map(Artifact::Pdf)
        }
        PdfEditOptions::SinglePage(options) => {
            let input = single_pdf_input(inputs)?;
            pdf_to_single_page_with_limits(input, options, limits).map(Artifact::Pdf)
        }
        PdfEditOptions::NUp(options) => {
            let input = single_pdf_input(inputs)?;
            nup_pdf_pages_with_limits(input, options, limits).map(Artifact::Pdf)
        }
        PdfEditOptions::Booklet(options) => {
            let input = single_pdf_input(inputs)?;
            booklet_pdf_pages_with_limits(input, options, limits).map(Artifact::Pdf)
        }
        PdfEditOptions::PageNumbers(options) => {
            let input = single_pdf_input(inputs)?;
            add_pdf_page_numbers_with_limits(input, options, limits).map(Artifact::Pdf)
        }
        PdfEditOptions::ImageToPdf(options) => {
            image_artifacts_to_pdf(inputs, options, limits).map(Artifact::Pdf)
        }
        PdfEditOptions::SvgToPdf(options) => {
            let input = single_svg_input(inputs)?;
            svg_to_pdf(input, options, limits).map(Artifact::Pdf)
        }
        PdfEditOptions::Watermark(options) => {
            watermark_pdf_artifacts(inputs, options, limits).map(Artifact::Pdf)
        }
    }
}

pub(crate) fn run_pdf_inspect(
    options: &PdfInspectOptions,
    inputs: &[Artifact],
    limits: &ResourceLimits,
) -> Result<Artifact, OxideError> {
    match options {
        PdfInspectOptions::Render(options) => {
            let input = single_pdf_input(inputs)?;
            render_pdf_page(input, options, limits).map(Artifact::Image)
        }
        PdfInspectOptions::ExtractText(options) => {
            let input = single_pdf_input(inputs)?;
            extract_text_from_pdf(input, options, limits).map(Artifact::Text)
        }
    }
}

pub(crate) fn run_pdf_security(options: &PdfSecurityOptions) -> Result<Artifact, OxideError> {
    Err(OxideError::UnsupportedPdfFeature {
        feature: format!("pdf_security operation '{}'", options.operation),
    })
}

pub(crate) fn run_pdf_compare(options: &PdfCompareOptions) -> Result<Artifact, OxideError> {
    Err(OxideError::UnsupportedPdfFeature {
        feature: format!("pdf_compare mode '{}'", options.mode),
    })
}

pub(crate) fn run_pdf_sign(
    options: &PdfSignOptions,
    inputs: &[Artifact],
    limits: &ResourceLimits,
) -> Result<Artifact, OxideError> {
    match options {
        PdfSignOptions::Verify(options) => {
            let input = single_pdf_input(inputs)?;
            verify_pdf_signatures(input, options, limits).map(Artifact::Text)
        }
    }
}

fn single_pdf_input(inputs: &[Artifact]) -> Result<&[u8], OxideError> {
    if inputs.len() != 1 {
        return Err(OxideError::InvalidInput {
            reason: "operator requires exactly one PDF input".to_owned(),
        });
    }

    pdf_bytes(&inputs[0])
}

fn single_svg_input(inputs: &[Artifact]) -> Result<&[u8], OxideError> {
    if inputs.len() != 1 {
        return Err(OxideError::InvalidInput {
            reason: "svg2pdf requires exactly one SVG input".to_owned(),
        });
    }

    match &inputs[0] {
        Artifact::Svg(svg) => Ok(&svg.bytes),
        Artifact::Bytes(bytes) => Ok(&bytes.bytes),
        _ => Err(OxideError::InvalidInput {
            reason: "expected SVG input artifact".to_owned(),
        }),
    }
}
