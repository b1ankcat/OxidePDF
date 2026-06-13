use crate::page_ops::parse_page_range;
use crate::{
    add_resource_dict_entry, enforce_input_bytes, enforce_max_pages, enforce_max_pixels,
    enforce_output_bytes, ensure_pdf_magic, load_pdf, map_pdf_extract_error,
    merge_resource_dictionary, object_to_f32, page_size, pdf_bytes, remap_imported_references,
    resource_limit, save_pdf, Artifact, BytesArtifact, ImageArtifact, OxideError, PdfArtifact,
    ResourceLimits, TextArtifact, TextExtractionDiagnostic, TextExtractionDiagnosticCode,
};
use lopdf::{dictionary, Dictionary, Object, Stream};
use pdf_writer::Finish;
use read_fonts::TableProvider;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

const A4_WIDTH: f32 = 595.0;
const A4_HEIGHT: f32 = 842.0;

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

/// Converts image artifacts into a PDF with one image per page.
pub fn image_artifacts_to_pdf(
    inputs: &[Artifact],
    options: &ImageToPdfOptions,
    limits: &ResourceLimits,
) -> Result<PdfArtifact, OxideError> {
    if inputs.is_empty() {
        return Err(OxideError::InvalidInput {
            reason: "img2pdf requires at least one image input".to_owned(),
        });
    }
    enforce_max_pages(inputs.len(), limits)?;

    let mut images = Vec::with_capacity(inputs.len());
    let mut total_pixels = 0u64;
    for input in inputs {
        let bytes = image_bytes(input)?;
        enforce_input_bytes(bytes.len(), limits)?;
        let decoded = decode_image(bytes)?;
        let pixels = u64::from(decoded.width) * u64::from(decoded.height);
        total_pixels = total_pixels
            .checked_add(pixels)
            .ok_or_else(|| resource_limit("max_pixels"))?;
        enforce_max_pixels(total_pixels, limits)?;
        images.push(decoded);
    }

    let layout = ImageLayout::from_options(options)?;
    let bytes = write_images_pdf(&images, layout)?;
    enforce_output_bytes(bytes.len(), limits)?;
    Ok(PdfArtifact {
        bytes: bytes.into(),
    })
}

/// Converts an SVG artifact into a PDF. Defaults to vector output.
pub fn svg_to_pdf(
    input: &[u8],
    options: &SvgToPdfOptions,
    limits: &ResourceLimits,
) -> Result<PdfArtifact, OxideError> {
    enforce_input_bytes(input.len(), limits)?;
    let tree = parse_svg(input)?;
    let pixels = svg_pixel_count(&tree)?;
    enforce_max_pixels(pixels, limits)?;
    enforce_max_pages(1, limits)?;

    let bytes = if options.rasterize {
        let image = rasterize_svg(&tree)?;
        write_images_pdf(&[image], ImageLayout::OriginalSize)?
    } else {
        let conversion_options = svg2pdf::ConversionOptions {
            embed_text: false,
            ..svg2pdf::ConversionOptions::default()
        };
        svg2pdf::to_pdf(&tree, conversion_options, svg2pdf::PageOptions::default())
            .map_err(|_| OxideError::WritePdf)?
    };

    enforce_output_bytes(bytes.len(), limits)?;
    Ok(PdfArtifact {
        bytes: bytes.into(),
    })
}

/// Renders a one-based PDF page to PNG bytes.
pub fn render_pdf_page(
    input: &[u8],
    options: &RenderOptions,
    limits: &ResourceLimits,
) -> Result<ImageArtifact, OxideError> {
    enforce_input_bytes(input.len(), limits)?;
    ensure_pdf_magic(input)?;
    let format = options.format.as_deref().unwrap_or("png");
    if format != "png" {
        return Err(OxideError::InvalidInput {
            reason: format!("unsupported render format '{format}'"),
        });
    }
    if options.page == 0 {
        return Err(OxideError::InvalidInput {
            reason: "page number must be one or greater".to_owned(),
        });
    }
    let scale = options.scale.unwrap_or(1.0);
    if !scale.is_finite() || scale <= 0.0 {
        return Err(OxideError::InvalidInput {
            reason: "render scale must be greater than zero".to_owned(),
        });
    }

    let pdf = hayro::hayro_syntax::Pdf::new(input.to_vec()).map_err(|_| OxideError::RenderPdf)?;
    let page_count = pdf.pages().len();
    enforce_max_pages(page_count, limits)?;
    let page_index = usize::try_from(options.page - 1).map_err(|_| OxideError::InvalidInput {
        reason: format!("page {} is out of range 1-{page_count}", options.page),
    })?;
    let page = pdf
        .pages()
        .get(page_index)
        .ok_or_else(|| OxideError::InvalidInput {
            reason: format!("page {} is out of range 1-{page_count}", options.page),
        })?;

    let cache = hayro::RenderCache::new();
    let interpreter_settings = hayro::hayro_interpret::InterpreterSettings::default();
    let render_settings = hayro::RenderSettings {
        x_scale: scale,
        y_scale: scale,
        bg_color: hayro::vello_cpu::color::palette::css::WHITE,
        ..Default::default()
    };
    let pixmap = hayro::render(page, &cache, &interpreter_settings, &render_settings);
    let bytes = pixmap.into_png().map_err(|_| OxideError::RenderPdf)?;
    if bytes.is_empty() {
        return Err(OxideError::RenderPdf);
    }
    enforce_output_bytes(bytes.len(), limits)?;

    Ok(ImageArtifact {
        bytes: bytes.into(),
    })
}

/// Extracts plain text from a PDF and records page-level diagnostics.
pub fn extract_text_from_pdf(
    input: &[u8],
    options: &ExtractTextOptions,
    limits: &ResourceLimits,
) -> Result<TextArtifact, OxideError> {
    enforce_input_bytes(input.len(), limits)?;
    ensure_pdf_magic(input)?;
    let format = options.format.as_deref().unwrap_or("plain");
    if format != "plain" {
        return Err(OxideError::InvalidInput {
            reason: format!("unsupported text extraction format '{format}'"),
        });
    }

    let pages =
        pdf_extract::extract_text_from_mem_by_pages(input).map_err(map_pdf_extract_error)?;
    if pages.is_empty() {
        return Err(OxideError::InvalidInput {
            reason: "PDF contains no pages".to_owned(),
        });
    }
    enforce_max_pages(pages.len(), limits)?;

    let diagnostics = pages
        .iter()
        .enumerate()
        .filter_map(|(index, page)| match page.trim().is_empty() {
            true => Some(TextExtractionDiagnostic {
                page: (index + 1) as u32,
                code: TextExtractionDiagnosticCode::NoTextLayer,
                message: "page has no extractable text layer".to_owned(),
            }),
            false => None,
        })
        .collect::<Vec<_>>();
    if diagnostics.len() == pages.len() {
        return Err(OxideError::InvalidInput {
            reason: "PDF has no extractable text layer".to_owned(),
        });
    }

    let artifact = TextArtifact {
        text: pages.concat(),
        diagnostics,
    };
    enforce_output_bytes(artifact.text.len(), limits)?;
    Ok(artifact)
}

