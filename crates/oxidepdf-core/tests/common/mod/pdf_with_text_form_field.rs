
pub fn pdf_with_text_form_field(readonly: bool) -> Vec<u8> {
    let mut document = lopdf::Document::with_version("1.7");
    let pages_id = document.new_object_id();
    let page_id = document.new_object_id();
    let field_id = document.new_object_id();
    let acroform_id = document.new_object_id();
    let catalog_id = document.new_object_id();
    let flags = if readonly { 1 } else { 0 };

    document.objects.insert(
        field_id,
        Object::Dictionary(lopdf::dictionary! {
            "FT" => "Tx",
            "T" => Object::string_literal("customer"),
            "V" => Object::string_literal(""),
            "Ff" => flags,
            "Rect" => Object::Array(vec![10.into(), 10.into(), 120.into(), 30.into()]),
            "P" => page_id,
        }),
    );
    document.objects.insert(
        page_id,
        Object::Dictionary(lopdf::dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => Object::Array(vec![0.into(), 0.into(), 200.into(), 200.into()]),
            "Annots" => Object::Array(vec![field_id.into()]),
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
        acroform_id,
        Object::Dictionary(lopdf::dictionary! {
            "Fields" => Object::Array(vec![field_id.into()]),
        }),
    );
    document.objects.insert(
        catalog_id,
        Object::Dictionary(lopdf::dictionary! {
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

pub fn pdf_with_named_outline_destination() -> Vec<u8> {
    let mut document = lopdf::Document::load_mem(&empty_page_pdf()).unwrap();
    let outline_id = document.new_object_id();
    let item_id = document.new_object_id();
    document.objects.insert(
        item_id,
        Object::Dictionary(lopdf::dictionary! {
            "Title" => Object::string_literal("Named destination"),
            "Parent" => outline_id,
            "Dest" => Object::Name(b"named-destination".to_vec()),
        }),
    );
    document.objects.insert(
        outline_id,
        Object::Dictionary(lopdf::dictionary! {
            "Type" => "Outlines",
            "First" => item_id,
            "Last" => item_id,
            "Count" => 1,
        }),
    );
    document
        .catalog_mut()
        .unwrap()
        .set("Outlines", Object::Reference(outline_id));

    let mut bytes = Vec::new();
    document.save_to(&mut bytes).unwrap();
    bytes
}

pub fn pdf_with_cyclic_outline() -> Vec<u8> {
    let mut document = lopdf::Document::load_mem(&empty_page_pdf()).unwrap();
    let page_id = *document.get_pages().get(&1).unwrap();
    let outline_id = document.new_object_id();
    let first_id = document.new_object_id();
    let second_id = document.new_object_id();
    let explicit_dest = || {
        Object::Array(vec![
            Object::Reference(page_id),
            Object::Name(b"Fit".to_vec()),
        ])
    };
    document.objects.insert(
        first_id,
        Object::Dictionary(lopdf::dictionary! {
            "Title" => Object::string_literal("First"),
            "Parent" => outline_id,
            "Dest" => explicit_dest(),
            "Next" => Object::Reference(second_id),
        }),
    );
    // `second` points its `Next` back at `first`, forming a sibling cycle that
    // would loop forever without the visited-set guard.
    document.objects.insert(
        second_id,
        Object::Dictionary(lopdf::dictionary! {
            "Title" => Object::string_literal("Second"),
            "Parent" => outline_id,
            "Dest" => explicit_dest(),
            "Next" => Object::Reference(first_id),
        }),
    );
    document.objects.insert(
        outline_id,
        Object::Dictionary(lopdf::dictionary! {
            "Type" => "Outlines",
            "First" => first_id,
            "Last" => second_id,
            "Count" => 2,
        }),
    );
    document
        .catalog_mut()
        .unwrap()
        .set("Outlines", Object::Reference(outline_id));

    let mut bytes = Vec::new();
    document.save_to(&mut bytes).unwrap();
    bytes
}

pub fn pdf_with_checkbox_form_field() -> Vec<u8> {
    let mut document = lopdf::Document::with_version("1.7");
    let pages_id = document.new_object_id();
    let page_id = document.new_object_id();
    let field_id = document.new_object_id();
    let acroform_id = document.new_object_id();
    let catalog_id = document.new_object_id();

    // A checkbox stores its value as a name (e.g. /Yes or /Off), not a string.
    document.objects.insert(
        field_id,
        Object::Dictionary(lopdf::dictionary! {
            "FT" => "Btn",
            "T" => Object::string_literal("subscribe"),
            "V" => Object::Name(b"Yes".to_vec()),
            "Rect" => Object::Array(vec![10.into(), 10.into(), 30.into(), 30.into()]),
            "P" => page_id,
        }),
    );
    document.objects.insert(
        page_id,
        Object::Dictionary(lopdf::dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => Object::Array(vec![0.into(), 0.into(), 200.into(), 200.into()]),
            "Annots" => Object::Array(vec![field_id.into()]),
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
        acroform_id,
        Object::Dictionary(lopdf::dictionary! {
            "Fields" => Object::Array(vec![field_id.into()]),
        }),
    );
    document.objects.insert(
        catalog_id,
        Object::Dictionary(lopdf::dictionary! {
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

pub fn pdf_with_named_info_value() -> Vec<u8> {
    let mut document = lopdf::Document::load_mem(&empty_page_pdf()).unwrap();
    // /Trapped is a name-valued Info entry; a string-only reader rejects it.
    let info_id = document.add_object(Object::Dictionary(lopdf::dictionary! {
        "Title" => Object::string_literal("Sample"),
        "Trapped" => Object::Name(b"True".to_vec()),
    }));
    document.trailer.set("Info", info_id);

    let mut bytes = Vec::new();
    document.save_to(&mut bytes).unwrap();
    bytes
}

pub fn pdf_with_malformed_names_tree() -> Vec<u8> {
    let mut document = lopdf::Document::load_mem(&empty_page_pdf()).unwrap();
    document
        .catalog_mut()
        .unwrap()
        .set("Names", Object::string_literal("malformed names tree"));

    let mut bytes = Vec::new();
    document.save_to(&mut bytes).unwrap();
    bytes
}

pub fn pdf_with_malformed_annotation_array() -> Vec<u8> {
    let mut document = lopdf::Document::load_mem(&empty_page_pdf()).unwrap();
    let page_id = *document.get_pages().get(&1).unwrap();
    document
        .get_object_mut(page_id)
        .unwrap()
        .as_dict_mut()
        .unwrap()
        .set("Annots", Object::string_literal("malformed annotations"));

    let mut bytes = Vec::new();
    document.save_to(&mut bytes).unwrap();
    bytes
}

pub fn pdf_with_malformed_acroform() -> Vec<u8> {
    let mut document = lopdf::Document::load_mem(&empty_page_pdf()).unwrap();
    document
        .catalog_mut()
        .unwrap()
        .set("AcroForm", Object::string_literal("malformed acroform"));

    let mut bytes = Vec::new();
    document.save_to(&mut bytes).unwrap();
    bytes
}

pub fn pdf_with_malformed_xobject_resources() -> Vec<u8> {
    let mut document = lopdf::Document::load_mem(&empty_page_pdf()).unwrap();
    let page_id = *document.get_pages().get(&1).unwrap();
    let page = document
        .get_object_mut(page_id)
        .unwrap()
        .as_dict_mut()
        .unwrap();
    page.set(
        "Resources",
        Object::Dictionary(lopdf::dictionary! {
            "XObject" => Object::string_literal("malformed xobject dictionary"),
        }),
    );

    let mut bytes = Vec::new();
    document.save_to(&mut bytes).unwrap();
    bytes
}
