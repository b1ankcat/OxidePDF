
pub fn pdf_with_blank_and_marked_page() -> Vec<u8> {
    let mut document = lopdf::Document::with_version("1.7");
    let pages_id = document.new_object_id();
    let blank_page_id = document.new_object_id();
    let marked_page_id = document.new_object_id();
    let marked_content_id = document.new_object_id();
    let catalog_id = document.new_object_id();
    let marked_content = lopdf::content::Content {
        operations: vec![lopdf::content::Operation::new("q", vec![])],
    }
    .encode()
    .unwrap();
    document.objects.insert(
        marked_content_id,
        lopdf::Object::Stream(lopdf::Stream::new(lopdf::Dictionary::new(), marked_content)),
    );
    document.objects.insert(
        blank_page_id,
        lopdf::Object::Dictionary(lopdf::dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => lopdf::Object::Array(vec![0.into(), 0.into(), 100.into(), 100.into()]),
        }),
    );
    document.objects.insert(
        marked_page_id,
        lopdf::Object::Dictionary(lopdf::dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => lopdf::Object::Array(vec![0.into(), 0.into(), 100.into(), 100.into()]),
            "Contents" => marked_content_id,
        }),
    );
    document.objects.insert(
        pages_id,
        lopdf::Object::Dictionary(lopdf::dictionary! {
            "Type" => "Pages",
            "Kids" => lopdf::Object::Array(vec![blank_page_id.into(), marked_page_id.into()]),
            "Count" => 2,
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

pub fn empty_page_pdf() -> Vec<u8> {
    let mut document = lopdf::Document::with_version("1.7");
    let pages_id = document.new_object_id();
    let page_id = document.new_object_id();
    let catalog_id = document.new_object_id();
    document.objects.insert(
        page_id,
        lopdf::Object::Dictionary(lopdf::dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => lopdf::Object::Array(vec![0.into(), 0.into(), 595.into(), 842.into()]),
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

pub fn form_pdf(readonly: bool) -> Vec<u8> {
    let mut document = lopdf::Document::with_version("1.7");
    let pages_id = document.new_object_id();
    let page_id = document.new_object_id();
    let field_id = document.new_object_id();
    let acroform_id = document.new_object_id();
    let catalog_id = document.new_object_id();
    let flags = if readonly { 1 } else { 0 };

    document.objects.insert(
        field_id,
        lopdf::Object::Dictionary(lopdf::dictionary! {
            "FT" => "Tx",
            "T" => lopdf::Object::string_literal("customer"),
            "V" => lopdf::Object::string_literal(""),
            "Ff" => flags,
            "Rect" => lopdf::Object::Array(vec![10.into(), 10.into(), 120.into(), 30.into()]),
            "P" => page_id,
        }),
    );
    document.objects.insert(
        page_id,
        lopdf::Object::Dictionary(lopdf::dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => lopdf::Object::Array(vec![0.into(), 0.into(), 200.into(), 200.into()]),
            "Annots" => lopdf::Object::Array(vec![field_id.into()]),
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
        acroform_id,
        lopdf::Object::Dictionary(lopdf::dictionary! {
            "Fields" => lopdf::Object::Array(vec![field_id.into()]),
        }),
    );
    document.objects.insert(
        catalog_id,
        lopdf::Object::Dictionary(lopdf::dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
            "AcroForm" => acroform_id,
        }),
    );
    document.trailer.set("Root", catalog_id);

    let mut bytes = Vec::new();
    document.save_to(&mut bytes).unwrap();
    bytes
}

pub fn pdf_with_rgb_fill_content() -> Vec<u8> {
    let mut document = lopdf::Document::with_version("1.7");
    let pages_id = document.new_object_id();
    let page_id = document.new_object_id();
    let content_id = document.new_object_id();
    let catalog_id = document.new_object_id();
    let content = lopdf::content::Content {
        operations: vec![
            lopdf::content::Operation::new(
                "rg",
                vec![
                    lopdf::Object::Real(1.0),
                    lopdf::Object::Real(0.0),
                    lopdf::Object::Real(0.0),
                ],
            ),
            lopdf::content::Operation::new("re", vec![0.into(), 0.into(), 100.into(), 100.into()]),
            lopdf::content::Operation::new("f", Vec::new()),
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
            "MediaBox" => lopdf::Object::Array(vec![0.into(), 0.into(), 100.into(), 100.into()]),
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

pub fn page_has_content_operator(path: &std::path::Path, page_number: u32, operator: &str) -> bool {
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
