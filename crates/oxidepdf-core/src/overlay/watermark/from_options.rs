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
    if options.font_path.is_none()
        && let Some(family) = options
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
    let (font_bytes, family_name) = if let Some(path) = &options.font_path {
        let bytes = std::fs::read(path).map_err(|_| OxideError::FontResolution)?;
        let mut db = fontdb::Database::new();
        db.load_font_data(bytes.clone());
        let face = db.faces().next().ok_or(OxideError::FontResolution)?;
        let family = face.families.first().ok_or(OxideError::FontResolution)?;
        (bytes, sanitize_pdf_name(&family.0))
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
