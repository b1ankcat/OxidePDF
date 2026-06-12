#![forbid(unsafe_code)]
#![doc = "Core contracts and shared logic for OxidePDF."]

/// Current workflow schema version.
///
/// Stage 1 only establishes the crate boundary. Stage 2 will add the full
/// serialized workflow contract around this version.
pub const WORKFLOW_SCHEMA_VERSION: u16 = 1;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workflow_schema_version_starts_at_one() {
        assert_eq!(WORKFLOW_SCHEMA_VERSION, 1);
    }
}
