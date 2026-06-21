use super::bytes::ArtifactBytes;
use crate::{OxideError, save_pdf};
use std::io::{self, Write};
use std::sync::Arc;

/// Artifact produced or consumed by workflow tasks.
///
/// `Eq` is intentionally not derived: the `PdfObject` variant wraps an
/// `Arc<lopdf::Document>`, which implements neither `PartialEq` nor `Eq`.
/// Equality is provided by a hand-written `PartialEq` that compares parsed
/// documents by shared identity (`Arc::ptr_eq`).
#[derive(Debug, Clone)]
pub enum Artifact {
    /// PDF artifact placeholder.
    Pdf(PdfArtifact),
    /// Parsed PDF object tree, shared across chained operators without
    /// re-serializing between steps.
    PdfObject(PdfObjectArtifact),
    /// Image artifact placeholder.
    Image(ImageArtifact),
    /// Text artifact.
    Text(TextArtifact),
    /// SVG artifact.
    Svg(SvgArtifact),
    /// Raw bytes.
    Bytes(BytesArtifact),
}

impl PartialEq for Artifact {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Pdf(left), Self::Pdf(right)) => left == right,
            // Parsed documents are not structurally comparable; two object
            // artifacts are equal only when they share the same backing tree.
            (Self::PdfObject(left), Self::PdfObject(right)) => {
                Arc::ptr_eq(&left.document, &right.document)
            }
            (Self::Image(left), Self::Image(right)) => left == right,
            (Self::Text(left), Self::Text(right)) => left == right,
            (Self::Svg(left), Self::Svg(right)) => left == right,
            (Self::Bytes(left), Self::Bytes(right)) => left == right,
            _ => false,
        }
    }
}

impl Artifact {
    /// Creates a raw byte artifact.
    pub fn bytes(bytes: impl AsRef<[u8]>) -> Result<Self, OxideError> {
        Ok(Self::Bytes(BytesArtifact {
            bytes: ArtifactBytes::from_slice(bytes.as_ref())?,
        }))
    }

    /// Creates a PDF artifact.
    pub fn pdf(bytes: impl AsRef<[u8]>) -> Result<Self, OxideError> {
        Ok(Self::Pdf(PdfArtifact {
            bytes: ArtifactBytes::from_slice(bytes.as_ref())?,
        }))
    }

    /// Creates a parsed-PDF object artifact from an owned document.
    pub fn pdf_object(document: lopdf::Document) -> Self {
        Self::PdfObject(PdfObjectArtifact {
            document: Arc::new(document),
        })
    }

