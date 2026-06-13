//! Web front end for OxidePDF.
//!
//! Placeholder crate. The browser/server-facing interface that drives the
//! `oxidepdf-core` workflow engine will live here. It is intentionally empty
//! for now so the workspace, licensing, and CI wiring are in place before the
//! implementation lands.

/// Returns the crate name. Exists only so the placeholder library has a
/// reachable, testable symbol until the real surface is added.
#[must_use]
pub fn name() -> &'static str {
    "oxidepdf-web"
}

#[cfg(test)]
mod tests {
    use super::name;

    #[test]
    fn name_is_crate_name() {
        assert_eq!(name(), "oxidepdf-web");
    }
}
