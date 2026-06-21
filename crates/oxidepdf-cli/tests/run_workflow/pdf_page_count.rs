
fn pdf_page_count(path: &std::path::Path) -> usize {
    lopdf::Document::load(path).unwrap().get_pages().len()
}

fn pdf_page_rotation(path: &std::path::Path, page_number: u32) -> i64 {
    let document = lopdf::Document::load(path).unwrap();
    let page_id = document.get_pages().get(&page_number).copied().unwrap();
    let page = document.get_object(page_id).unwrap().as_dict().unwrap();
    page.get(b"Rotate")
        .and_then(lopdf::Object::as_i64)
        .unwrap_or(0)
}

fn pdf_page_box(path: &std::path::Path, page_number: u32, key: &[u8]) -> [f32; 4] {
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

fn pdf_object_to_f32(object: &lopdf::Object) -> f32 {
    match object {
        lopdf::Object::Integer(value) => *value as f32,
        lopdf::Object::Real(value) => *value,
        other => panic!("unexpected page box value: {other:?}"),
    }
}

fn page_content_contains(path: &std::path::Path, page_number: u32, expected: &str) -> bool {
    let document = lopdf::Document::load(path).unwrap();
    let page_id = document.get_pages().get(&page_number).copied().unwrap();
    String::from_utf8_lossy(&document.get_page_content(page_id).unwrap()).contains(expected)
}

fn page_has_content_operator(path: &std::path::Path, page_number: u32, operator: &str) -> bool {
    let document = lopdf::Document::load(path).unwrap();
    let page_id = document.get_pages().get(&page_number).copied().unwrap();
    document
        .get_page_contents(page_id)
        .into_iter()
        .filter_map(|content_id| document.get_object(content_id).ok())
        .filter_map(|object| object.as_stream().ok())
        .filter_map(|stream| lopdf::content::Content::decode(&stream.content).ok())
        .flat_map(|content| content.operations)
        .any(|operation| operation.operator == operator)
}

fn pdf_with_rgb_image() -> Vec<u8> {
    let mut document = lopdf::Document::with_version("1.7");
    let pages_id = document.new_object_id();
    let page_id = document.new_object_id();
    let image_id = document.new_object_id();
    let content_id = document.new_object_id();
    let catalog_id = document.new_object_id();
    document.objects.insert(
        image_id,
        lopdf::Object::Stream(lopdf::Stream::new(
            lopdf::dictionary! {
                "Type" => "XObject",
                "Subtype" => "Image",
                "Width" => 1,
                "Height" => 3,
                "ColorSpace" => "DeviceRGB",
                "BitsPerComponent" => 8,
            },
            b"rgbpixel!".to_vec(),
        )),
    );
    document.objects.insert(
        content_id,
        lopdf::Object::Stream(lopdf::Stream::new(
            lopdf::Dictionary::new(),
            b"q 1 0 0 3 0 0 cm /Im1 Do Q".to_vec(),
        )),
    );
    document.objects.insert(
        page_id,
        lopdf::Object::Dictionary(lopdf::dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => lopdf::Object::Array(vec![0.into(), 0.into(), 10.into(), 10.into()]),
            "Resources" => lopdf::Object::Dictionary(lopdf::dictionary! {
                "XObject" => lopdf::Object::Dictionary(lopdf::dictionary! {
                    "Im1" => image_id,
                }),
            }),
            "Contents" => content_id,
        }),
    );
    document.objects.insert(
        pages_id,
        lopdf::Object::Dictionary(lopdf::dictionary! {
            "Type" => "Pages",
            "Kids" => lopdf::Object::Array(vec![page_id.into()]),
            "Count" => 1,
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
