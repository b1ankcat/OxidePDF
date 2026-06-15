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

/// Unified overlay content kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OverlayKind {
    /// Existing watermark semantics.
    Watermark,
    /// Stamp text or image appearance.
    Stamp,
    /// Visual signature appearance only; does not create a digital signature.
    SignatureAppearance,
    /// Overlay a page from a second PDF.
    PdfPage,
    /// Text overlay.
    Text,
    /// Image overlay.
    Image,
    /// SVG overlay.
    Svg,
}

/// Options for the unified overlay engine.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OverlayOptions {
    /// Overlay kind.
    pub kind: OverlayKind,
    /// Text for text, stamp, signature appearance, or watermark overlays.
    pub text: Option<String>,
    /// Font family name discovered via fontdb.
    pub font: Option<String>,
    /// Explicit font file for text overlays.
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
    /// Scale for image, SVG, and PDF page overlays.
    pub scale: Option<f32>,
    /// Rasterize SVG before overlaying. Defaults to vector output when false.
    #[serde(default)]
    pub rasterize: bool,
    /// One-based source page for PDF page overlays.
    pub source_page: Option<u32>,
}

/// Image resource inspection options.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ImageInspectOptions {}

/// Image extraction options.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImageExtractOptions {
    pub name: String,
}

/// Image resource edit options.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImageEditOptions {
    pub action: ImageEditAction,
    pub name: Option<String>,
    pub page: Option<u32>,
}

/// Image resource edit action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageEditAction {
    Add,
    Replace,
    Delete,
}

/// Color content stream edit options.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ColorEditOptions {
    pub action: ColorEditAction,
    pub pages: Option<String>,
    pub from: Option<[f32; 3]>,
    pub to: Option<[f32; 3]>,
    pub factor: Option<f32>,
    #[serde(default)]
    pub rasterize_pages: bool,
}

/// Color content stream edit action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ColorEditAction {
    Contrast,
    Invert,
    Replace,
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
