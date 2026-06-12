use thiserror::Error;

/// Structured core error.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum OxideError {
    /// Workflow is invalid.
    #[error("invalid workflow: {reason}")]
    InvalidWorkflow {
        /// Non-sensitive reason.
        reason: String,
    },
    /// Input is invalid.
    #[error("invalid input: {reason}")]
    InvalidInput {
        /// Non-sensitive reason.
        reason: String,
    },
    /// PDF feature is not supported.
    #[error("unsupported PDF feature: {feature}")]
    UnsupportedPdfFeature {
        /// Non-sensitive feature name.
        feature: String,
    },
    /// PDF is encrypted and no usable password was provided.
    #[error("encrypted PDF")]
    EncryptedPdf,
    /// Provided password is incorrect.
    #[error("incorrect password")]
    IncorrectPassword,
    /// PDF parsing failed.
    #[error("PDF parse error")]
    ParsePdf,
    /// PDF writing failed.
    #[error("PDF write error")]
    WritePdf,
    /// PDF rendering failed.
    #[error("PDF render error")]
    RenderPdf,
    /// Text extraction failed.
    #[error("text extraction error")]
    ExtractText,
    /// Font resolution failed.
    #[error("font resolution error")]
    FontResolution,
    /// SVG parsing failed.
    #[error("SVG parse error")]
    SvgParse,
    /// Image decoding failed.
    #[error("image decode error")]
    ImageDecode,
    /// A resource limit was exceeded.
    #[error("resource limit exceeded: {limit}")]
    ResourceLimitExceeded {
        /// Non-sensitive limit name.
        limit: String,
    },
    /// I/O failed.
    #[error("I/O error")]
    Io,
    /// Internal invariant failed.
    #[error("internal error")]
    Internal,
}

impl OxideError {
    /// Stable machine-readable error code for CLI and Web mappings.
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidWorkflow { .. } => "invalid_workflow",
            Self::InvalidInput { .. } => "invalid_input",
            Self::UnsupportedPdfFeature { .. } => "unsupported_pdf_feature",
            Self::EncryptedPdf => "encrypted_pdf",
            Self::IncorrectPassword => "incorrect_password",
            Self::ParsePdf => "parse_pdf",
            Self::WritePdf => "write_pdf",
            Self::RenderPdf => "render_pdf",
            Self::ExtractText => "extract_text",
            Self::FontResolution => "font_resolution",
            Self::SvgParse => "svg_parse",
            Self::ImageDecode => "image_decode",
            Self::ResourceLimitExceeded { .. } => "resource_limit_exceeded",
            Self::Io => "io",
            Self::Internal => "internal",
        }
    }
}
