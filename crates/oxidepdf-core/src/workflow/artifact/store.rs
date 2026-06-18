use super::kind::Artifact;
use crate::OxideError;
use crate::workflow::ArtifactRef;
use std::collections::HashMap;

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

    /// Re-evaluates every artifact in the store against `threshold`, spilling
    /// oversized inline payloads to temp files and pulling previously-spilled
    /// payloads back inline when they now fit. Called once at workflow startup
    /// so the workflow-configured threshold is applied to inputs that were built
    /// with the default threshold.
    pub(crate) fn rethreshold(&mut self, threshold: Option<u64>) -> Result<(), OxideError> {
        let ids: Vec<ArtifactRef> = self.artifacts.keys().cloned().collect();
        for id in ids {
            if let Some(artifact) = self.artifacts.remove(&id) {
                // Clone is O(1): ArtifactBytes is Arc-backed. Keep a copy so
                // the artifact can be restored if spilling fails (disk pressure).
                let backup = artifact.clone();
                match artifact.spilled_to_threshold(threshold) {
                    Ok(rethresholded) => {
                        self.artifacts.insert(id, rethresholded);
                    }
                    Err(e) => {
                        self.artifacts.insert(id, backup);
                        return Err(e);
                    }
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::ArtifactBytes;

    #[test]
    fn from_vec_with_threshold_keeps_payload_inline_at_or_below_threshold() {
        let bytes = ArtifactBytes::from_vec_with_threshold(vec![1, 2, 3], Some(3)).unwrap();

        assert_eq!(bytes.as_slice(), &[1, 2, 3]);
        assert!(!bytes.is_spilled());
    }

    #[test]
    fn from_vec_with_threshold_spills_payload_above_threshold() {
        let bytes = ArtifactBytes::from_vec_with_threshold(vec![1, 2, 3], Some(2)).unwrap();

        assert_eq!(bytes.as_slice(), &[1, 2, 3]);
        assert!(bytes.is_spilled());
    }

    #[test]
    fn spilled_to_threshold_moves_inline_payload_to_mapping() {
        let bytes = ArtifactBytes::from_vec_with_threshold(vec![4, 5, 6], None).unwrap();

        let spilled = bytes.spilled_to_threshold(Some(0)).unwrap();

        assert_eq!(spilled.as_slice(), &[4, 5, 6]);
        assert!(spilled.is_spilled());
    }

    #[test]
    fn spilled_to_threshold_moves_mapped_payload_back_inline() {
        let bytes = ArtifactBytes::from_vec_with_threshold(vec![7, 8, 9], Some(0)).unwrap();
        assert!(bytes.is_spilled());

        let inline = bytes.spilled_to_threshold(None).unwrap();

        assert_eq!(inline.as_slice(), &[7, 8, 9]);
        assert!(!inline.is_spilled());
    }

    #[test]
    fn write_output_to_returns_bytes_written() {
        let artifact = crate::Artifact::bytes(b"abc").unwrap();
        let mut output = Vec::new();
        let written = artifact
            .write_output_to(&mut output, &crate::ResourceLimits::default())
            .unwrap();

        assert_eq!(written, 3);
        assert_eq!(output, b"abc");
    }

    #[test]
    fn write_output_to_enforces_output_limit() {
        let artifact = crate::Artifact::bytes(b"abc").unwrap();
        let mut output = Vec::new();
        let error = artifact
            .write_output_to(
                &mut output,
                &crate::ResourceLimits {
                    max_output_bytes: Some(2),
                    ..crate::ResourceLimits::default()
                },
            )
            .unwrap_err();

        assert_eq!(
            error,
            crate::OxideError::ResourceLimitExceeded {
                limit: "max_output_bytes".to_owned()
            }
        );
    }
}
