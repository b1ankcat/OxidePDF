
/// Parsed PDF object tree shared across chained operators.
///
/// Holding the document as an `Arc<lopdf::Document>` lets several tasks read the
/// same parsed tree without re-parsing, and lets a chain of object-level
/// operators pass the document along without serializing to bytes and parsing
/// again between every step. Serialization happens once, at the output boundary.
#[derive(Debug, Clone)]
pub struct PdfObjectArtifact {
    /// The shared, parsed PDF document.
    pub document: Arc<lopdf::Document>,
}

/// Image artifact placeholder for later operators.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageArtifact {
    /// Encoded image bytes.
    pub bytes: ArtifactBytes,
}

/// Text artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextArtifact {
    /// Extracted or generated text.
    pub text: String,
    /// Page-level extraction diagnostics reserved for structured output.
    pub diagnostics: Vec<TextExtractionDiagnostic>,
}

/// Page-level diagnostic emitted by text extraction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextExtractionDiagnostic {
    /// One-based page number.
    pub page: u32,
    /// Stable diagnostic code.
    pub code: TextExtractionDiagnosticCode,
    /// Non-sensitive diagnostic message.
    pub message: String,
}

/// Stable text extraction diagnostic code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextExtractionDiagnosticCode {
    /// Page has no extractable text layer.
    NoTextLayer,
}

/// SVG artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SvgArtifact {
    /// SVG document bytes.
    pub bytes: ArtifactBytes,
}

/// Byte artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BytesArtifact {
    /// Raw bytes.
    pub bytes: ArtifactBytes,
}
