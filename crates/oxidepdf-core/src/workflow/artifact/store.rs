use super::kind::Artifact;
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
}
