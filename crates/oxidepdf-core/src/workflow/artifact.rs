use super::ArtifactRef;
use crate::{OxideError, save_pdf};
use std::collections::HashMap;
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
    pub fn bytes(bytes: impl AsRef<[u8]>) -> Self {
        Self::Bytes(BytesArtifact {
            bytes: ArtifactBytes::from(bytes.as_ref()),
        })
    }

    /// Creates a PDF artifact.
    pub fn pdf(bytes: impl AsRef<[u8]>) -> Self {
        Self::Pdf(PdfArtifact {
            bytes: ArtifactBytes::from(bytes.as_ref()),
        })
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

    /// Creates an image artifact.
    pub fn image(bytes: impl AsRef<[u8]>) -> Self {
        Self::Image(ImageArtifact {
            bytes: ArtifactBytes::from(bytes.as_ref()),
        })
    }

    /// Creates an SVG artifact.
    pub fn svg(bytes: impl AsRef<[u8]>) -> Self {
        Self::Svg(SvgArtifact {
            bytes: ArtifactBytes::from(bytes.as_ref()),
        })
    }

    /// Applies a workflow's configured spill threshold to this artifact's
    /// payload. Byte-backed artifacts produced during execution are built with
    /// the default threshold; this re-evaluates them against the workflow's
    /// threshold. Text and parsed-object artifacts carry no `ArtifactBytes` and
    /// pass through.
    pub(super) fn spilled_to_threshold(self, threshold: Option<u64>) -> Self {
        match self {
            Self::Pdf(artifact) => Self::Pdf(PdfArtifact {
                bytes: artifact.bytes.spilled_to_threshold(threshold),
            }),
            Self::Image(artifact) => Self::Image(ImageArtifact {
                bytes: artifact.bytes.spilled_to_threshold(threshold),
            }),
            Self::Svg(artifact) => Self::Svg(SvgArtifact {
                bytes: artifact.bytes.spilled_to_threshold(threshold),
            }),
            Self::Bytes(artifact) => Self::Bytes(BytesArtifact {
                bytes: artifact.bytes.spilled_to_threshold(threshold),
            }),
            // The parsed object tree is already an in-memory structure, not a
            // byte buffer, so spill thresholds do not apply.
            Self::PdfObject(artifact) => Self::PdfObject(artifact),
            Self::Text(artifact) => Self::Text(artifact),
        }
    }
}

/// Default spill threshold when `ResourceLimits::spill_threshold_bytes` is left
/// at its default. Artifacts larger than this are spilled to a memory-mapped
/// temp file instead of the heap. 64 MiB keeps typical PDFs inline while capping
/// the resident heap footprint of very large inputs and outputs.
pub(super) const DEFAULT_SPILL_THRESHOLD_BYTES: usize = 64 * 1024 * 1024;

/// Reference-counted artifact payload.
///
/// Cloning an `ArtifactBytes` only bumps an atomic reference count; no bytes are
/// copied. This lets the executor hand the same large PDF to several tasks, and
/// keep inputs in the store, without duplicating multi-hundred-megabyte buffers.
///
/// Payloads above the spill threshold are spilled to a memory-mapped temp
/// file so they do not occupy heap; smaller payloads stay inline. Both forms
/// expose the same `&[u8]` view, so consumers never observe the difference.
#[derive(Clone)]
pub struct ArtifactBytes {
    storage: Storage,
}

#[derive(Clone)]
enum Storage {
    /// Heap-resident payload.
    Inline(Arc<[u8]>),
    /// Payload backed by a memory-mapped temporary file.
    Mapped(Arc<MappedTemp>),
}

/// A temp file plus its read-only memory mapping. The file is kept alive
/// alongside the mapping and removed when the last reference is dropped.
///
/// The stored mapping is read-only so `ArtifactBytes` stays `Sync` and can be
/// shared across the parallel executor; the payload is written once through a
/// transient writable mapping during [`spill_to_mmap`].
struct MappedTemp {
    // Held so the backing file outlives the mapping and is cleaned up on drop.
    _file: tempfile::NamedTempFile,
    mmap: tiverse_mmap::Mmap<tiverse_mmap::ReadOnly>,
}