/// Adds a text, image, or SVG watermark to selected PDF pages.
pub fn watermark_pdf_artifacts(
    inputs: &[Artifact],
    options: &WatermarkOptions,
    limits: &ResourceLimits,
) -> Result<PdfArtifact, OxideError> {
    let kind = match options.kind {
        WatermarkKind::Text => OverlayKind::Watermark,
        WatermarkKind::Image => OverlayKind::Image,
        WatermarkKind::Svg => OverlayKind::Svg,
    };
    overlay_pdf_artifacts(
        inputs,
        &OverlayOptions {
            kind,
            text: options.text.clone(),
            font: options.font.clone(),
            font_path: options.font_path.clone(),
            font_size: options.font_size,
            opacity: options.opacity,
            rotation: options.rotation,
            position: options.position.clone(),
            pages: options.pages.clone(),
            scale: options.scale,
            rasterize: options.rasterize,
            source_page: None,
        },
        limits,
    )
}

/// Adds text, image, SVG, stamp, signature appearance, or PDF page overlays.
pub fn overlay_pdf_artifacts(
    inputs: &[Artifact],
    options: &OverlayOptions,
    limits: &ResourceLimits,
) -> Result<PdfArtifact, OxideError> {
    let (pdf_input, overlay_input) = overlay_inputs(inputs, options.kind)?;
    enforce_input_bytes(pdf_input.len(), limits)?;
    let mut document = load_pdf(pdf_input)?;
    let page_count = document.get_pages().len() as u32;
    enforce_max_pages(page_count as usize, limits)?;
    let pages = match options.pages.as_deref() {
        Some(pages) => parse_page_range(pages, page_count)?,
        None => (1..=page_count).collect(),
    };
    let settings = WatermarkSettings::from_options(options)?;

    match options.kind {
        OverlayKind::Watermark
        | OverlayKind::Text
        | OverlayKind::Stamp
        | OverlayKind::SignatureAppearance => {
            let text = options
                .text
                .as_deref()
                .filter(|text| !text.is_empty())
                .ok_or_else(|| OxideError::InvalidInput {
                    reason: "text overlay requires non-empty text".to_owned(),
                })?;
            let font = resolve_watermark_font(options)?;
            append_text_watermark(&mut document, &pages, text, &font, settings)?;
        }
        OverlayKind::Image => {
            let image = decode_limited_image(
                overlay_input.ok_or_else(|| OxideError::InvalidInput {
                    reason: "image overlay requires an image input".to_owned(),
                })?,
                limits,
            )?;
            append_image_watermark(&mut document, &pages, &image, settings)?;
        }
        OverlayKind::Svg => {
            let svg = overlay_input.ok_or_else(|| OxideError::InvalidInput {
                reason: "SVG overlay requires an SVG input".to_owned(),
            })?;
            enforce_input_bytes(svg.len(), limits)?;
            let tree = parse_svg(svg)?;
            let pixels = svg_pixel_count(&tree)?;
            enforce_max_pixels(pixels, limits)?;
            if options.rasterize {
                let image = rasterize_svg(&tree)?;
                append_image_watermark(&mut document, &pages, &image, settings)?;
            } else {
                append_svg_watermark(&mut document, &pages, &tree, settings)?;
            }
        }
        OverlayKind::PdfPage => {
            let source = overlay_input.ok_or_else(|| OxideError::InvalidInput {
                reason: "PDF page overlay requires a second PDF input".to_owned(),
            })?;
            enforce_input_bytes(source.len(), limits)?;
            append_pdf_page_overlay(&mut document, &pages, source, options.source_page, settings)?;
        }
    }

    let bytes = save_pdf(document)?;
    enforce_output_bytes(bytes.len(), limits)?;
    Ok(PdfArtifact {
        bytes: bytes.into(),
    })
}

/// Inspects image XObject resources.
pub fn inspect_pdf_images(
    input: &[u8],
    _options: &ImageInspectOptions,
) -> Result<TextArtifact, OxideError> {
    let document = load_pdf(input)?;
    let mut images = Vec::new();
    for (page, page_id) in document.get_pages() {
        for (name, id, dict) in page_image_xobjects(&document, page_id)? {
            let width = required_image_dimension(&dict, b"Width")?;
            let height = required_image_dimension(&dict, b"Height")?;
            images.push(ImageResourceReport {
                page,
                name: String::from_utf8_lossy(&name).into_owned(),
                object_id: format!("{} {}", id.0, id.1),
                width,
                height,
            });
        }
    }
    images.sort_by(|left, right| {
        left.page
            .cmp(&right.page)
            .then_with(|| left.name.cmp(&right.name))
    });
    let text =
        serde_json::to_string_pretty(&ImageReport { images }).map_err(|_| OxideError::Internal)?;
    Ok(TextArtifact {
        text,
        diagnostics: Vec::new(),
    })
}

/// Adds, replaces, or deletes image XObject resources.
pub fn edit_pdf_images_artifacts(
    inputs: &[Artifact],
    options: &ImageEditOptions,
    limits: &ResourceLimits,
) -> Result<PdfArtifact, OxideError> {
    let pdf = inputs.first().ok_or_else(|| OxideError::InvalidInput {
        reason: "image edit requires a PDF input".to_owned(),
    })?;
    let pdf = pdf_bytes(pdf)?;
    enforce_input_bytes(pdf.len(), limits)?;
    let mut document = load_pdf(pdf)?;
    enforce_max_pages(document.get_pages().len(), limits)?;

    match options.action {
        ImageEditAction::Add => {
            if inputs.len() != 2 {
                return Err(OxideError::InvalidInput {
                    reason: "image add requires PDF input and image input".to_owned(),
                });
            }
            let image = decode_limited_image(image_bytes(&inputs[1])?, limits)?;
            let page = options.page.ok_or_else(|| OxideError::InvalidInput {
                reason: "image add requires page".to_owned(),
            })?;
            let name = required_image_name(options.name.as_deref(), "image add")?;
            add_image_to_page(&mut document, page, name, &image)?;
        }
        ImageEditAction::Replace => {
            if inputs.len() != 2 {
                return Err(OxideError::InvalidInput {
                    reason: "image replace requires PDF input and image input".to_owned(),
                });
            }
            let image = decode_limited_image(image_bytes(&inputs[1])?, limits)?;
            let name = required_image_name(options.name.as_deref(), "image replace")?;
            replace_image_resource(&mut document, name, &image)?;
        }
        ImageEditAction::Delete => {
            if inputs.len() != 1 {
                return Err(OxideError::InvalidInput {
                    reason: "image delete requires exactly one PDF input".to_owned(),
                });
            }
            let name = required_image_name(options.name.as_deref(), "image delete")?;
            delete_image_resource(&mut document, name)?;
        }
    }

    let bytes = save_pdf(document)?;
    enforce_output_bytes(bytes.len(), limits)?;
    Ok(PdfArtifact {
        bytes: bytes.into(),
    })
}

