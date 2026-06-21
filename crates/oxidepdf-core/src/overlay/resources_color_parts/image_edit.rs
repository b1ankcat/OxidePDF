fn append_xobject_watermark(
    document: &mut lopdf::Document,
    pages: &[u32],
    xobject_id: lopdf::ObjectId,
    natural_width: f32,
    natural_height: f32,
    resource_name: Vec<u8>,
    settings: WatermarkSettings,
) -> Result<(), OxideError> {
    let gs_id = graphics_state(document, settings.opacity);
    if natural_width <= 0.0 || natural_height <= 0.0 {
        return Err(OxideError::InvalidInput {
            reason: "watermark image dimensions must be greater than zero".to_owned(),
        });
    }
    let page_map = document.get_pages();
    for page_number in pages {
        let page_id = *page_map
            .get(page_number)
            .ok_or_else(|| OxideError::InvalidInput {
                reason: format!("page {page_number} is out of range"),
            })?;
        add_resource_dict_entry(
            document,
            page_id,
            b"XObject",
            resource_name.clone(),
            Object::Reference(xobject_id),
        )?;
        add_resource_dict_entry(
            document,
            page_id,
            b"ExtGState",
            b"OxWmGS".to_vec(),
            Object::Reference(gs_id),
        )?;
        let (page_width, page_height) = page_size(document, page_id)?;
        let scale = (page_width / natural_width)
            .min(page_height / natural_height)
            .min(1.0)
            * settings.scale;
        let width = natural_width * scale;
        let height = natural_height * scale;
        let (x, y) = watermark_origin(settings.position, page_width, page_height, width, height);
        let content = xobject_watermark_content(&resource_name, settings, x, y, width, height)?;
        document
            .add_page_contents(page_id, content)
            .map_err(|_| OxideError::WritePdf)?;
    }

    Ok(())
}

fn add_image_to_page(
    document: &mut lopdf::Document,
    page: u32,
    name: &str,
    image: &DecodedImage,
) -> Result<(), OxideError> {
    let page_id = *document
        .get_pages()
        .get(&page)
        .ok_or_else(|| OxideError::InvalidInput {
            reason: format!("page {page} is out of range"),
        })?;
    let image_id = document.add_object(image_xobject(image));
    if image.width == 0 || image.height == 0 {
        return Err(OxideError::InvalidInput {
            reason: "image dimensions must be greater than zero".to_owned(),
        });
    }
    let safe_name = sanitize_pdf_name(name);
    add_resource_dict_entry(
        document,
        page_id,
        b"XObject",
        safe_name.clone(),
        Object::Reference(image_id),
    )?;
    let (page_width, page_height) = page_size(document, page_id)?;
    let scale = (page_width / image.width as f32)
        .min(page_height / image.height as f32)
        .min(1.0);
    let width = image.width as f32 * scale;
    let height = image.height as f32 * scale;
    let content = xobject_watermark_content(
        &safe_name,
        WatermarkSettings {
            opacity: 1.0,
            rotation_degrees: 0.0,
            position: WatermarkPosition::Center,
            scale: 1.0,
            font_size: 1.0,
        },
        (page_width - width) / 2.0,
        (page_height - height) / 2.0,
        width,
        height,
    )?;
    document
        .add_page_contents(page_id, content)
        .map_err(|_| OxideError::WritePdf)
}

fn replace_image_resource(
    document: &mut lopdf::Document,
    name: &str,
    image: &DecodedImage,
) -> Result<(), OxideError> {
    let safe_name = sanitize_pdf_name(name);
    let mut replaced = false;
    for (_, page_id) in document.get_pages() {
        let Some(id) = page_xobject_reference(document, page_id, &safe_name)? else {
            continue;
        };
        let stream = document
            .get_object_mut(id)
            .and_then(Object::as_stream_mut)
            .map_err(|_| OxideError::ParsePdf)?;
        *stream = image_xobject(image);
        replaced = true;
    }
    if !replaced {
        return Err(OxideError::InvalidInput {
            reason: format!("image '{name}' not found"),
        });
    }
    Ok(())
}

fn delete_image_resource(document: &mut lopdf::Document, name: &str) -> Result<(), OxideError> {
    let safe_name = sanitize_pdf_name(name);
    let mut removed = false;
    for page_id in document.get_pages().into_values() {
        let resources = document
            .get_or_create_resources(page_id)
            .and_then(Object::as_dict_mut)
            .map_err(|_| OxideError::ParsePdf)?;
        if let Some(xobjects) = optional_xobject_dict_mut(resources)? {
            removed |= xobjects.remove(&safe_name).is_some();
        }
    }
    if !removed {
        return Err(OxideError::InvalidInput {
            reason: format!("image '{name}' not found"),
        });
    }
    Ok(())
}

fn required_image_name<'a>(name: Option<&'a str>, label: &str) -> Result<&'a str, OxideError> {
    name.filter(|value| !value.is_empty())
        .ok_or_else(|| OxideError::InvalidInput {
            reason: format!("{label} requires non-empty image name"),
        })
}