impl MappedTemp {
    fn as_slice(&self) -> &[u8] {
        &self.mmap
    }
}

impl ArtifactBytes {
    /// Returns the payload length in bytes.
    pub fn len(&self) -> usize {
        self.as_slice().len()
    }

    /// Returns whether the payload is empty.
    pub fn is_empty(&self) -> bool {
        self.as_slice().is_empty()
    }

    /// Returns the payload as a byte slice.
    pub fn as_slice(&self) -> &[u8] {
        match &self.storage {
            Storage::Inline(bytes) => bytes,
            Storage::Mapped(mapped) => mapped.as_slice(),
        }
    }

    /// Returns whether the payload is currently spilled to a temp file.
    /// Primarily intended for tests and diagnostics.
    pub fn is_spilled(&self) -> bool {
        matches!(self.storage, Storage::Mapped(_))
    }

    /// Builds a payload from owned bytes using the default spill threshold,
    /// spilling to a memory-mapped temp file when the size exceeds it. Spilling
    /// is best-effort: if a temp file cannot be created or mapped, the payload
    /// stays inline.
    fn from_vec(bytes: Vec<u8>) -> Self {
        Self::from_vec_with_threshold(bytes, Some(DEFAULT_SPILL_THRESHOLD_BYTES as u64))
    }

    /// Builds a payload from owned bytes, spilling to a memory-mapped temp file
    /// when the size exceeds `threshold`. A `threshold` of `None` keeps the
    /// payload inline regardless of size. Spilling is best-effort: if a temp
    /// file cannot be created or mapped, the payload stays inline.
    fn from_vec_with_threshold(bytes: Vec<u8>, threshold: Option<u64>) -> Self {
        if threshold.is_some_and(|threshold| bytes.len() as u64 > threshold) {
            if let Some(mapped) = spill_to_mmap(&bytes) {
                return Self {
                    storage: Storage::Mapped(Arc::new(mapped)),
                };
            }
        }
        Self {
            storage: Storage::Inline(Arc::from(bytes.into_boxed_slice())),
        }
    }

    /// Builds a payload from an existing `Arc<[u8]>` without copying or spilling.
    ///
    /// Use this when the caller already owns a reference-counted buffer (for
    /// example one shared from another `ArtifactBytes` or a custom source): the
    /// `Arc` is adopted directly, so no allocation or copy occurs. Because the
    /// buffer is already heap-resident and shared, it is kept inline rather than
    /// spilled — spilling an existing `Arc` would copy the very bytes this path
    /// exists to avoid copying.
    pub fn from_arc(bytes: Arc<[u8]>) -> Self {
        Self {
            storage: Storage::Inline(bytes),
        }
    }

    /// Reconciles the payload's storage with `threshold`: spills an oversized
    /// inline payload to a memory-mapped temp file, and pulls a spilled payload
    /// back inline when it now fits within the threshold. This is where a
    /// workflow's configured spill threshold is applied to artifacts produced
    /// during execution, which operators build with the default threshold — so
    /// raising or lowering the threshold both take effect. A `threshold` of
    /// `None` keeps everything inline.
    fn spilled_to_threshold(self, threshold: Option<u64>) -> Self {
        let over_threshold = |len: usize| threshold.is_some_and(|threshold| len as u64 > threshold);
        match &self.storage {
            // Inline but now over the threshold: spill it. Best-effort — a spill
            // failure leaves it inline (a placement choice, not a behavior).
            Storage::Inline(bytes) if over_threshold(bytes.len()) => match spill_to_mmap(bytes) {
                Some(mapped) => Self {
                    storage: Storage::Mapped(Arc::new(mapped)),
                },
                None => self,
            },
            // Spilled but now within the threshold: pull it back to the heap.
            Storage::Mapped(mapped) if !over_threshold(mapped.as_slice().len()) => Self {
                storage: Storage::Inline(Arc::from(mapped.as_slice().to_vec().into_boxed_slice())),
            },
            _ => self,
        }
    }
}

