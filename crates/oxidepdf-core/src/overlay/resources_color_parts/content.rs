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
