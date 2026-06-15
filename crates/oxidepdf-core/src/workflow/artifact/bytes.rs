use crate::OxideError;
use std::sync::Arc;

/// Default spill threshold when `ResourceLimits::spill_threshold_bytes` is left
/// at its default. Artifacts larger than this are spilled to a memory-mapped
/// temp file instead of the heap. 64 MiB keeps typical PDFs inline while capping
/// the resident heap footprint of very large inputs and outputs.
pub(crate) const DEFAULT_SPILL_THRESHOLD_BYTES: usize = 64 * 1024 * 1024;

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
    /// returns an artifact storage error if temp file or mapping setup fails.
    pub fn from_vec(bytes: Vec<u8>) -> Result<Self, OxideError> {
        Self::from_vec_with_threshold(bytes, Some(DEFAULT_SPILL_THRESHOLD_BYTES as u64))
    }

    /// Builds a payload from owned bytes, spilling to a memory-mapped temp file
    /// when the size exceeds `threshold`. A `threshold` of `None` keeps the
    /// payload inline regardless of size.
    pub fn from_vec_with_threshold(
        bytes: Vec<u8>,
        threshold: Option<u64>,
    ) -> Result<Self, OxideError> {
        if threshold.is_some_and(|threshold| bytes.len() as u64 > threshold) {
            let mapped = spill_to_mmap(&bytes)?;
            return Ok(Self {
                storage: Storage::Mapped(Arc::new(mapped)),
            });
        }
        Ok(Self {
            storage: Storage::Inline(Arc::from(bytes.into_boxed_slice())),
        })
    }

    /// Builds a payload from copied bytes using the default spill threshold.
    pub fn from_slice(bytes: &[u8]) -> Result<Self, OxideError> {
        Self::from_vec(bytes.to_vec())
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
    pub(super) fn spilled_to_threshold(self, threshold: Option<u64>) -> Result<Self, OxideError> {
        let over_threshold = |len: usize| threshold.is_some_and(|threshold| len as u64 > threshold);
        match &self.storage {
            // Inline but now over the threshold: spill it, propagating storage
            // failures instead of silently keeping oversized payloads inline.
            Storage::Inline(bytes) if over_threshold(bytes.len()) => {
                let mapped = spill_to_mmap(bytes)?;
                Ok(Self {
                    storage: Storage::Mapped(Arc::new(mapped)),
                })
            }
            // Spilled but now within the threshold: pull it back to the heap.
            Storage::Mapped(mapped) if !over_threshold(mapped.as_slice().len()) => Ok(Self {
                storage: Storage::Inline(Arc::from(mapped.as_slice().to_vec().into_boxed_slice())),
            }),
            _ => Ok(self),
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
/// Returns an artifact storage error on any IO or mapping failure.
fn spill_to_mmap(bytes: &[u8]) -> Result<MappedTemp, OxideError> {
    let file = tempfile::NamedTempFile::new().map_err(|_| OxideError::ArtifactStorage)?;
    file.as_file()
        .set_len(bytes.len() as u64)
        .map_err(|_| OxideError::ArtifactStorage)?;
    {
        let mut writable = tiverse_mmap::MmapOptions::new()
            .path(file.path())
            .shared()
            .map_readwrite()
            .map_err(|_| OxideError::ArtifactStorage)?;
        writable.as_mut_slice().copy_from_slice(bytes);
    }
    let mmap = tiverse_mmap::MmapOptions::new()
        .path(file.path())
        .map_readonly()
        .map_err(|_| OxideError::ArtifactStorage)?;
    Ok(MappedTemp { _file: file, mmap })
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

impl TryFrom<Vec<u8>> for ArtifactBytes {
    type Error = OxideError;

    fn try_from(bytes: Vec<u8>) -> Result<Self, Self::Error> {
        Self::from_vec(bytes)
    }
}

impl TryFrom<&[u8]> for ArtifactBytes {
    type Error = OxideError;

    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        Self::from_slice(bytes)
    }
}

impl From<Arc<[u8]>> for ArtifactBytes {
    fn from(bytes: Arc<[u8]>) -> Self {
        Self::from_arc(bytes)
    }
}