/// Spills `bytes` to a temp file, writing through a single memory mapping.
///
/// The file is pre-sized with `set_len`, mapped `MAP_SHARED` read-write, and the
/// payload is copied straight into the mapped region — so the bytes are placed
/// once, in the pages backing the file, instead of being staged in the kernel
/// write cache by a `write()` and then mapped back in. The writable mapping is
/// then dropped and the populated file is re-mapped read-only for storage, which
/// keeps `ArtifactBytes` `Sync` (a read-write mapping is not) so it can be shared
/// across the parallel executor.
///
/// Returns `None` on any IO or mapping failure; this is a memory-placement
/// optimization, not a behavioral path, so the caller keeps the payload inline
/// instead.
fn spill_to_mmap(bytes: &[u8]) -> Option<MappedTemp> {
    let file = tempfile::NamedTempFile::new().ok()?;
    file.as_file().set_len(bytes.len() as u64).ok()?;
    {
        let mut writable = tiverse_mmap::MmapOptions::new()
            .path(file.path())
            .shared()
            .map_readwrite()
            .ok()?;
        writable.as_mut_slice().copy_from_slice(bytes);
    }
    let mmap = tiverse_mmap::MmapOptions::new()
        .path(file.path())
        .map_readonly()
        .ok()?;
    Some(MappedTemp { _file: file, mmap })
}

impl std::fmt::Debug for ArtifactBytes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ArtifactBytes")
            .field("len", &self.len())
            .field("spilled", &self.is_spilled())
            .finish()
    }
}

impl PartialEq for ArtifactBytes {
    fn eq(&self, other: &Self) -> bool {
        self.as_slice() == other.as_slice()
    }
}

impl Eq for ArtifactBytes {}

impl std::ops::Deref for ArtifactBytes {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl AsRef<[u8]> for ArtifactBytes {
    fn as_ref(&self) -> &[u8] {
        self.as_slice()
    }
}

impl From<Vec<u8>> for ArtifactBytes {
    fn from(bytes: Vec<u8>) -> Self {
        Self::from_vec(bytes)
    }
}

impl From<&[u8]> for ArtifactBytes {
    fn from(bytes: &[u8]) -> Self {
        Self::from_vec(bytes.to_vec())
    }
}

impl From<Arc<[u8]>> for ArtifactBytes {
    fn from(bytes: Arc<[u8]>) -> Self {
        Self::from_arc(bytes)
    }
}

/// PDF artifact placeholder for later operators.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdfArtifact {
    /// Serialized PDF bytes. Produced by operators that have not yet migrated to
    /// the object-level path, and at the workflow output boundary.
    pub bytes: ArtifactBytes,
}

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

/// In-memory artifact store used by the executor.
///
/// Lookups are keyed by `ArtifactRef` with no ordering requirement, so a
/// `HashMap` gives O(1) amortized insert/get/remove.
///
/// `Eq` is not derived because `Artifact` is only `PartialEq` (its `PdfObject`
/// variant wraps a non-`Eq` parsed document).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ArtifactStore {
    artifacts: HashMap<ArtifactRef, Artifact>,
}

impl ArtifactStore {
    /// Creates an empty store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts or replaces an artifact.
    pub fn insert(&mut self, id: ArtifactRef, artifact: Artifact) -> Option<Artifact> {
        self.artifacts.insert(id, artifact)
    }

    /// Returns an artifact by id.
    pub fn get(&self, id: &ArtifactRef) -> Option<&Artifact> {
        self.artifacts.get(id)
    }

    /// Removes an artifact, returning it if present.
    ///
    /// The executor calls this to evict an artifact once its last consumer has
    /// run and no output references it, keeping peak memory close to the working
    /// set rather than the full set of every artifact ever produced.
    pub fn remove(&mut self, id: &ArtifactRef) -> Option<Artifact> {
        self.artifacts.remove(id)
    }
}
