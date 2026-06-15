fn validate_color_options(options: &ColorEditOptions) -> Result<(), OxideError> {
    match options.action {
        ColorEditAction::Contrast => {
            let factor = options.factor.unwrap_or(1.0);
            if !factor.is_finite() || factor <= 0.0 {
                return Err(OxideError::InvalidInput {
                    reason: "contrast factor must be greater than zero".to_owned(),
                });
            }
        }
        ColorEditAction::Invert => {}
        ColorEditAction::Replace => {
            validate_rgb(options.from, "replace from")?;
            validate_rgb(options.to, "replace to")?;
        }
    }
    Ok(())
}

fn validate_rgb(value: Option<[f32; 3]>, label: &str) -> Result<(), OxideError> {
    let rgb = value.ok_or_else(|| OxideError::InvalidInput {
        reason: format!("color {label} must be provided"),
    })?;
    if rgb
        .iter()
        .any(|component| !component.is_finite() || !(0.0..=1.0).contains(component))
    {
        return Err(OxideError::InvalidInput {
            reason: format!("color {label} components must be between 0.0 and 1.0"),
        });
    }
    Ok(())
}

fn rewrite_page_colors(
    document: &mut lopdf::Document,
    page_id: lopdf::ObjectId,
    options: &ColorEditOptions,
) -> Result<(), OxideError> {
    let content = document
        .get_page_content(page_id)
        .map_err(|_| OxideError::ParsePdf)?;
    let mut content =
        lopdf::content::Content::decode(&content).map_err(|_| OxideError::ParsePdf)?;
    for operation in &mut content.operations {
        match operation.operator.as_str() {
            "rg" | "RG" => rewrite_rgb_operation(operation, options)?,
            "g" | "G" | "k" | "K" | "cs" | "CS" | "sc" | "SC" | "scn" | "SCN" | "sh" => {
                return Err(OxideError::UnsupportedPdfFeature {
                    feature: format!(
                        "color operation '{}' is not supported by vector color editing",
                        operation.operator
                    ),
                });
            }
            _ => {}
        }
    }
    let bytes = content.encode().map_err(|_| OxideError::WritePdf)?;
    replace_page_content(document, page_id, bytes)
}

fn rewrite_rgb_operation(
    operation: &mut lopdf::content::Operation,
    options: &ColorEditOptions,
) -> Result<(), OxideError> {
    if operation.operands.len() != 3 {
        return Err(OxideError::ParsePdf);
    }
    let current = [
        object_to_f32(&operation.operands[0])?,
        object_to_f32(&operation.operands[1])?,
        object_to_f32(&operation.operands[2])?,
    ];
    let updated = match options.action {
        ColorEditAction::Invert => [1.0 - current[0], 1.0 - current[1], 1.0 - current[2]],
        ColorEditAction::Replace => {
            let from = options.from.unwrap();
            let to = options.to.unwrap();
            if rgb_matches(current, from) {
                to
            } else {
                current
            }
        }
        ColorEditAction::Contrast => {
            let factor = options.factor.unwrap_or(1.0);
            current.map(|component| ((component - 0.5) * factor + 0.5).clamp(0.0, 1.0))
        }
    };
    operation.operands = updated.into_iter().map(Object::Real).collect();
    Ok(())
}

fn rgb_matches(left: [f32; 3], right: [f32; 3]) -> bool {
    left.iter()
        .zip(right.iter())
        .all(|(left, right)| (left - right).abs() <= 0.0001)
}

fn replace_page_content(
    document: &mut lopdf::Document,
    page_id: lopdf::ObjectId,
    content: Vec<u8>,
) -> Result<(), OxideError> {
    let content_id = document.add_object(Stream::new(Dictionary::new(), content));
    let page = document
        .get_object_mut(page_id)
        .and_then(Object::as_dict_mut)
        .map_err(|_| OxideError::ParsePdf)?;
    page.set("Contents", content_id);
    Ok(())
}