/// Extracts the decoded RGB payload for a named image XObject.
pub fn extract_pdf_image(
    input: &[u8],
    options: &ImageExtractOptions,
    limits: &ResourceLimits,
) -> Result<BytesArtifact, OxideError> {
    enforce_input_bytes(input.len(), limits)?;
    let document = load_pdf(input)?;
    let name = options.name.as_bytes();
    for (_, page_id) in document.get_pages() {
        for (resource_name, stream_id, _) in page_image_xobjects(&document, page_id)? {
            if resource_name == name {
                let stream = document
                    .get_object(stream_id)
                    .and_then(Object::as_stream)
                    .map_err(|_| OxideError::ParsePdf)?;
                let bytes = stream
                    .get_plain_content()
                    .map_err(|_| OxideError::ParsePdf)?;
                enforce_output_bytes(bytes.len(), limits)?;
                return Ok(BytesArtifact {
                    bytes: bytes.into(),
                });
            }
        }
    }
    Err(OxideError::InvalidInput {
        reason: format!("image '{}' not found", options.name),
    })
}

/// Edits simple RGB color operators in page content streams.
pub fn edit_pdf_colors(
    input: &[u8],
    options: &ColorEditOptions,
    limits: &ResourceLimits,
) -> Result<PdfArtifact, OxideError> {
    enforce_input_bytes(input.len(), limits)?;
    let mut document = load_pdf(input)?;
    edit_colors_on_document(&mut document, options, limits)?;
    let bytes = save_pdf(document)?;
    enforce_output_bytes(bytes.len(), limits)?;
    Ok(PdfArtifact {
        bytes: bytes.into(),
    })
}

/// Rewrites RGB color operators of an already-parsed document.
pub(crate) fn edit_colors_on_document(
    document: &mut lopdf::Document,
    options: &ColorEditOptions,
    limits: &ResourceLimits,
) -> Result<(), OxideError> {
    if options.rasterize_pages {
        return Err(OxideError::UnsupportedPdfFeature {
            feature: "color rasterize_pages is not supported by the vector content path".to_owned(),
        });
    }
    let page_count = document.get_pages().len() as u32;
    enforce_max_pages(page_count as usize, limits)?;
    let pages = match options.pages.as_deref() {
        Some(pages) => parse_page_range(pages, page_count)?,
        None => (1..=page_count).collect(),
    };
    validate_color_options(options)?;
    let page_map = document.get_pages();
    for page in pages {
        let page_id = *page_map
            .get(&page)
            .ok_or_else(|| OxideError::InvalidInput {
                reason: format!("page {page} is out of range"),
            })?;
        rewrite_page_colors(document, page_id, options)?;
    }
    Ok(())
}

#[derive(Debug, Serialize)]
struct ImageReport {
    images: Vec<ImageResourceReport>,
}

#[derive(Debug, Serialize)]
struct ImageResourceReport {
    page: u32,
    name: String,
    object_id: String,
    width: i64,
    height: i64,
}

fn overlay_inputs(
    inputs: &[Artifact],
    kind: OverlayKind,
) -> Result<(&[u8], Option<&[u8]>), OxideError> {
    match kind {
        OverlayKind::Watermark
        | OverlayKind::Text
        | OverlayKind::Stamp
        | OverlayKind::SignatureAppearance => {
            if inputs.len() != 1 {
                return Err(OxideError::InvalidInput {
                    reason: "text overlay requires exactly one PDF input".to_owned(),
                });
            }
            Ok((pdf_bytes(&inputs[0])?, None))
        }
        OverlayKind::Image | OverlayKind::Svg | OverlayKind::PdfPage => {
            if inputs.len() != 2 {
                return Err(OxideError::InvalidInput {
                    reason: "overlay requires PDF input and overlay input".to_owned(),
                });
            }
            let pdf = pdf_bytes(&inputs[0])?;
            let overlay = match kind {
                OverlayKind::Image => image_bytes(&inputs[1])?,
                OverlayKind::Svg => svg_bytes(&inputs[1])?,
                OverlayKind::PdfPage => pdf_bytes(&inputs[1])?,
                OverlayKind::Watermark
                | OverlayKind::Text
                | OverlayKind::Stamp
                | OverlayKind::SignatureAppearance => unreachable!(),
            };
            Ok((pdf, Some(overlay)))
        }
    }
}

fn image_bytes(artifact: &Artifact) -> Result<&[u8], OxideError> {
    match artifact {
        Artifact::Image(image) => Ok(&image.bytes),
        Artifact::Bytes(bytes) => Ok(&bytes.bytes),
        _ => Err(OxideError::InvalidInput {
            reason: "expected image input artifact".to_owned(),
        }),
    }
}

fn svg_bytes(artifact: &Artifact) -> Result<&[u8], OxideError> {
    match artifact {
        Artifact::Svg(svg) => Ok(&svg.bytes),
        Artifact::Bytes(bytes) => Ok(&bytes.bytes),
        _ => Err(OxideError::InvalidInput {
            reason: "expected SVG input artifact".to_owned(),
        }),
    }
}

