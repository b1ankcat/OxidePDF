
fn pdf_string(object: &Object) -> Result<String, OxideError> {
    object
        .as_str()
        .map(|value| String::from_utf8_lossy(value).into_owned())
        .map_err(|_| OxideError::ParsePdf)
}

/// Renders a field value for reporting. String values are decoded; name values
/// (used by checkboxes and radio buttons, e.g. `/Yes`, `/Off`) are rendered as
/// their name text. Other object kinds are reported as having no value rather
/// than failing the whole inspection.
fn display_value(object: &Object) -> Option<String> {
    match object {
        Object::String(bytes, _) => Some(String::from_utf8_lossy(bytes).into_owned()),
        Object::Name(name) => Some(String::from_utf8_lossy(name).into_owned()),
        _ => None,
    }
}

/// Sets `/NeedAppearances true` on the AcroForm dictionary so viewers
/// regenerate appearance streams for the values written during a fill.
fn set_need_appearances(document: &mut lopdf::Document) -> Result<(), OxideError> {
    let catalog = document.catalog().map_err(|_| OxideError::ParsePdf)?;
    // The AcroForm may be an indirect reference or an inline dictionary; both
    // are valid and need the flag. Resolve to whichever dictionary applies.
    let acroform = match catalog.get(b"AcroForm") {
        Ok(Object::Reference(id)) => {
            let id = *id;
            document.get_object_mut(id).and_then(Object::as_dict_mut)
        }
        Ok(Object::Dictionary(_)) => document
            .catalog_mut()
            .and_then(|catalog| catalog.get_mut(b"AcroForm"))
            .and_then(Object::as_dict_mut),
        // No AcroForm present: nothing to flag.
        _ => return Ok(()),
    };
    acroform
        .map_err(|_| OxideError::ParsePdf)?
        .set("NeedAppearances", Object::Boolean(true));
    Ok(())
}
