use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct CompressionOptions {
    pub mode: CompressionMode,
    pub images: Option<CompressionImageOptions>,
}

impl Default for CompressionOptions {
    fn default() -> Self {
        Self {
            mode: CompressionMode::Lossless,
            images: None,
        }
    }
}

#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum CompressionMode {
    #[default]
    Lossless,
    Lossy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CompressionImageOptions {
    pub quality: Option<u8>,
    pub max_width: Option<u32>,
    pub max_height: Option<u32>,
    pub format: Option<CompressionImageFormat>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CompressionImageFormat {
    Jpeg,
    Png,
    Webp,
}
