fn page_image_xobjects(
    document: &lopdf::Document,
    page_id: lopdf::ObjectId,
) -> Result<Vec<(Vec<u8>, lopdf::ObjectId, Dictionary)>, OxideError> {
    let (direct_resources, inherited_resource_ids) = document
        .get_page_resources(page_id)
        .map_err(|_| OxideError::ParsePdf)?;
    let mut resources = Dictionary::new();
    for resource_id in inherited_resource_ids.iter().rev() {
        let inherited = document
            .get_dictionary(*resource_id)
            .map_err(|_| OxideError::ParsePdf)?;
        merge_resource_dictionary(&mut resources, inherited);
    }
    if let Some(direct) = direct_resources {
        merge_resource_dictionary(&mut resources, direct);
    }
    let Some(xobjects) = optional_xobject_dict(&resources)? else {
        return Ok(Vec::new());
    };
    let mut images = Vec::new();
    for (name, object) in xobjects.iter() {
        let id = object.as_reference().map_err(|_| OxideError::ParsePdf)?;
        let stream = document
            .get_object(id)
            .and_then(Object::as_stream)
            .map_err(|_| OxideError::ParsePdf)?;
        if stream
            .dict
            .get(b"Subtype")
            .and_then(Object::as_name)
            .is_ok_and(|subtype| subtype == b"Image")
        {
            images.push((name.clone(), id, stream.dict.clone()));
        }
    }
    Ok(images)
}

fn page_xobject_reference(
    document: &lopdf::Document,
    page_id: lopdf::ObjectId,
    name: &[u8],
) -> Result<Option<lopdf::ObjectId>, OxideError> {
    let (direct_resources, inherited_resource_ids) = document
        .get_page_resources(page_id)
        .map_err(|_| OxideError::ParsePdf)?;
    let mut resources = Dictionary::new();
    for resource_id in inherited_resource_ids.iter().rev() {
        let inherited = document
            .get_dictionary(*resource_id)
            .map_err(|_| OxideError::ParsePdf)?;
        merge_resource_dictionary(&mut resources, inherited);
    }
    if let Some(direct) = direct_resources {
        merge_resource_dictionary(&mut resources, direct);
    }
    let Some(xobjects) = optional_xobject_dict(&resources)? else {
        return Ok(None);
    };
    if !xobjects.has(name) {
        return Ok(None);
    }
    let id = xobjects
        .get(name)
        .and_then(Object::as_reference)
        .map_err(|_| OxideError::ParsePdf)?;
    Ok(Some(id))
}

fn optional_xobject_dict(resources: &Dictionary) -> Result<Option<&Dictionary>, OxideError> {
    if !resources.has(b"XObject") {
        return Ok(None);
    }
    resources
        .get(b"XObject")
        .and_then(Object::as_dict)
        .map(Some)
        .map_err(|_| OxideError::ParsePdf)
}

fn optional_xobject_dict_mut(
    resources: &mut Dictionary,
) -> Result<Option<&mut Dictionary>, OxideError> {
    if !resources.has(b"XObject") {
        return Ok(None);
    }
    resources
        .get_mut(b"XObject")
        .and_then(Object::as_dict_mut)
        .map(Some)
        .map_err(|_| OxideError::ParsePdf)
}

fn required_image_dimension(dict: &Dictionary, key: &[u8]) -> Result<i64, OxideError> {
    dict.get(key)
        .and_then(Object::as_i64)
        .map_err(|_| OxideError::ParsePdf)
}

fn imported_page_resources(
    source: &lopdf::Document,
    target: &mut lopdf::Document,
    page_id: lopdf::ObjectId,
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
    let mut imported = BTreeMap::new();
    remap_imported_references(&mut resource_object, source, target, &mut imported)?;
    resource_object
        .as_dict()
        .cloned()
        .map_err(|_| OxideError::ParsePdf)
}

