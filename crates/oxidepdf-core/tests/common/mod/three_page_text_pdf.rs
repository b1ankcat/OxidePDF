
fn three_page_text_pdf() -> Vec<u8> {
    let mut document = lopdf::Document::with_version("1.7");
    let pages_id = document.new_object_id();
    let font_id = document.new_object_id();
    let catalog_id = document.new_object_id();
    let mut page_ids = Vec::new();

    document.objects.insert(
        font_id,
        Object::Dictionary(lopdf::dictionary! {
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
                lopdf::content::Operation::new("Tf", vec![Object::Name(b"F1".to_vec()), 12.into()]),
                lopdf::content::Operation::new("Td", vec![72.into(), 720.into()]),
                lopdf::content::Operation::new(
                    "Tj",
                    vec![Object::string_literal(format!(
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
            Object::Stream(Stream::new(Dictionary::new(), content)),
        );
        document.objects.insert(
            page_id,
            Object::Dictionary(lopdf::dictionary! {
                "Type" => "Page",
                "Parent" => pages_id,
                "MediaBox" => Object::Array(vec![0.into(), 0.into(), 612.into(), 792.into()]),
                "Resources" => Object::Dictionary(lopdf::dictionary! {
                    "Font" => Object::Dictionary(lopdf::dictionary! {
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

fn test_jpeg() -> Vec<u8> {
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

pub fn pdf_with_media_box(width: i64, height: i64) -> Vec<u8> {
    let mut document = lopdf::Document::load_mem(&empty_page_pdf()).unwrap();
    let page_id = *document.get_pages().get(&1).unwrap();
    document
        .get_object_mut(page_id)
        .unwrap()
        .as_dict_mut()
        .unwrap()
        .set(
            "MediaBox",
            Object::Array(vec![0.into(), 0.into(), width.into(), height.into()]),
        );

    let mut bytes = Vec::new();
    document.save_to(&mut bytes).unwrap();
    bytes
}

pub fn pdf_with_unreferenced_stream_object() -> Vec<u8> {
    let mut document = lopdf::Document::load_mem(&empty_page_pdf()).unwrap();
    let unused_id = document.new_object_id();
    document.objects.insert(
        unused_id,
        Object::Stream(Stream::new(Dictionary::new(), b"unused".to_vec())),
    );

    let mut bytes = Vec::new();
    document.save_to(&mut bytes).unwrap();
    bytes
}

pub fn pdf_with_large_plain_content_stream() -> Vec<u8> {
    let content = b"0 0 0 rg\n0 0 100 100 re f\n".repeat(64);
    pdf_with_content_stream(Stream::new(Dictionary::new(), content))
}

pub fn pdf_with_unsupported_filtered_stream() -> Vec<u8> {
    let mut stream = Stream::new(Dictionary::new(), b"not jpeg data".to_vec());
    stream.dict.set("Filter", "DCTDecode");
    pdf_with_content_stream(stream)
}

pub fn pdf_with_content_stream(stream: Stream) -> Vec<u8> {
    let mut document = lopdf::Document::with_version("1.7");
    let pages_id = document.new_object_id();
    let page_id = document.new_object_id();
    let content_id = document.new_object_id();
    let catalog_id = document.new_object_id();

    document.objects.insert(content_id, Object::Stream(stream));
    document.objects.insert(
        page_id,
        Object::Dictionary(lopdf::dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => Object::Array(vec![0.into(), 0.into(), 100.into(), 100.into()]),
            "Contents" => content_id,
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

pub fn pdf_with_duplicate_image_resources() -> Vec<u8> {
    let mut document = lopdf::Document::load_mem(&empty_page_pdf()).unwrap();
    let page_id = *document.get_pages().get(&1).unwrap();
    let left_id = document.add_object(test_image_stream());
    let right_id = document.add_object(test_image_stream());

    document
        .get_object_mut(page_id)
        .unwrap()
        .as_dict_mut()
        .unwrap()
        .set(
            "Resources",
            Object::Dictionary(lopdf::dictionary! {
                "XObject" => Object::Dictionary(lopdf::dictionary! {
                    "Left" => left_id,
                    "Right" => right_id,
                }),
            }),
        );

    let mut bytes = Vec::new();
    document.save_to(&mut bytes).unwrap();
    bytes
}

pub fn test_image_stream() -> Stream {
    Stream::new(
        lopdf::dictionary! {
            "Type" => "XObject",
            "Subtype" => "Image",
            "Width" => 1,
            "Height" => 3,
            "ColorSpace" => "DeviceRGB",
            "BitsPerComponent" => 8,
        },
        b"rgbpixel!".to_vec(),
    )
}

pub fn first_page_content_stream(document: &lopdf::Document) -> &Stream {
    let page_id = *document.get_pages().get(&1).unwrap();
    let content_id = document
        .get_dictionary(page_id)
        .unwrap()
        .get(b"Contents")
        .unwrap()
        .as_reference()
        .unwrap();
    document
        .get_object(content_id)
        .unwrap()
        .as_stream()
        .unwrap()
}

pub fn duplicate_image_resource_ids(
    document: &lopdf::Document,
) -> (lopdf::ObjectId, lopdf::ObjectId) {
    let resources = page_resources(document, 1);
    let xobjects = resources.get(b"XObject").unwrap().as_dict().unwrap();
    (
        xobjects.get(b"Left").unwrap().as_reference().unwrap(),
        xobjects.get(b"Right").unwrap().as_reference().unwrap(),
    )
}

pub fn metadata_entries<const N: usize>(entries: [(&str, &str); N]) -> Vec<MetadataEntry> {
    entries
        .into_iter()
        .map(|(key, value)| MetadataEntry {
            key: key.to_owned(),
            value: value.to_owned(),
        })
        .collect()
}
