
fn imposed_page_operations(
    resource_name: &[u8],
    slot: usize,
    columns: u32,
    rows: u32,
    layout: PageLayout,
    source_width: f32,
    source_height: f32,
) -> Result<Vec<lopdf::content::Operation>, OxideError> {
    if !source_width.is_finite()
        || !source_height.is_finite()
        || source_width <= 0.0
        || source_height <= 0.0
    {
        return Err(OxideError::ParsePdf);
    }
    let cell_width = layout.output_width / columns as f32;
    let cell_height = layout.output_height / rows as f32;
    let column = (slot as u32) % columns;
    let row_from_top = (slot as u32) / columns;
    let scale = (cell_width / source_width).min(cell_height / source_height);
    let width = source_width * scale;
    let height = source_height * scale;
    let x = column as f32 * cell_width + (cell_width - width) / 2.0;
    let y = layout.output_height - (row_from_top + 1) as f32 * cell_height
        + (cell_height - height) / 2.0;

    Ok(vec![
        lopdf::content::Operation::new("q", vec![]),
        lopdf::content::Operation::new(
            "cm",
            vec![
                Object::Real(scale),
                Object::Real(0.0),
                Object::Real(0.0),
                Object::Real(scale),
                Object::Real(x),
                Object::Real(y),
            ],
        ),
        lopdf::content::Operation::new("Do", vec![Object::Name(resource_name.to_vec())]),
        lopdf::content::Operation::new("Q", vec![]),
    ])
}

fn page_form_xobject_from_source(
    source: &lopdf::Document,
    target: &mut lopdf::Document,
    page_id: lopdf::ObjectId,
    imported: &mut BTreeMap<lopdf::ObjectId, lopdf::ObjectId>,
) -> Result<lopdf::ObjectId, OxideError> {
    let content = source
        .get_page_content(page_id)
        .map_err(|_| OxideError::ParsePdf)?;
    let resources = imported_page_resources_with_cache(source, target, page_id, imported)?;
    let (width, height) = page_size(source, page_id)?;
    if width <= 0.0 || height <= 0.0 {
        return Err(OxideError::ParsePdf);
    }
    let mut dictionary = dictionary! {
        "Type" => "XObject",
        "Subtype" => "Form",
        "BBox" => crop_box_object([0.0, 0.0, width, height]),
        "Matrix" => Object::Array(vec![
            Object::Real(1.0),
            Object::Real(0.0),
            Object::Real(0.0),
            Object::Real(1.0),
            Object::Real(0.0),
            Object::Real(0.0),
        ]),
    };
    dictionary.set("Resources", resources);
    Ok(target.add_object(Stream::new(dictionary, content)))
}

fn imported_page_resources_with_cache(
    source: &lopdf::Document,
    target: &mut lopdf::Document,
    page_id: lopdf::ObjectId,
    imported: &mut BTreeMap<lopdf::ObjectId, lopdf::ObjectId>,
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
    remap_imported_references(&mut resource_object, source, target, imported)?;
    resource_object
        .as_dict()
        .cloned()
        .map_err(|_| OxideError::ParsePdf)
}
