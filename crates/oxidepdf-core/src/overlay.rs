use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Options for image-to-PDF conversion.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ImageToPdfOptions {
    /// Layout mode such as `fit`, `fill`, or `original_size`.
    pub layout: Option<String>,
}

/// Options for SVG-to-PDF conversion.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SvgToPdfOptions {
    /// User-selected rasterization mode. Defaults to vector output when false.
    pub rasterize: bool,
}

/// Options for text extraction.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ExtractTextOptions {
    /// Output format, initially `plain`.
    pub format: Option<String>,
}

/// Options for watermarking.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WatermarkOptions {
    /// Watermark kind.
    pub kind: WatermarkKind,
    /// Text for text watermarks.
    pub text: Option<String>,
    /// Font family name discovered via fontdb.
    pub font: Option<String>,
    /// Explicit font file for text watermarks.
    pub font_path: Option<PathBuf>,
    /// Font size in PDF points.
    pub font_size: Option<f32>,
    /// Opacity from 0.0 to 1.0.
    pub opacity: Option<f32>,
    /// Rotation in degrees.
    pub rotation: Option<f32>,
    /// Position such as `center`.
    pub position: Option<String>,
    /// Page range, for example `1,3-5`. Defaults to all pages.
    pub pages: Option<String>,
    /// Scale for image and SVG watermarks.
    pub scale: Option<f32>,
    /// Rasterize SVG before watermarking. Defaults to vector output when false.
    #[serde(default)]
    pub rasterize: bool,
}

/// Watermark content kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WatermarkKind {
    /// Text watermark.
    Text,
    /// Image watermark.
    Image,
    /// SVG watermark.
    Svg,
}

/// Options for rendering.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RenderOptions {
    /// One-based page number.
    pub page: u32,
    /// Optional output format such as `png`.
    pub format: Option<String>,
    /// Optional render scale.
    pub scale: Option<f32>,
}
