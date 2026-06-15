fn validated_rect(left: f32, bottom: f32, right: f32, top: f32) -> Result<[f32; 4], OxideError> {
    if [left, bottom, right, top]
        .into_iter()
        .any(|value| !value.is_finite())
    {
        return Err(OxideError::InvalidInput {
            reason: "page box coordinates must be finite".to_owned(),
        });
    }
    if left >= right || bottom >= top {
        return Err(OxideError::InvalidInput {
            reason: "page box coordinates must satisfy left < right and bottom < top".to_owned(),
        });
    }

    Ok([left, bottom, right, top])
}

fn crop_box_object(rect: [f32; 4]) -> Object {
    Object::Array(rect.into_iter().map(Object::Real).collect())
}

fn normalize_rotation(degrees: i16) -> Result<i16, OxideError> {
    match degrees.rem_euclid(360) {
        90 => Ok(90),
        180 => Ok(180),
        270 => Ok(270),
        _ => Err(OxideError::InvalidInput {
            reason: "rotation must be 90, 180, or 270 degrees".to_owned(),
        }),
    }
}

fn keep_pages(document: &mut lopdf::Document, selected_pages: &[u32]) -> Result<(), OxideError> {
    let page_count = document.get_pages().len() as u32;
    if selected_pages.is_empty() {
        return Err(OxideError::InvalidInput {
            reason: "at least one page must be selected".to_owned(),
        });
    }
    let pages_before_delete = document.get_pages();
    let selected_page_ids = selected_pages
        .iter()
        .map(|page| {
            pages_before_delete
                .get(page)
                .copied()
                .ok_or_else(|| OxideError::InvalidInput {
                    reason: format!("page {page} is out of range"),
                })
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mut delete_pages = (1..=page_count)
        .filter(|page| !selected_pages.contains(page))
        .collect::<Vec<_>>();
    delete_pages.sort_unstable_by(|left, right| right.cmp(left));
    document.delete_pages(&delete_pages);
    rebuild_pages_tree(document, &selected_page_ids)
}

fn merge_documents(documents: Vec<lopdf::Document>) -> Result<lopdf::Document, OxideError> {
    let mut next_id = 1;
    let mut merged = lopdf::Document::with_version("1.7");
    let mut document_pages = BTreeMap::new();
    let mut document_objects = BTreeMap::new();

    for mut document in documents {
        document.renumber_objects_with(next_id);
        next_id = document.max_id + 1;

        for page_id in document.get_pages().into_values() {
            let page = document
                .get_object(page_id)
                .cloned()
                .map_err(|_| OxideError::ParsePdf)?;
            document_pages.insert(page_id, page);
        }
        document_objects.extend(document.objects);
    }

    let mut catalog_object = None;
    let mut pages_object = None;
    for (object_id, object) in document_objects {
        match object.type_name().unwrap_or(b"") {
            b"Catalog" => {
                if catalog_object.is_none() {
                    catalog_object = Some((object_id, object));
                }
            }
            b"Pages" => {
                if pages_object.is_none() {
                    pages_object = Some((object_id, object));
                }
            }
            b"Page" | b"Outlines" | b"Outline" => {}
            _ => {
                merged.objects.insert(object_id, object);
            }
        }
    }

    let (pages_id, pages_object) = pages_object.ok_or(OxideError::ParsePdf)?;
    for (page_id, page) in &document_pages {
        let dictionary = page.as_dict().map_err(|_| OxideError::ParsePdf)?;
        let mut dictionary = dictionary.clone();
        dictionary.set("Parent", pages_id);
        merged
            .objects
            .insert(*page_id, lopdf::Object::Dictionary(dictionary));
    }

    let mut pages_dictionary = pages_object
        .as_dict()
        .map_err(|_| OxideError::ParsePdf)?
        .clone();
    pages_dictionary.set("Count", document_pages.len() as u32);
    pages_dictionary.set(
        "Kids",
        document_pages
            .keys()
            .copied()
            .map(lopdf::Object::Reference)
            .collect::<Vec<_>>(),
    );
    merged
        .objects
        .insert(pages_id, lopdf::Object::Dictionary(pages_dictionary));

    let (catalog_id, catalog_object) = catalog_object.ok_or(OxideError::ParsePdf)?;
    let mut catalog_dictionary = catalog_object
        .as_dict()
        .map_err(|_| OxideError::ParsePdf)?
        .clone();
    catalog_dictionary.set("Pages", pages_id);
    catalog_dictionary.remove(b"Outlines");
    merged
        .objects
        .insert(catalog_id, lopdf::Object::Dictionary(catalog_dictionary));
    merged.trailer.set("Root", catalog_id);
    merged.max_id = merged
        .objects
        .keys()
        .map(|(id, _)| *id)
        .max()
        .unwrap_or_default();

    Ok(merged)
}

fn page_is_structurally_blank(
    document: &lopdf::Document,
    page_id: lopdf::ObjectId,
) -> Result<bool, OxideError> {
    let page = document
        .get_object(page_id)
        .and_then(Object::as_dict)
        .map_err(|_| OxideError::ParsePdf)?;
    let has_content = match page.get(b"Contents") {
        Ok(Object::Array(items)) => !items.is_empty(),
        Ok(Object::Stream(stream)) => !stream.content.is_empty(),
        Ok(Object::Reference(id)) => {
            let stream = document
                .get_object(*id)
                .and_then(Object::as_stream)
                .map_err(|_| OxideError::ParsePdf)?;
            !stream.content.is_empty()
        }
        Ok(Object::Null) | Err(_) => false,
        Ok(_) => true,
    };
    if has_content {
        return Ok(false);
    }
    let has_resources = match page.get(b"Resources") {
        Ok(Object::Dictionary(dictionary)) => !dictionary.is_empty(),
        Ok(Object::Reference(id)) => {
            let dictionary = document
                .get_object(*id)
                .and_then(Object::as_dict)
                .map_err(|_| OxideError::ParsePdf)?;
            !dictionary.is_empty()
        }
        Ok(_) => true,
        Err(_) => false,
    };

    Ok(!has_resources)
}

fn scale_page_boxes(
    document: &mut lopdf::Document,
    page_id: lopdf::ObjectId,
    factor: f32,
) -> Result<(), OxideError> {
    let page = document
        .get_object_mut(page_id)
        .and_then(Object::as_dict_mut)
        .map_err(|_| OxideError::ParsePdf)?;
    for key in [
        b"MediaBox".as_slice(),
        b"CropBox",
        b"BleedBox",
        b"TrimBox",
        b"ArtBox",
    ] {
        if let Ok(object) = page.get_mut(key) {
            scale_box_object(object, factor)?;
        }
    }
    Ok(())
}

fn scale_box_object(object: &mut Object, factor: f32) -> Result<(), OxideError> {
    let values = object.as_array_mut().map_err(|_| OxideError::ParsePdf)?;
    if values.len() != 4 {
        return Err(OxideError::ParsePdf);
    }
    for value in values {
        *value = Object::Real(object_to_f32(value)? * factor);
    }
    Ok(())
}

fn prepend_page_transform(
    document: &mut lopdf::Document,
    page_id: lopdf::ObjectId,
    factor: f32,
) -> Result<(), OxideError> {
    let existing = document
        .get_page_content(page_id)
        .map_err(|_| OxideError::ParsePdf)?;
    let mut operations = vec![
        lopdf::content::Operation::new("q", vec![]),
        lopdf::content::Operation::new(
            "cm",
            vec![
                Object::Real(factor),
                Object::Real(0.0),
                Object::Real(0.0),
                Object::Real(factor),
                Object::Real(0.0),
                Object::Real(0.0),
            ],
        ),
    ];
    operations.extend(
        lopdf::content::Content::decode(&existing)
            .map_err(|_| OxideError::ParsePdf)?
            .operations,
    );
    operations.push(lopdf::content::Operation::new("Q", vec![]));

    let content = lopdf::content::Content { operations }
        .encode()
        .map_err(|_| OxideError::WritePdf)?;
    let content_id = document.add_object(Stream::new(Dictionary::new(), content));
    let page = document
        .get_object_mut(page_id)
        .and_then(Object::as_dict_mut)
        .map_err(|_| OxideError::ParsePdf)?;
    page.set("Contents", Object::Reference(content_id));
    Ok(())
}

fn merge_page_resources_into(
    document: &lopdf::Document,
    page_id: lopdf::ObjectId,
    resources: &mut Dictionary,
) -> Result<(), OxideError> {
    let (direct_resources, inherited_resource_ids) = document
        .get_page_resources(page_id)
        .map_err(|_| OxideError::ParsePdf)?;
    for resource_id in inherited_resource_ids.iter().rev() {
        let inherited = document
            .get_dictionary(*resource_id)
            .map_err(|_| OxideError::ParsePdf)?;
        merge_resource_dictionary(resources, inherited);
    }
    if let Some(direct) = direct_resources {
        merge_resource_dictionary(resources, direct);
    }
    Ok(())
}
