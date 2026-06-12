use serde::{Deserialize, Serialize};

/// PDF password, encryption, and permission operations.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PdfSecurityOptions {
    /// Explicit operation name. Stage 18 implements concrete operations.
    pub operation: String,
}
