fn signature_field_id_by_name(
    document: &lopdf::Document,
    field_name: &str,
) -> Result<lopdf::ObjectId, OxideError> {
    let mut matches = Vec::new();
    if let Ok(catalog) = document.catalog()
        && let Ok(acroform) = catalog
            .get(b"AcroForm")
            .and_then(|object| deref_dictionary(document, object))
        && let Ok(fields) = acroform.get(b"Fields").and_then(lopdf::Object::as_array)
    {
        for field in fields {
            collect_signature_field_ids(document, field, None, field_name, &mut matches)?;
        }
    }

    matches.sort_unstable();
    matches.dedup();
    match matches.as_slice() {
        [field_id] => Ok(*field_id),
        [] => Err(OxideError::InvalidInput {
            reason: "signature field was not found".to_owned(),
        }),
        _ => Err(OxideError::InvalidInput {
            reason: "signature field name is ambiguous".to_owned(),
        }),
    }
}

fn collect_signature_field_ids(
    document: &lopdf::Document,
    object: &lopdf::Object,
    inherited_name: Option<String>,
    target_name: &str,
    matches: &mut Vec<lopdf::ObjectId>,
) -> Result<(), OxideError> {
    let field_id = match object {
        lopdf::Object::Reference(id) => Some(*id),
        _ => None,
    };
    let dictionary = deref_dictionary(document, object).map_err(|_| OxideError::ParsePdf)?;
    let field_name = dictionary
        .get(b"T")
        .ok()
        .and_then(pdf_string)
        .or(inherited_name);
    if dictionary.get(b"FT").and_then(lopdf::Object::as_name).ok() == Some(b"Sig")
        && field_name.as_deref() == Some(target_name)
        && let Some(field_id) = field_id
    {
        matches.push(field_id);
    }
    if let Ok(kids) = dictionary.get(b"Kids").and_then(lopdf::Object::as_array) {
        for kid in kids {
            collect_signature_field_ids(document, kid, field_name.clone(), target_name, matches)?;
        }
    }

    Ok(())
}

fn signature_field_has_value_material(field: &Dictionary) -> bool {
    field.get(b"V").is_ok() || (field.get(b"ByteRange").is_ok() && field.get(b"Contents").is_ok())
}

fn remove_signature_field_references(
    document: &mut lopdf::Document,
    field_id: lopdf::ObjectId,
) -> Result<(), OxideError> {
    remove_from_acroform_fields(document, field_id)?;
    for (_, page_id) in document.get_pages() {
        let Ok(page) = document
            .get_object_mut(page_id)
            .and_then(lopdf::Object::as_dict_mut)
        else {
            continue;
        };
        remove_reference_from_array_entry(page, b"Annots", field_id);
    }
    for object in document.objects.values_mut() {
        if let Ok(dictionary) = object.as_dict_mut() {
            remove_reference_from_array_entry(dictionary, b"Kids", field_id);
        }
    }

    Ok(())
}

fn remove_from_acroform_fields(
    document: &mut lopdf::Document,
    field_id: lopdf::ObjectId,
) -> Result<(), OxideError> {
    let acroform_id = {
        let catalog = document.catalog().map_err(|_| OxideError::ParsePdf)?;
        match catalog.get(b"AcroForm") {
            Ok(lopdf::Object::Reference(id)) => Some(*id),
            Ok(_) => None,
            Err(_) => None,
        }
    };
    let Some(acroform_id) = acroform_id else {
        return Ok(());
    };
    let acroform = document
        .get_object_mut(acroform_id)
        .and_then(lopdf::Object::as_dict_mut)
        .map_err(|_| OxideError::ParsePdf)?;
    remove_reference_from_array_entry(acroform, b"Fields", field_id);

    Ok(())
}

fn remove_reference_from_array_entry(
    dictionary: &mut Dictionary,
    key: &[u8],
    field_id: lopdf::ObjectId,
) {
    let Ok(array) = dictionary
        .get_mut(key)
        .and_then(lopdf::Object::as_array_mut)
    else {
        return;
    };
    array.retain(|object| !matches!(object, lopdf::Object::Reference(id) if *id == field_id));
}