#[derive(Debug, Clone)]
struct DecodedImage {
    width: u32,
    height: u32,
    rgb: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ImageLayout {
    Fit,
    OriginalSize,
}

impl ImageLayout {
    fn from_options(options: &ImageToPdfOptions) -> Result<Self, OxideError> {
        match options.layout.as_deref().unwrap_or("fit") {
            "fit" => Ok(Self::Fit),
            "original_size" => Ok(Self::OriginalSize),
            other => Err(OxideError::InvalidInput {
                reason: format!("unsupported image layout '{other}'"),
            }),
        }
    }
}

fn decode_image(input: &[u8]) -> Result<DecodedImage, OxideError> {
    let format = image::guess_format(input).map_err(|_| OxideError::ImageDecode)?;
    match format {
        image::ImageFormat::Jpeg | image::ImageFormat::Png | image::ImageFormat::WebP => {}
        _ => return Err(OxideError::ImageDecode),
    }
    let image = image::load_from_memory_with_format(input, format)
        .map_err(|_| OxideError::ImageDecode)?
        .to_rgb8();

    Ok(DecodedImage {
        width: image.width(),
        height: image.height(),
        rgb: image.into_raw(),
    })
}

fn decode_limited_image(input: &[u8], limits: &ResourceLimits) -> Result<DecodedImage, OxideError> {
    enforce_input_bytes(input.len(), limits)?;
    let decoded = decode_image(input)?;
    let pixels = u64::from(decoded.width) * u64::from(decoded.height);
    enforce_max_pixels(pixels, limits)?;
    Ok(decoded)
}

fn write_images_pdf(images: &[DecodedImage], layout: ImageLayout) -> Result<Vec<u8>, OxideError> {
    let mut next_ref = 1;
    let mut alloc_ref = || {
        let reference = pdf_writer::Ref::new(next_ref);
        next_ref += 1;
        reference
    };
    let catalog_id = alloc_ref();
    let pages_id = alloc_ref();
    let page_ids = (0..images.len()).map(|_| alloc_ref()).collect::<Vec<_>>();
    let image_ids = (0..images.len()).map(|_| alloc_ref()).collect::<Vec<_>>();
    let content_ids = (0..images.len()).map(|_| alloc_ref()).collect::<Vec<_>>();

    let mut pdf = pdf_writer::Pdf::new();
    pdf.catalog(catalog_id).pages(pages_id);
    pdf.pages(pages_id)
        .kids(page_ids.iter().copied())
        .count(images.len() as i32);

    for (((page_id, image_id), content_id), image) in page_ids
        .iter()
        .zip(image_ids.iter())
        .zip(content_ids.iter())
        .zip(images.iter())
    {
        let image_name = pdf_writer::Name(b"Im1");
        let (page_width, page_height, image_width, image_height, x, y) =
            image_placement(image, layout);

        let mut page = pdf.page(*page_id);
        page.media_box(pdf_writer::Rect::new(0.0, 0.0, page_width, page_height));
        page.parent(pages_id);
        page.contents(*content_id);
        page.resources().x_objects().pair(image_name, *image_id);
        page.finish();

        let mut image_object = pdf.image_xobject(*image_id, &image.rgb);
        image_object.width(image.width as i32);
        image_object.height(image.height as i32);
        image_object.color_space().device_rgb();
        image_object.bits_per_component(8);
        image_object.finish();

        let mut content = pdf_writer::Content::new();
        content.save_state();
        content.transform([image_width, 0.0, 0.0, image_height, x, y]);
        content.x_object(image_name);
        content.restore_state();
        pdf.stream(*content_id, &content.finish());
    }

    Ok(pdf.finish())
}

fn image_placement(image: &DecodedImage, layout: ImageLayout) -> (f32, f32, f32, f32, f32, f32) {
    let original_width = image.width as f32;
    let original_height = image.height as f32;
    match layout {
        ImageLayout::OriginalSize => (
            original_width,
            original_height,
            original_width,
            original_height,
            0.0,
            0.0,
        ),
        ImageLayout::Fit => {
            let scale = (A4_WIDTH / original_width)
                .min(A4_HEIGHT / original_height)
                .min(1.0);
            let image_width = original_width * scale;
            let image_height = original_height * scale;
            let x = (A4_WIDTH - image_width) / 2.0;
            let y = (A4_HEIGHT - image_height) / 2.0;
            (A4_WIDTH, A4_HEIGHT, image_width, image_height, x, y)
        }
    }
}

fn parse_svg(input: &[u8]) -> Result<svg2pdf::usvg::Tree, OxideError> {
    ensure_svg_magic(input)?;
    let options = svg2pdf::usvg::Options::default();
    svg2pdf::usvg::Tree::from_data(input, &options).map_err(|_| OxideError::SvgParse)
}

fn ensure_svg_magic(input: &[u8]) -> Result<(), OxideError> {
    let input = input
        .strip_prefix(&[0xEF, 0xBB, 0xBF])
        .unwrap_or(input)
        .trim_ascii_start();
    if input.starts_with(b"<svg") || input.starts_with(b"<?xml") {
        return Ok(());
    }

    Err(OxideError::SvgParse)
}

fn svg_pixel_count(tree: &svg2pdf::usvg::Tree) -> Result<u64, OxideError> {
    let size = tree.size().to_int_size();
    Ok(u64::from(size.width()) * u64::from(size.height()))
}

fn rasterize_svg(tree: &svg2pdf::usvg::Tree) -> Result<DecodedImage, OxideError> {
    let size = tree.size().to_int_size();
    let mut pixmap =
        resvg::tiny_skia::Pixmap::new(size.width(), size.height()).ok_or(OxideError::RenderPdf)?;
    resvg::render(
        tree,
        resvg::tiny_skia::Transform::identity(),
        &mut pixmap.as_mut(),
    );
    let mut rgb =
        Vec::with_capacity((u64::from(size.width()) * u64::from(size.height()) * 3) as usize);
    for pixel in pixmap.data().chunks_exact(4) {
        rgb.extend_from_slice(&[pixel[0], pixel[1], pixel[2]]);
    }

    Ok(DecodedImage {
        width: size.width(),
        height: size.height(),
        rgb,
    })
}

#[derive(Debug, Clone, Copy)]
struct WatermarkSettings {
    opacity: f32,
    rotation_degrees: f32,
    position: WatermarkPosition,
    scale: f32,
    font_size: f32,
}

impl WatermarkSettings {
    fn from_options(options: &OverlayOptions) -> Result<Self, OxideError> {
        let opacity = options.opacity.unwrap_or(0.25);
        if !opacity.is_finite() || !(0.0..=1.0).contains(&opacity) {
            return Err(OxideError::InvalidInput {
                reason: "watermark opacity must be between 0.0 and 1.0".to_owned(),
            });
        }
        let rotation_degrees = options.rotation.unwrap_or(0.0);
        if !rotation_degrees.is_finite() {
            return Err(OxideError::InvalidInput {
                reason: "watermark rotation must be finite".to_owned(),
            });
        }
        let scale = options.scale.unwrap_or(0.35);
        if !scale.is_finite() || scale <= 0.0 {
            return Err(OxideError::InvalidInput {
                reason: "watermark scale must be greater than zero".to_owned(),
            });
        }
        let font_size = options.font_size.unwrap_or(48.0);
        if !font_size.is_finite() || font_size <= 0.0 {
            return Err(OxideError::InvalidInput {
                reason: "watermark font size must be greater than zero".to_owned(),
            });
        }

        Ok(Self {
            opacity,
            rotation_degrees,
            position: WatermarkPosition::parse(options.position.as_deref().unwrap_or("center"))?,
            scale,
            font_size,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WatermarkPosition {
    Center,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl WatermarkPosition {
    fn parse(value: &str) -> Result<Self, OxideError> {
        match value {
            "center" => Ok(Self::Center),
            "top_left" => Ok(Self::TopLeft),
            "top_right" => Ok(Self::TopRight),
            "bottom_left" => Ok(Self::BottomLeft),
            "bottom_right" => Ok(Self::BottomRight),
            other => Err(OxideError::InvalidInput {
                reason: format!("unsupported watermark position '{other}'"),
            }),
        }
    }
}

#[derive(Debug, Clone)]
struct ResolvedFont {
    resource_name: Vec<u8>,
    base_font: Vec<u8>,
    metrics: FontMetrics,
}

#[derive(Debug, Clone, Copy)]
struct FontMetrics {
    units_per_em: u16,
    ascent: i16,
    descent: i16,
}

fn resolve_watermark_font(options: &OverlayOptions) -> Result<ResolvedFont, OxideError> {
    if options.font_path.is_none() {
        if let Some(family) = options
            .font
            .as_deref()
            .filter(|family| is_standard_pdf_font(family))
        {
            return Ok(ResolvedFont {
                resource_name: b"OxWmF1".to_vec(),
                base_font: family.as_bytes().to_vec(),
                metrics: FontMetrics {
                    units_per_em: 1000,
                    ascent: 718,
                    descent: -207,
                },
            });
        }
    }
    let (font_bytes, family_name) = if let Some(path) = &options.font_path {
        let bytes = std::fs::read(path).map_err(|_| OxideError::FontResolution)?;
        let mut db = fontdb::Database::new();
        db.load_font_data(bytes.clone());
        let face = db.faces().next().ok_or(OxideError::FontResolution)?;
        (bytes, sanitize_pdf_name(&face.families[0].0))
    } else {
        let family = options.font.as_deref().ok_or(OxideError::FontResolution)?;
        let mut db = fontdb::Database::new();
        db.load_system_fonts();
        let query = fontdb::Query {
            families: &[fontdb::Family::Name(family)],
            ..fontdb::Query::default()
        };
        let id = db.query(&query).ok_or(OxideError::FontResolution)?;
        let bytes = db
            .with_face_data(id, |data, _index| data.to_vec())
            .ok_or(OxideError::FontResolution)?;
        (bytes, sanitize_pdf_name(family))
    };

    let metrics = read_font_metrics(&font_bytes)?;
    Ok(ResolvedFont {
        resource_name: b"OxWmF1".to_vec(),
        base_font: family_name,
        metrics,
    })
}

fn is_standard_pdf_font(family: &str) -> bool {
    matches!(
        family,
        "Courier"
            | "Courier-Bold"
            | "Courier-Oblique"
            | "Courier-BoldOblique"
            | "Helvetica"
            | "Helvetica-Bold"
            | "Helvetica-Oblique"
            | "Helvetica-BoldOblique"
            | "Times-Roman"
            | "Times-Bold"
            | "Times-Italic"
            | "Times-BoldItalic"
            | "Symbol"
            | "ZapfDingbats"
    )
}

fn read_font_metrics(bytes: &[u8]) -> Result<FontMetrics, OxideError> {
    let font = skrifa::FontRef::from_index(bytes, 0).map_err(|_| OxideError::FontResolution)?;
    let head = font.head().map_err(|_| OxideError::FontResolution)?;
    let hhea = font.hhea().map_err(|_| OxideError::FontResolution)?;
    Ok(FontMetrics {
        units_per_em: head.units_per_em(),
        ascent: hhea.ascender().into(),
        descent: hhea.descender().into(),
    })
}

fn sanitize_pdf_name(value: &str) -> Vec<u8> {
    let name = value
        .bytes()
        .filter(|byte| byte.is_ascii_alphanumeric() || *byte == b'-' || *byte == b'_')
        .collect::<Vec<_>>();
    if name.is_empty() {
        b"OxideWatermarkFont".to_vec()
    } else {
        name
    }
}

fn append_text_watermark(
    document: &mut lopdf::Document,
    pages: &[u32],
    text: &str,
    font: &ResolvedFont,
    settings: WatermarkSettings,
) -> Result<(), OxideError> {
    if !text.is_ascii() {
        return Err(OxideError::UnsupportedPdfFeature {
            feature: "non-ASCII text watermark".to_owned(),
        });
    }
    let font_id = document.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => Object::Name(font.base_font.clone()),
        "Encoding" => Object::Name(b"WinAnsiEncoding".to_vec()),
    });
    let gs_id = graphics_state(document, settings.opacity);
    let page_map = document.get_pages();

    for page_number in pages {
        let page_id = *page_map
            .get(page_number)
            .ok_or_else(|| OxideError::InvalidInput {
                reason: format!("page {page_number} is out of range"),
            })?;
        add_resource_dict_entry(
            document,
            page_id,
            b"Font",
            font.resource_name.clone(),
            Object::Reference(font_id),
        )?;
        add_resource_dict_entry(
            document,
            page_id,
            b"ExtGState",
            b"OxWmGS".to_vec(),
            Object::Reference(gs_id),
        )?;
        let (page_width, page_height) = page_size(document, page_id)?;
        let text_width = approximate_text_width(text, font.metrics, settings.font_size);
        let text_height = settings.font_size;
        let (x, y) = watermark_origin(
            settings.position,
            page_width,
            page_height,
            text_width,
            text_height,
        );
        let content = text_watermark_content(text, &font.resource_name, settings, x, y)?;
        document
            .add_page_contents(page_id, content)
            .map_err(|_| OxideError::WritePdf)?;
    }

    Ok(())
}

fn append_image_watermark(
    document: &mut lopdf::Document,
    pages: &[u32],
    image: &DecodedImage,
    settings: WatermarkSettings,
) -> Result<(), OxideError> {
    let image_id = document.add_object(image_xobject(image));
    append_xobject_watermark(
        document,
        pages,
        image_id,
        image.width as f32,
        image.height as f32,
        b"OxWmIm".to_vec(),
        settings,
    )
}

fn append_svg_watermark(
    document: &mut lopdf::Document,
    pages: &[u32],
    tree: &svg2pdf::usvg::Tree,
    settings: WatermarkSettings,
) -> Result<(), OxideError> {
    let size = tree.size();
    let width = size.width();
    let height = size.height();
    let svg_id = svg_form_xobject(document, tree, width, height)?;
    append_xobject_watermark(
        document,
        pages,
        svg_id,
        width,
        height,
        b"OxWmSvg".to_vec(),
        settings,
    )
}

fn append_pdf_page_overlay(
    target: &mut lopdf::Document,
    pages: &[u32],
    source: &[u8],
    source_page: Option<u32>,
    settings: WatermarkSettings,
) -> Result<(), OxideError> {
    let source = load_pdf(source)?;
    let source_page = source_page.unwrap_or(1);
    let source_page_id =
        *source
            .get_pages()
            .get(&source_page)
            .ok_or_else(|| OxideError::InvalidInput {
                reason: format!("source page {source_page} is out of range"),
            })?;
    let (width, height) = page_size(&source, source_page_id)?;
    let content = source
        .get_page_content(source_page_id)
        .map_err(|_| OxideError::ParsePdf)?;
    let resources = imported_page_resources(&source, target, source_page_id)?;
    let form_id = target.add_object(form_xobject(width, height, resources, content));
    append_xobject_watermark(
        target,
        pages,
        form_id,
        width,
        height,
        b"OxPdfOverlay".to_vec(),
        settings,
    )
}

fn svg_form_xobject(
    target: &mut lopdf::Document,
    tree: &svg2pdf::usvg::Tree,
    width: f32,
    height: f32,
) -> Result<lopdf::ObjectId, OxideError> {
    let conversion_options = svg2pdf::ConversionOptions {
        embed_text: false,
        ..svg2pdf::ConversionOptions::default()
    };
    let bytes = svg2pdf::to_pdf(tree, conversion_options, svg2pdf::PageOptions::default())
        .map_err(|_| OxideError::WritePdf)?;
    let source = lopdf::Document::load_mem(&bytes).map_err(|_| OxideError::ParsePdf)?;
    let page_id = source
        .get_pages()
        .into_values()
        .next()
        .ok_or(OxideError::ParsePdf)?;
    let content = source
        .get_page_content(page_id)
        .map_err(|_| OxideError::ParsePdf)?;
    let resources = imported_page_resources(&source, target, page_id)?;

    let mut dict = dictionary! {
        "Type" => "XObject",
        "Subtype" => "Form",
        "BBox" => Object::Array(vec![
            Object::Real(0.0),
            Object::Real(0.0),
            Object::Real(width),
            Object::Real(height),
        ]),
        "Matrix" => Object::Array(vec![
            Object::Real(1.0),
            Object::Real(0.0),
            Object::Real(0.0),
            Object::Real(1.0),
            Object::Real(0.0),
            Object::Real(0.0),
        ]),
    };
    dict.set("Resources", resources);
    Ok(target.add_object(Stream::new(dict, content)))
}

fn form_xobject(width: f32, height: f32, resources: Dictionary, content: Vec<u8>) -> Stream {
    let mut dict = dictionary! {
        "Type" => "XObject",
        "Subtype" => "Form",
        "BBox" => Object::Array(vec![
            Object::Real(0.0),
            Object::Real(0.0),
            Object::Real(width),
            Object::Real(height),
        ]),
        "Matrix" => Object::Array(vec![
            Object::Real(1.0),
            Object::Real(0.0),
            Object::Real(0.0),
            Object::Real(1.0),
            Object::Real(0.0),
            Object::Real(0.0),
        ]),
    };
    dict.set("Resources", resources);
    Stream::new(dict, content)
}

fn page_image_xobjects(
    document: &lopdf::Document,
    page_id: lopdf::ObjectId,
) -> Result<Vec<(Vec<u8>, lopdf::ObjectId, Dictionary)>, OxideError> {
    let (direct_resources, inherited_resource_ids) = document
        .get_page_resources(page_id)
        .map_err(|_| OxideError::ParsePdf)?;
    let mut resources = Dictionary::new();
    for resource_id in inherited_resource_ids.iter().rev() {
        let inherited = document
            .get_dictionary(*resource_id)
            .map_err(|_| OxideError::ParsePdf)?;
        merge_resource_dictionary(&mut resources, inherited);
    }
    if let Some(direct) = direct_resources {
        merge_resource_dictionary(&mut resources, direct);
    }
    let Some(xobjects) = optional_xobject_dict(&resources)? else {
        return Ok(Vec::new());
    };
    let mut images = Vec::new();
    for (name, object) in xobjects.iter() {
        let id = object.as_reference().map_err(|_| OxideError::ParsePdf)?;
        let stream = document
            .get_object(id)
            .and_then(Object::as_stream)
            .map_err(|_| OxideError::ParsePdf)?;
        if stream
            .dict
            .get(b"Subtype")
            .and_then(Object::as_name)
            .is_ok_and(|subtype| subtype == b"Image")
        {
            images.push((name.clone(), id, stream.dict.clone()));
        }
    }
    Ok(images)
}

fn page_xobject_reference(
    document: &lopdf::Document,
    page_id: lopdf::ObjectId,
    name: &[u8],
) -> Result<Option<lopdf::ObjectId>, OxideError> {
    let (direct_resources, inherited_resource_ids) = document
        .get_page_resources(page_id)
        .map_err(|_| OxideError::ParsePdf)?;
    let mut resources = Dictionary::new();
    for resource_id in inherited_resource_ids.iter().rev() {
        let inherited = document
            .get_dictionary(*resource_id)
            .map_err(|_| OxideError::ParsePdf)?;
        merge_resource_dictionary(&mut resources, inherited);
    }
    if let Some(direct) = direct_resources {
        merge_resource_dictionary(&mut resources, direct);
    }
    let Some(xobjects) = optional_xobject_dict(&resources)? else {
        return Ok(None);
    };
    if !xobjects.has(name) {
        return Ok(None);
    }
    let id = xobjects
        .get(name)
        .and_then(Object::as_reference)
        .map_err(|_| OxideError::ParsePdf)?;
    Ok(Some(id))
}

fn optional_xobject_dict(resources: &Dictionary) -> Result<Option<&Dictionary>, OxideError> {
    if !resources.has(b"XObject") {
        return Ok(None);
    }
    resources
        .get(b"XObject")
        .and_then(Object::as_dict)
        .map(Some)
        .map_err(|_| OxideError::ParsePdf)
}

fn optional_xobject_dict_mut(
    resources: &mut Dictionary,
) -> Result<Option<&mut Dictionary>, OxideError> {
    if !resources.has(b"XObject") {
        return Ok(None);
    }
    resources
        .get_mut(b"XObject")
        .and_then(Object::as_dict_mut)
        .map(Some)
        .map_err(|_| OxideError::ParsePdf)
}

fn required_image_dimension(dict: &Dictionary, key: &[u8]) -> Result<i64, OxideError> {
    dict.get(key)
        .and_then(Object::as_i64)
        .map_err(|_| OxideError::ParsePdf)
}

fn imported_page_resources(
    source: &lopdf::Document,
    target: &mut lopdf::Document,
    page_id: lopdf::ObjectId,
) -> Result<Dictionary, OxideError> {
    let (direct_resources, inherited_resource_ids) = source
        .get_page_resources(page_id)
        .map_err(|_| OxideError::ParsePdf)?;
    let mut resources = Dictionary::new();
    for resource_id in inherited_resource_ids.iter().rev() {
        let inherited = source
            .get_dictionary(*resource_id)
            .map_err(|_| OxideError::ParsePdf)?;
        merge_resource_dictionary(&mut resources, inherited);
    }
    if let Some(direct) = direct_resources {
        merge_resource_dictionary(&mut resources, direct);
    }

    let mut resource_object = Object::Dictionary(resources);
    let mut imported = BTreeMap::new();
    remap_imported_references(&mut resource_object, source, target, &mut imported)?;
    resource_object
        .as_dict()
        .cloned()
        .map_err(|_| OxideError::ParsePdf)
}

fn append_xobject_watermark(
    document: &mut lopdf::Document,
    pages: &[u32],
    xobject_id: lopdf::ObjectId,
    natural_width: f32,
    natural_height: f32,
    resource_name: Vec<u8>,
    settings: WatermarkSettings,
) -> Result<(), OxideError> {
    let gs_id = graphics_state(document, settings.opacity);
    let page_map = document.get_pages();
    for page_number in pages {
        let page_id = *page_map
            .get(page_number)
            .ok_or_else(|| OxideError::InvalidInput {
                reason: format!("page {page_number} is out of range"),
            })?;
        add_resource_dict_entry(
            document,
            page_id,
            b"XObject",
            resource_name.clone(),
            Object::Reference(xobject_id),
        )?;
        add_resource_dict_entry(
            document,
            page_id,
            b"ExtGState",
            b"OxWmGS".to_vec(),
            Object::Reference(gs_id),
        )?;
        let (page_width, page_height) = page_size(document, page_id)?;
        let scale = (page_width / natural_width)
            .min(page_height / natural_height)
            .min(1.0)
            * settings.scale;
        let width = natural_width * scale;
        let height = natural_height * scale;
        let (x, y) = watermark_origin(settings.position, page_width, page_height, width, height);
        let content = xobject_watermark_content(&resource_name, settings, x, y, width, height)?;
        document
            .add_page_contents(page_id, content)
            .map_err(|_| OxideError::WritePdf)?;
    }

    Ok(())
}

fn add_image_to_page(
    document: &mut lopdf::Document,
    page: u32,
    name: &str,
    image: &DecodedImage,
) -> Result<(), OxideError> {
    let page_id = *document
        .get_pages()
        .get(&page)
        .ok_or_else(|| OxideError::InvalidInput {
            reason: format!("page {page} is out of range"),
        })?;
    let image_id = document.add_object(image_xobject(image));
    add_resource_dict_entry(
        document,
        page_id,
        b"XObject",
        name.as_bytes().to_vec(),
        Object::Reference(image_id),
    )?;
    let (page_width, page_height) = page_size(document, page_id)?;
    let scale = (page_width / image.width as f32)
        .min(page_height / image.height as f32)
        .min(1.0);
    let width = image.width as f32 * scale;
    let height = image.height as f32 * scale;
    let content = xobject_watermark_content(
        name.as_bytes(),
        WatermarkSettings {
            opacity: 1.0,
            rotation_degrees: 0.0,
            position: WatermarkPosition::Center,
            scale: 1.0,
            font_size: 1.0,
        },
        (page_width - width) / 2.0,
        (page_height - height) / 2.0,
        width,
        height,
    )?;
    document
        .add_page_contents(page_id, content)
        .map_err(|_| OxideError::WritePdf)
}

fn replace_image_resource(
    document: &mut lopdf::Document,
    name: &str,
    image: &DecodedImage,
) -> Result<(), OxideError> {
    let mut replaced = false;
    for (_, page_id) in document.get_pages() {
        let Some(id) = page_xobject_reference(document, page_id, name.as_bytes())? else {
            continue;
        };
        let stream = document
            .get_object_mut(id)
            .and_then(Object::as_stream_mut)
            .map_err(|_| OxideError::ParsePdf)?;
        *stream = image_xobject(image);
        replaced = true;
    }
    if !replaced {
        return Err(OxideError::InvalidInput {
            reason: format!("image '{name}' not found"),
        });
    }
    Ok(())
}

fn delete_image_resource(document: &mut lopdf::Document, name: &str) -> Result<(), OxideError> {
    let mut removed = false;
    for page_id in document.get_pages().into_values() {
        let resources = document
            .get_or_create_resources(page_id)
            .and_then(Object::as_dict_mut)
            .map_err(|_| OxideError::ParsePdf)?;
        if let Some(xobjects) = optional_xobject_dict_mut(resources)? {
            removed |= xobjects.remove(name.as_bytes()).is_some();
        }
    }
    if !removed {
        return Err(OxideError::InvalidInput {
            reason: format!("image '{name}' not found"),
        });
    }
    Ok(())
}

fn required_image_name<'a>(name: Option<&'a str>, label: &str) -> Result<&'a str, OxideError> {
    name.filter(|value| !value.is_empty())
        .ok_or_else(|| OxideError::InvalidInput {
            reason: format!("{label} requires non-empty image name"),
        })
}

fn validate_color_options(options: &ColorEditOptions) -> Result<(), OxideError> {
    match options.action {
        ColorEditAction::Contrast => {
            let factor = options.factor.unwrap_or(1.0);
            if !factor.is_finite() || factor <= 0.0 {
                return Err(OxideError::InvalidInput {
                    reason: "contrast factor must be greater than zero".to_owned(),
                });
            }
        }
        ColorEditAction::Invert => {}
        ColorEditAction::Replace => {
            validate_rgb(options.from, "replace from")?;
            validate_rgb(options.to, "replace to")?;
        }
    }
    Ok(())
}

fn validate_rgb(value: Option<[f32; 3]>, label: &str) -> Result<(), OxideError> {
    let rgb = value.ok_or_else(|| OxideError::InvalidInput {
        reason: format!("color {label} must be provided"),
    })?;
    if rgb
        .iter()
        .any(|component| !component.is_finite() || !(0.0..=1.0).contains(component))
    {
        return Err(OxideError::InvalidInput {
            reason: format!("color {label} components must be between 0.0 and 1.0"),
        });
    }
    Ok(())
}

fn rewrite_page_colors(
    document: &mut lopdf::Document,
    page_id: lopdf::ObjectId,
    options: &ColorEditOptions,
) -> Result<(), OxideError> {
    let content = document
        .get_page_content(page_id)
        .map_err(|_| OxideError::ParsePdf)?;
    let mut content =
        lopdf::content::Content::decode(&content).map_err(|_| OxideError::ParsePdf)?;
    for operation in &mut content.operations {
        match operation.operator.as_str() {
            "rg" | "RG" => rewrite_rgb_operation(operation, options)?,
            "g" | "G" | "k" | "K" | "cs" | "CS" | "sc" | "SC" | "scn" | "SCN" | "sh" => {
                return Err(OxideError::UnsupportedPdfFeature {
                    feature: format!(
                        "color operation '{}' is not supported by vector color editing",
                        operation.operator
                    ),
                });
            }
            _ => {}
        }
    }
    let bytes = content.encode().map_err(|_| OxideError::WritePdf)?;
    replace_page_content(document, page_id, bytes)
}

fn rewrite_rgb_operation(
    operation: &mut lopdf::content::Operation,
    options: &ColorEditOptions,
) -> Result<(), OxideError> {
    if operation.operands.len() != 3 {
        return Err(OxideError::ParsePdf);
    }
    let current = [
        object_to_f32(&operation.operands[0])?,
        object_to_f32(&operation.operands[1])?,
        object_to_f32(&operation.operands[2])?,
    ];
    let updated = match options.action {
        ColorEditAction::Invert => [1.0 - current[0], 1.0 - current[1], 1.0 - current[2]],
        ColorEditAction::Replace => {
            let from = options.from.unwrap();
            let to = options.to.unwrap();
            if rgb_matches(current, from) {
                to
            } else {
                current
            }
        }
        ColorEditAction::Contrast => {
            let factor = options.factor.unwrap_or(1.0);
            current.map(|component| ((component - 0.5) * factor + 0.5).clamp(0.0, 1.0))
        }
    };
    operation.operands = updated.into_iter().map(Object::Real).collect();
    Ok(())
}

fn rgb_matches(left: [f32; 3], right: [f32; 3]) -> bool {
    left.iter()
        .zip(right.iter())
        .all(|(left, right)| (left - right).abs() <= 0.0001)
}

fn replace_page_content(
    document: &mut lopdf::Document,
    page_id: lopdf::ObjectId,
    content: Vec<u8>,
) -> Result<(), OxideError> {
    let content_id = document.add_object(Stream::new(Dictionary::new(), content));
    let page = document
        .get_object_mut(page_id)
        .and_then(Object::as_dict_mut)
        .map_err(|_| OxideError::ParsePdf)?;
    page.set("Contents", content_id);
    Ok(())
}

fn graphics_state(document: &mut lopdf::Document, opacity: f32) -> lopdf::ObjectId {
    document.add_object(dictionary! {
        "Type" => "ExtGState",
        "ca" => Object::Real(opacity),
        "CA" => Object::Real(opacity),
    })
}

fn watermark_origin(
    position: WatermarkPosition,
    page_width: f32,
    page_height: f32,
    width: f32,
    height: f32,
) -> (f32, f32) {
    let margin = 36.0;
    match position {
        WatermarkPosition::Center => ((page_width - width) / 2.0, (page_height - height) / 2.0),
        WatermarkPosition::TopLeft => (margin, page_height - height - margin),
        WatermarkPosition::TopRight => (page_width - width - margin, page_height - height - margin),
        WatermarkPosition::BottomLeft => (margin, margin),
        WatermarkPosition::BottomRight => (page_width - width - margin, margin),
    }
}

fn approximate_text_width(text: &str, metrics: FontMetrics, font_size: f32) -> f32 {
    let em = f32::from(metrics.units_per_em.max(1));
    let height_units = i32::from(metrics.ascent) - i32::from(metrics.descent);
    let height_ratio = (height_units.max(1) as f32 / em).max(0.5);
    text.len() as f32 * font_size * 0.55 * height_ratio
}

fn text_watermark_content(
    text: &str,
    font_name: &[u8],
    settings: WatermarkSettings,
    x: f32,
    y: f32,
) -> Result<Vec<u8>, OxideError> {
    let matrix = rotation_matrix(settings.rotation_degrees, x, y);
    lopdf::content::Content {
        operations: vec![
            lopdf::content::Operation::new("q", vec![]),
            lopdf::content::Operation::new("gs", vec![Object::Name(b"OxWmGS".to_vec())]),
            lopdf::content::Operation::new(
                "cm",
                matrix.iter().copied().map(Object::Real).collect(),
            ),
            lopdf::content::Operation::new("BT", vec![]),
            lopdf::content::Operation::new(
                "Tf",
                vec![
                    Object::Name(font_name.to_vec()),
                    Object::Real(settings.font_size),
                ],
            ),
            lopdf::content::Operation::new("Td", vec![Object::Integer(0), Object::Integer(0)]),
            lopdf::content::Operation::new("Tj", vec![Object::string_literal(text)]),
            lopdf::content::Operation::new("ET", vec![]),
            lopdf::content::Operation::new("Q", vec![]),
        ],
    }
    .encode()
    .map_err(|_| OxideError::WritePdf)
}

fn xobject_watermark_content(
    resource_name: &[u8],
    settings: WatermarkSettings,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
) -> Result<Vec<u8>, OxideError> {
    let mut matrix = rotation_matrix(settings.rotation_degrees, x, y);
    matrix[0] *= width;
    matrix[1] *= width;
    matrix[2] *= height;
    matrix[3] *= height;
    lopdf::content::Content {
        operations: vec![
            lopdf::content::Operation::new("q", vec![]),
            lopdf::content::Operation::new("gs", vec![Object::Name(b"OxWmGS".to_vec())]),
            lopdf::content::Operation::new(
                "cm",
                matrix.iter().copied().map(Object::Real).collect(),
            ),
            lopdf::content::Operation::new("Do", vec![Object::Name(resource_name.to_vec())]),
            lopdf::content::Operation::new("Q", vec![]),
        ],
    }
    .encode()
    .map_err(|_| OxideError::WritePdf)
}

fn rotation_matrix(degrees: f32, x: f32, y: f32) -> [f32; 6] {
    let radians = degrees.to_radians();
    let cos = radians.cos();
    let sin = radians.sin();
    [cos, sin, -sin, cos, x, y]
}

fn image_xobject(image: &DecodedImage) -> Stream {
    let dict = dictionary! {
        "Type" => "XObject",
        "Subtype" => "Image",
        "Width" => image.width as i64,
        "Height" => image.height as i64,
        "ColorSpace" => "DeviceRGB",
        "BitsPerComponent" => 8,
    };
    Stream::new(dict, image.rgb.clone())
}
