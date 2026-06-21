
fn generated_fixture_pdf() -> Vec<u8> {
    let mut document = lopdf::Document::with_version("1.7");
    let pages_id = document.new_object_id();
    let font_id = document.new_object_id();
    let catalog_id = document.new_object_id();
    let mut page_ids = Vec::new();

    document.objects.insert(
        font_id,
        lopdf::Object::Dictionary(lopdf::dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => "Helvetica",
        }),
    );

    for page_number in 1..=3 {
        let page_id = document.new_object_id();
        let content_id = document.new_object_id();
        let content = lopdf::content::Content {
            operations: vec![
                lopdf::content::Operation::new("BT", vec![]),
                lopdf::content::Operation::new(
                    "Tf",
                    vec![lopdf::Object::Name(b"F1".to_vec()), 12.into()],
                ),
                lopdf::content::Operation::new("Td", vec![72.into(), 720.into()]),
                lopdf::content::Operation::new(
                    "Tj",
                    vec![lopdf::Object::string_literal(format!(
                        "OxidePDF fixture page {page_number}"
                    ))],
                ),
                lopdf::content::Operation::new("ET", vec![]),
            ],
        }
        .encode()
        .unwrap();
        document.objects.insert(
            content_id,
            lopdf::Object::Stream(lopdf::Stream::new(lopdf::Dictionary::new(), content)),
        );
        document.objects.insert(
            page_id,
            lopdf::Object::Dictionary(lopdf::dictionary! {
                "Type" => "Page",
                "Parent" => pages_id,
                "MediaBox" => lopdf::Object::Array(vec![0.into(), 0.into(), 612.into(), 792.into()]),
                "Resources" => lopdf::Object::Dictionary(lopdf::dictionary! {
                    "Font" => lopdf::Object::Dictionary(lopdf::dictionary! {
                        "F1" => font_id,
                    }),
                }),
                "Contents" => content_id,
            }),
        );
        page_ids.push(page_id);
    }

    document.objects.insert(
        pages_id,
        lopdf::Object::Dictionary(lopdf::dictionary! {
            "Type" => "Pages",
            "Kids" => lopdf::Object::Array(page_ids.iter().copied().map(lopdf::Object::Reference).collect()),
            "Count" => page_ids.len() as i64,
        }),
    );
    document.objects.insert(
        catalog_id,
        lopdf::Object::Dictionary(lopdf::dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        }),
    );
    document.trailer.set("Root", catalog_id);

    let mut bytes = Vec::new();
    document.save_to(&mut bytes).unwrap();
    bytes
}

fn generated_fixture_jpg() -> Vec<u8> {
    let image = image::RgbImage::from_fn(8, 8, |x, y| {
        if (x + y) % 2 == 0 {
            image::Rgb([220, 38, 38])
        } else {
            image::Rgb([37, 99, 235])
        }
    });
    let mut bytes = Vec::new();
    let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, 85);
    encoder.encode_image(&image).unwrap();
    bytes
}

fn generated_signature_pdf() -> Vec<u8> {
    let mut document = lopdf::Document::with_version("1.7");
    let pages_id = document.new_object_id();
    let page_id = document.new_object_id();
    let sig_field_id = document.new_object_id();
    let sig_value_id = document.new_object_id();
    let acroform_id = document.new_object_id();
    let catalog_id = document.new_object_id();

    let sig_value = lopdf::dictionary! {
        "Type" => "Sig",
        "Filter" => "Adobe.PPKLite",
        "SubFilter" => "adbe.pkcs7.detached",
        "ByteRange" => lopdf::Object::Array(vec![0.into(), 64.into(), 192.into(), 64.into()]),
        "Contents" => lopdf::Object::String(vec![0x30, 0x82], lopdf::StringFormat::Hexadecimal),
    };
    document
        .objects
        .insert(sig_value_id, lopdf::Object::Dictionary(sig_value));

    let sig_field = lopdf::dictionary! {
        "Type" => "Annot",
        "Subtype" => "Widget",
        "FT" => "Sig",
        "T" => lopdf::Object::string_literal("Approval"),
        "V" => sig_value_id,
        "Rect" => lopdf::Object::Array(vec![0.into(), 0.into(), 0.into(), 0.into()]),
        "P" => page_id,
    };
    document
        .objects
        .insert(sig_field_id, lopdf::Object::Dictionary(sig_field));

    let page = lopdf::dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => lopdf::Object::Array(vec![0.into(), 0.into(), 200.into(), 200.into()]),
        "Annots" => lopdf::Object::Array(vec![sig_field_id.into()]),
    };
    document
        .objects
        .insert(page_id, lopdf::Object::Dictionary(page));

    let pages = lopdf::dictionary! {
        "Type" => "Pages",
        "Kids" => lopdf::Object::Array(vec![page_id.into()]),
        "Count" => 1,
    };
    document
        .objects
        .insert(pages_id, lopdf::Object::Dictionary(pages));

    let acroform = lopdf::dictionary! {
        "Fields" => lopdf::Object::Array(vec![sig_field_id.into()]),
    };
    document
        .objects
        .insert(acroform_id, lopdf::Object::Dictionary(acroform));

    let catalog = lopdf::dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
        "AcroForm" => acroform_id,
    };
    document
        .objects
        .insert(catalog_id, lopdf::Object::Dictionary(catalog));
    document.trailer.set("Root", catalog_id);

    let mut bytes = Vec::new();
    document.save_to(&mut bytes).unwrap();
    bytes
}

