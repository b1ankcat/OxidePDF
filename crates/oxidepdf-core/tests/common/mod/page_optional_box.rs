
pub fn page_optional_box(
    document: &lopdf::Document,
    page_number: u32,
    key: &[u8],
) -> Option<[f32; 4]> {
    let page_id = document.get_pages().get(&page_number).copied().unwrap();
    let page = document.get_object(page_id).unwrap().as_dict().unwrap();
    let values = page.get(key).ok()?.as_array().ok()?;
    Some([
        object_to_f32(&values[0]).unwrap(),
        object_to_f32(&values[1]).unwrap(),
        object_to_f32(&values[2]).unwrap(),
        object_to_f32(&values[3]).unwrap(),
    ])
}

pub fn page_box(document: &lopdf::Document, page_number: u32, key: &[u8]) -> [f32; 4] {
    page_optional_box(document, page_number, key).unwrap()
}

pub fn page_content_contains(document: &lopdf::Document, page_number: u32, operator: &str) -> bool {
    let page_id = document.get_pages().get(&page_number).copied().unwrap();
    document
        .get_page_content(page_id)
        .ok()
        .and_then(|content| lopdf::content::Content::decode(&content).ok())
        .is_some_and(|content| {
            content
                .operations
                .iter()
                .any(|operation| operation.operator == operator)
        })
}

pub fn page_rgb_operator(
    document: &lopdf::Document,
    page_number: u32,
    operator: &str,
) -> Option<[f32; 3]> {
    let page_id = document.get_pages().get(&page_number).copied().unwrap();
    let content = document.get_page_content(page_id).ok()?;
    let content = lopdf::content::Content::decode(&content).ok()?;
    content.operations.iter().find_map(|operation| {
        if operation.operator == operator && operation.operands.len() == 3 {
            Some([
                object_to_f32(&operation.operands[0]).ok()?,
                object_to_f32(&operation.operands[1]).ok()?,
                object_to_f32(&operation.operands[2]).ok()?,
            ])
        } else {
            None
        }
    })
}

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
        Object::Stream(Stream::new(Dictionary::new(), marked_content)),
    );
    document.objects.insert(
        blank_page_id,
        Object::Dictionary(lopdf::dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => Object::Array(vec![0.into(), 0.into(), 100.into(), 100.into()]),
        }),
    );
    document.objects.insert(
        marked_page_id,
        Object::Dictionary(lopdf::dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => Object::Array(vec![0.into(), 0.into(), 100.into(), 100.into()]),
            "Contents" => marked_content_id,
        }),
    );
    document.objects.insert(
        pages_id,
        Object::Dictionary(lopdf::dictionary! {
            "Type" => "Pages",
            "Kids" => Object::Array(vec![blank_page_id.into(), marked_page_id.into()]),
            "Count" => 2,
        }),
    );
    document.objects.insert(
        catalog_id,
        Object::Dictionary(lopdf::dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        }),
    );
    document.trailer.set("Root", catalog_id);

    let mut bytes = Vec::new();
    document.save_to(&mut bytes).unwrap();
    bytes
}

pub fn pdf_with_blank_page_and_missing_resources() -> Vec<u8> {
    let mut document = lopdf::Document::with_version("1.7");
    let pages_id = document.new_object_id();
    let page_id = document.new_object_id();
    let catalog_id = document.new_object_id();

    document.objects.insert(
        page_id,
        Object::Dictionary(lopdf::dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => Object::Array(vec![0.into(), 0.into(), 100.into(), 100.into()]),
            "Resources" => Object::Reference((99, 0)),
        }),
    );
    document.objects.insert(
        pages_id,
        Object::Dictionary(lopdf::dictionary! {
            "Type" => "Pages",
            "Kids" => Object::Array(vec![page_id.into()]),
            "Count" => 1,
        }),
    );
    document.objects.insert(
        catalog_id,
        Object::Dictionary(lopdf::dictionary! {
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
    let mut pdf = pdf_writer::Pdf::new();
    let catalog_id = pdf_writer::Ref::new(1);
    let pages_id = pdf_writer::Ref::new(2);
    let page_id = pdf_writer::Ref::new(3);

    pdf.catalog(catalog_id).pages(pages_id);
    pdf.pages(pages_id).kids([page_id]).count(1);
    let mut page = pdf.page(page_id);
    page.media_box(pdf_writer::Rect::new(0.0, 0.0, A4_WIDTH, A4_HEIGHT));
    page.parent(pages_id);
    page.finish();

    pdf.finish()
}

pub fn fixture_pdf() -> &'static [u8] {
    static PDF: std::sync::OnceLock<Vec<u8>> = std::sync::OnceLock::new();
    PDF.get_or_init(three_page_text_pdf)
}

pub fn fixture_jpg() -> &'static [u8] {
    static JPG: std::sync::OnceLock<Vec<u8>> = std::sync::OnceLock::new();
    JPG.get_or_init(test_jpeg)
}

pub fn fixture_signature_pdf() -> &'static [u8] {
    static PDF: std::sync::OnceLock<Vec<u8>> = std::sync::OnceLock::new();
    PDF.get_or_init(|| {
        let mut bytes = b"%PDF-1.7
1 0 obj
<< /Type /Sig /SubFilter /adbe.pkcs7.detached /ByteRange [0 64 192 64] /Contents <3082> >>
endobj
%%EOF"
            .to_vec();
        bytes.resize(256, b' ');
        bytes
    })
}

pub fn blank_three_page_pdf() -> Vec<u8> {
    let mut document = lopdf::Document::with_version("1.7");
    let pages_id = document.new_object_id();
    let catalog_id = document.new_object_id();
    let mut page_ids = Vec::new();

    for _ in 0..3 {
        let page_id = document.new_object_id();
        document.objects.insert(
            page_id,
            Object::Dictionary(lopdf::dictionary! {
                "Type" => "Page",
                "Parent" => pages_id,
                "MediaBox" => Object::Array(vec![0.into(), 0.into(), 612.into(), 792.into()]),
            }),
        );
        page_ids.push(page_id);
    }

    document.objects.insert(
        pages_id,
        Object::Dictionary(lopdf::dictionary! {
            "Type" => "Pages",
            "Kids" => Object::Array(page_ids.iter().copied().map(Object::Reference).collect()),
            "Count" => page_ids.len() as i64,
        }),
    );
    document.objects.insert(
        catalog_id,
        Object::Dictionary(lopdf::dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        }),
    );
    document.trailer.set("Root", catalog_id);

    let mut bytes = Vec::new();
    document.save_to(&mut bytes).unwrap();
    bytes
}