    /// Returns the artifact's bytes for materialization at an output boundary.
    ///
    /// Byte-backed artifacts borrow their buffer; a parsed object tree is
    /// serialized here (the single serialization point for the object-level
    /// path). Cloning the document out of the shared `Arc` is required because
    /// serialization renumbers and prunes objects, which needs ownership.
    pub fn output_bytes(&self) -> Result<std::borrow::Cow<'_, [u8]>, OxideError> {
        match self {
            Self::Pdf(pdf) => Ok(std::borrow::Cow::Borrowed(pdf.bytes.as_slice())),
            Self::Image(image) => Ok(std::borrow::Cow::Borrowed(image.bytes.as_slice())),
            Self::Svg(svg) => Ok(std::borrow::Cow::Borrowed(svg.bytes.as_slice())),
            Self::Bytes(bytes) => Ok(std::borrow::Cow::Borrowed(bytes.bytes.as_slice())),
            Self::Text(text) => Ok(std::borrow::Cow::Borrowed(text.text.as_bytes())),
            Self::PdfObject(artifact) => {
                let document = (*artifact.document).clone();
                Ok(std::borrow::Cow::Owned(save_pdf(document)?))
            }
        }
    }

    /// Writes this artifact to an output boundary without first materializing
    /// byte-backed artifacts into a new buffer.
    pub fn write_output_to(
        &self,
        writer: impl Write,
        limits: &crate::ResourceLimits,
    ) -> Result<u64, OxideError> {
        let mut writer = LimitWriter::new(writer, limits.max_output_bytes);
        match self {
            Self::Pdf(pdf) => writer.write_all(&pdf.bytes).map_err(write_error)?,
            Self::Image(image) => writer.write_all(&image.bytes).map_err(write_error)?,
            Self::Svg(svg) => writer.write_all(&svg.bytes).map_err(write_error)?,
            Self::Bytes(bytes) => writer.write_all(&bytes.bytes).map_err(write_error)?,
            Self::Text(text) => writer
                .write_all(text.text.as_bytes())
                .map_err(write_error)?,
            Self::PdfObject(artifact) => {
                let mut document = (*artifact.document).clone();
                document.prune_objects();
                document.renumber_objects();
                if document.save_to(&mut writer).is_err() {
                    if writer.limit_exceeded() {
                        return Err(crate::resource_limit("max_output_bytes"));
                    }
                    return Err(OxideError::WritePdf);
                }
            }
        }
        Ok(writer.bytes_written())
    }

    /// Creates an image artifact.
    pub fn image(bytes: impl AsRef<[u8]>) -> Result<Self, OxideError> {
        Ok(Self::Image(ImageArtifact {
            bytes: ArtifactBytes::from_slice(bytes.as_ref())?,
        }))
    }

    /// Creates an SVG artifact.
    pub fn svg(bytes: impl AsRef<[u8]>) -> Result<Self, OxideError> {
        Ok(Self::Svg(SvgArtifact {
            bytes: ArtifactBytes::from_slice(bytes.as_ref())?,
        }))
    }

    /// Applies a workflow's configured spill threshold to this artifact's
    /// payload. Byte-backed artifacts produced during execution are built with
    /// the default threshold; this re-evaluates them against the workflow's
    /// threshold. Text and parsed-object artifacts carry no `ArtifactBytes` and
    /// pass through.
    pub(crate) fn spilled_to_threshold(self, threshold: Option<u64>) -> Result<Self, OxideError> {
        match self {
            Self::Pdf(artifact) => Ok(Self::Pdf(PdfArtifact {
                bytes: artifact.bytes.spilled_to_threshold(threshold)?,
            })),
            Self::Image(artifact) => Ok(Self::Image(ImageArtifact {
                bytes: artifact.bytes.spilled_to_threshold(threshold)?,
            })),
            Self::Svg(artifact) => Ok(Self::Svg(SvgArtifact {
                bytes: artifact.bytes.spilled_to_threshold(threshold)?,
            })),
            Self::Bytes(artifact) => Ok(Self::Bytes(BytesArtifact {
                bytes: artifact.bytes.spilled_to_threshold(threshold)?,
            })),
            // The parsed object tree is already an in-memory structure, not a
            // byte buffer, so spill thresholds do not apply.
            Self::PdfObject(artifact) => Ok(Self::PdfObject(artifact)),
            Self::Text(artifact) => Ok(Self::Text(artifact)),
        }
    }
}

struct LimitWriter<W> {
    inner: W,
    max_output_bytes: Option<u64>,
    written: u64,
    limit_exceeded: bool,
}

impl<W> LimitWriter<W> {
    fn new(inner: W, max_output_bytes: Option<u64>) -> Self {
        Self {
            inner,
            max_output_bytes,
            written: 0,
            limit_exceeded: false,
        }
    }

    fn bytes_written(&self) -> u64 {
        self.written
    }

    fn limit_exceeded(&self) -> bool {
        self.limit_exceeded
    }
}

impl<W: Write> Write for LimitWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let next = self
            .written
            .checked_add(buf.len() as u64)
            .ok_or_else(limit_io_error)?;
        if self.max_output_bytes.is_some_and(|limit| next > limit) {
            self.limit_exceeded = true;
            return Err(limit_io_error());
        }
        let written = self.inner.write(buf)?;
        self.written = self
            .written
            .checked_add(written as u64)
            .ok_or_else(limit_io_error)?;
        if self
            .max_output_bytes
            .is_some_and(|limit| self.written > limit)
        {
            self.limit_exceeded = true;
            return Err(limit_io_error());
        }
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

fn limit_io_error() -> io::Error {
    io::Error::other("max_output_bytes")
}

fn write_error(error: io::Error) -> OxideError {
    if error.kind() == io::ErrorKind::Other && error.to_string() == "max_output_bytes" {
        return crate::resource_limit("max_output_bytes");
    }
    OxideError::WritePdf
}

/// Default spill threshold when `ResourceLimits::spill_threshold_bytes` is left
/// PDF artifact placeholder for later operators.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdfArtifact {
    /// Serialized PDF bytes. Produced by operators that have not yet migrated to
    /// the object-level path, and at the workflow output boundary.
    pub bytes: ArtifactBytes,
}
