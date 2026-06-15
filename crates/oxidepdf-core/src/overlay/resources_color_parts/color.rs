#[derive(Clone, Copy)]
enum ColorEditPlan {
    Invert,
    Replace { from: [f32; 3], to: [f32; 3] },
    Contrast { factor: f32 },
}

fn color_edit_plan(options: &ColorEditOptions) -> Result<ColorEditPlan, OxideError> {
    match options.action {
        ColorEditAction::Contrast => {
            let factor = options.factor.unwrap_or(1.0);
            if !factor.is_finite() || factor <= 0.0 {
                return Err(OxideError::InvalidInput {
                    reason: "contrast factor must be greater than zero".to_owned(),
                });
            }
            Ok(ColorEditPlan::Contrast { factor })
        }
        ColorEditAction::Invert => Ok(ColorEditPlan::Invert),
        ColorEditAction::Replace => {
            let from = validated_rgb(options.from, "replace from")?;
            let to = validated_rgb(options.to, "replace to")?;
            Ok(ColorEditPlan::Replace { from, to })
        }
    }
}

fn validated_rgb(value: Option<[f32; 3]>, label: &str) -> Result<[f32; 3], OxideError> {
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
    Ok(rgb)
}

fn rewrite_page_colors(
    document: &mut lopdf::Document,
    page_id: lopdf::ObjectId,
    plan: ColorEditPlan,
) -> Result<(), OxideError> {
    let content = document
        .get_page_content(page_id)
        .map_err(|_| OxideError::ParsePdf)?;
    let mut content =
        lopdf::content::Content::decode(&content).map_err(|_| OxideError::ParsePdf)?;
    for operation in &mut content.operations {
        match operation.operator.as_str() {
            "rg" | "RG" => rewrite_rgb_operation(operation, plan)?,
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
    plan: ColorEditPlan,
) -> Result<(), OxideError> {
    let [red, green, blue] = operation.operands.as_slice() else {
        return Err(OxideError::ParsePdf);
    };
    let current = [
        object_to_f32(red)?,
        object_to_f32(green)?,
        object_to_f32(blue)?,
    ];
    let updated = match plan {
        ColorEditPlan::Invert => [1.0 - current[0], 1.0 - current[1], 1.0 - current[2]],
        ColorEditPlan::Replace { from, to } => {
            if rgb_matches(current, from) {
                to
            } else {
                current
            }
        }
        ColorEditPlan::Contrast { factor } => {
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
