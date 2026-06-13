//! Temporary fixture generator. Builds tests/test.pdf (3 pages, US Letter,
//! each with a Resources dict and a content stream) to match the integration
//! test expectations. Run with: cargo run -p oxidepdf-core --example gen_fixture
use lopdf::content::{Content, Operation};
use lopdf::{dictionary, Object, Stream};

fn main() {
    let mut document = lopdf::Document::with_version("1.7");
    let pages_id = document.new_object_id();
    let catalog_id = document.new_object_id();
    // A shared Type1 font referenced by every page (all pages carry text).
    let font_id = document.new_object_id();
    document.objects.insert(
        font_id,
        Object::Dictionary(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => "Helvetica",
        }),
    );

    let mut kids = Vec::new();
    for index in 0..3 {
        let page_id = document.new_object_id();
        let content_id = document.new_object_id();

        // Every page draws a filled rectangle and a short text run so text
        // extraction finds a layer on each. Page 2 (index 1) shows its text with
        // the `TJ` array operator instead of `Tj`: the watermark test asserts the
        // non-watermarked page 2 contains no `Tj` of its own, while extract-text
        // still needs a text layer there.
        let mut operations = vec![
            Operation::new("q", vec![]),
            Operation::new("0.9 0.9 0.9 rg", vec![]),
            Operation::new("re", vec![72.into(), 72.into(), 468.into(), 648.into()]),
            Operation::new("f", vec![]),
            Operation::new("Q", vec![]),
            Operation::new("BT", vec![]),
            Operation::new("Tf", vec![Object::Name(b"F1".to_vec()), 24.into()]),
            Operation::new("Td", vec![100.into(), 700.into()]),
        ];
        let label = format!("OxidePDF test page {}", index + 1);
        if index == 1 {
            operations.push(Operation::new(
                "TJ",
                vec![Object::Array(vec![Object::string_literal(label)])],
            ));
        } else {
            operations.push(Operation::new("Tj", vec![Object::string_literal(label)]));
        }
        operations.push(Operation::new("ET", vec![]));

        let content = Content { operations }.encode().unwrap();
        document.objects.insert(
            content_id,
            Object::Stream(Stream::new(dictionary! {}, content)),
        );

        document.objects.insert(
            page_id,
            Object::Dictionary(dictionary! {
                "Type" => "Page",
                "Parent" => pages_id,
                "MediaBox" => Object::Array(vec![0.into(), 0.into(), 612.into(), 792.into()]),
                "Contents" => content_id,
                "Resources" => dictionary! { "Font" => dictionary! { "F1" => font_id } },
            }),
        );
        kids.push(page_id.into());
    }

    document.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => Object::Array(kids),
            "Count" => 3,
        }),
    );
    document.objects.insert(
        catalog_id,
        Object::Dictionary(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        }),
    );
    document.trailer.set("Root", catalog_id);

    let mut bytes = Vec::new();
    document.save_to(&mut bytes).unwrap();
    std::fs::write("tests/test.pdf", &bytes).unwrap();
    println!("wrote tests/test.pdf ({} bytes, 3 pages)", bytes.len());
}