pub fn pdf_page_count(path: &std::path::Path) -> usize {
    lopdf::Document::load(path).unwrap().get_pages().len()
}

pub fn pdf_page_rotation(path: &std::path::Path, page_number: u32) -> i64 {
    let document = lopdf::Document::load(path).unwrap();
    let page_id = document.get_pages().get(&page_number).copied().unwrap();
    let page = document.get_object(page_id).unwrap().as_dict().unwrap();
    page.get(b"Rotate")
        .and_then(lopdf::Object::as_i64)
        .unwrap_or(0)
}

pub fn pdf_page_box(path: &std::path::Path, page_number: u32, key: &[u8]) -> [f32; 4] {
    let document = lopdf::Document::load(path).unwrap();
    let page_id = document.get_pages().get(&page_number).copied().unwrap();
    let page = document.get_object(page_id).unwrap().as_dict().unwrap();
    let values = page.get(key).unwrap().as_array().unwrap();
    [
        pdf_object_to_f32(&values[0]),
        pdf_object_to_f32(&values[1]),
        pdf_object_to_f32(&values[2]),
        pdf_object_to_f32(&values[3]),
    ]
}

pub fn pdf_object_to_f32(object: &lopdf::Object) -> f32 {
    match object {
        lopdf::Object::Integer(value) => *value as f32,
        lopdf::Object::Real(value) => *value,
        other => panic!("unexpected page box value: {other:?}"),
    }
}

pub fn pdf_page_xobject_count(path: &std::path::Path, page_number: u32) -> usize {
    let document = lopdf::Document::load(path).unwrap();
    let page_id = document.get_pages().get(&page_number).copied().unwrap();
    let page = document.get_object(page_id).unwrap().as_dict().unwrap();
    let resources = page.get(b"Resources").unwrap().as_dict().unwrap();
    resources
        .get(b"XObject")
        .and_then(lopdf::Object::as_dict)
        .map(|dictionary| dictionary.len())
        .unwrap_or(0)
}

pub fn pdf_page_content_contains(path: &std::path::Path, page_number: u32, expected: &str) -> bool {
    let document = lopdf::Document::load(path).unwrap();
    let page_id = document.get_pages().get(&page_number).copied().unwrap();
    String::from_utf8_lossy(&document.get_page_content(page_id).unwrap()).contains(expected)
}

pub fn pdf_rgb_operator(
    path: &std::path::Path,
    page_number: u32,
    operator: &str,
) -> Option<[f32; 3]> {
    let document = lopdf::Document::load(path).unwrap();
    let page_id = document.get_pages().get(&page_number).copied().unwrap();
    let content = document.get_page_content(page_id).ok()?;
    let content = lopdf::content::Content::decode(&content).ok()?;
    content.operations.iter().find_map(|operation| {
        if operation.operator == operator && operation.operands.len() == 3 {
            Some([
                pdf_object_to_f32(&operation.operands[0]),
                pdf_object_to_f32(&operation.operands[1]),
                pdf_object_to_f32(&operation.operands[2]),
            ])
        } else {
            None
        }
    })
}
