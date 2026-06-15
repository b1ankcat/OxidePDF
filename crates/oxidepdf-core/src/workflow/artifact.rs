mod bytes;
mod kind;
mod store;

pub use bytes::ArtifactBytes;
pub(super) use bytes::DEFAULT_SPILL_THRESHOLD_BYTES;
pub use kind::{
    Artifact, BytesArtifact, ImageArtifact, PdfArtifact, PdfObjectArtifact, SvgArtifact,
    TextArtifact, TextExtractionDiagnostic, TextExtractionDiagnosticCode,
};
pub use store::ArtifactStore;
