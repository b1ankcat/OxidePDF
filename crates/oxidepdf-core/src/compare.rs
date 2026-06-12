use serde::{Deserialize, Serialize};

/// PDF comparison operations.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PdfCompareOptions {
    /// Explicit comparison mode. Stage 19 implements concrete modes.
    pub mode: String,
}
