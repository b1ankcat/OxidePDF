use crate::{
    AnnotationInspectOptions, AttachmentExtractOptions, AttachmentInspectOptions,
    ExtractTextOptions, FormInspectOptions, ImageExtractOptions, ImageInspectOptions,
    MetadataInspectOptions, OutlineInspectOptions, OxideError, RenderOptions,
};
use serde::{Deserialize, Serialize};

/// PDF inspection operations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "PdfInspectOptionsDef", into = "PdfInspectOptionsDef")]
pub enum PdfInspectOptions {
    /// Render PDF pages to images.
    Render(RenderOptions),
    /// Extract text from a PDF.
    ExtractText(ExtractTextOptions),
    /// Inspect document metadata.
    Metadata(MetadataInspectOptions),
    /// Inspect outline tree.
    Outline(OutlineInspectOptions),
    /// Inspect embedded file attachments.
    Attachments(AttachmentInspectOptions),
    /// Extract an embedded file attachment.
    AttachmentExtract(AttachmentExtractOptions),
    /// Inspect annotations.
    Annotations(AnnotationInspectOptions),
    /// Inspect AcroForm fields.
    Forms(FormInspectOptions),
    /// Inspect page image XObject resources.
    Images(ImageInspectOptions),
    /// Extract one image XObject resource.
    ImageExtract(ImageExtractOptions),
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct PdfInspectOptionsDef {
    render: Option<RenderOptions>,
    extract_text: Option<ExtractTextOptions>,
    metadata: Option<MetadataInspectOptions>,
    outline: Option<OutlineInspectOptions>,
    attachments: Option<AttachmentInspectOptions>,
    attachment_extract: Option<AttachmentExtractOptions>,
    annotations: Option<AnnotationInspectOptions>,
    forms: Option<FormInspectOptions>,
    images: Option<ImageInspectOptions>,
    image_extract: Option<ImageExtractOptions>,
}

impl TryFrom<PdfInspectOptionsDef> for PdfInspectOptions {
    type Error = OxideError;

    fn try_from(value: PdfInspectOptionsDef) -> Result<Self, Self::Error> {
        let operation_count = [
            value.render.is_some(),
            value.extract_text.is_some(),
            value.metadata.is_some(),
            value.outline.is_some(),
            value.attachments.is_some(),
            value.attachment_extract.is_some(),
            value.annotations.is_some(),
            value.forms.is_some(),
            value.images.is_some(),
            value.image_extract.is_some(),
        ]
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
        if let Some(options) = value.metadata {
            return Ok(Self::Metadata(options));
        }
        if let Some(options) = value.outline {
            return Ok(Self::Outline(options));
        }
        if let Some(options) = value.attachments {
            return Ok(Self::Attachments(options));
        }
        if let Some(options) = value.attachment_extract {
            return Ok(Self::AttachmentExtract(options));
        }
        if let Some(options) = value.annotations {
            return Ok(Self::Annotations(options));
        }
        if let Some(options) = value.forms {
            return Ok(Self::Forms(options));
        }
        if let Some(options) = value.images {
            return Ok(Self::Images(options));
        }
        if let Some(options) = value.image_extract {
            return Ok(Self::ImageExtract(options));
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
            PdfInspectOptions::Metadata(options) => Self {
                metadata: Some(options),
                ..Self::default()
            },
            PdfInspectOptions::Outline(options) => Self {
                outline: Some(options),
                ..Self::default()
            },
            PdfInspectOptions::Attachments(options) => Self {
                attachments: Some(options),
                ..Self::default()
            },
            PdfInspectOptions::AttachmentExtract(options) => Self {
                attachment_extract: Some(options),
                ..Self::default()
            },
            PdfInspectOptions::Annotations(options) => Self {
                annotations: Some(options),
                ..Self::default()
            },
            PdfInspectOptions::Forms(options) => Self {
                forms: Some(options),
                ..Self::default()
            },
            PdfInspectOptions::Images(options) => Self {
                images: Some(options),
                ..Self::default()
            },
            PdfInspectOptions::ImageExtract(options) => Self {
                image_extract: Some(options),
                ..Self::default()
            },
        }
    }
}
