
fn find_attachment_stream(
    document: &lopdf::Document,
    name: &str,
    limits: &ResourceLimits,
) -> Result<Vec<u8>, OxideError> {
    let Some(entries) = embedded_file_entries(document)? else {
        return Err(OxideError::InvalidInput {
            reason: format!("attachment '{name}' not found"),
        });
    };
    for pair in entries.chunks(2) {
        let (entry_name, file_spec_object) = attachment_name_pair(pair)?;
        if matches_pdf_string(entry_name, name) {
            let file_spec = deref_dict(document, file_spec_object)?;
            // Reject oversized attachments by their declared size before
            // decompressing, so a decompression bomb cannot allocate past the
            // output limit while being read.
            if let Some(declared) = attachment_declared_size(document, file_spec)? {
                enforce_output_bytes(declared, limits)?;
            }
            let bytes = attachment_bytes(document, file_spec)?;
            enforce_output_bytes(bytes.len(), limits)?;
            return Ok(bytes);
        }
    }
    Err(OxideError::InvalidInput {
        reason: format!("attachment '{name}' not found"),
    })
}

fn attachment_name_pair(pair: &[Object]) -> Result<(&Object, &Object), OxideError> {
    let [name, file_spec] = pair else {
        return Err(OxideError::ParsePdf);
    };
    Ok((name, file_spec))
}

/// Reports an attachment's size for inspection without decompressing the
/// stream. Falls back to the decompressed length only when no declared size is
/// available.
fn attachment_reported_size(
    document: &lopdf::Document,
    file_spec: &Dictionary,
) -> Result<usize, OxideError> {
    if let Some(size) = attachment_declared_size(document, file_spec)? {
        return Ok(size);
    }
    Ok(attachment_bytes(document, file_spec)?.len())
}

/// Returns the embedded file's declared size from `/Params /Size` when present,
/// without touching the (possibly compressed) stream body.
fn attachment_declared_size(
    document: &lopdf::Document,
    file_spec: &Dictionary,
) -> Result<Option<usize>, OxideError> {
    let stream = embedded_file_stream(document, file_spec)?;
    let Ok(params) = stream.dict.get(b"Params").and_then(Object::as_dict) else {
        return Ok(None);
    };
    match params.get(b"Size").and_then(Object::as_i64) {
        Ok(size) if size >= 0 => Ok(Some(size as usize)),
        _ => Ok(None),
    }
}

fn attachment_bytes(
    document: &lopdf::Document,
    file_spec: &Dictionary,
) -> Result<Vec<u8>, OxideError> {
    embedded_file_stream(document, file_spec)?
        .get_plain_content()
        .map_err(|_| OxideError::ParsePdf)
}

/// Resolves a Filespec's embedded file stream by following `/EF /F`.
fn embedded_file_stream<'a>(
    document: &'a lopdf::Document,
    file_spec: &Dictionary,
) -> Result<&'a Stream, OxideError> {
    let ef = file_spec
        .get(b"EF")
        .and_then(Object::as_dict)
        .map_err(|_| OxideError::ParsePdf)?;
    let stream_id = ef
        .get(b"F")
        .and_then(Object::as_reference)
        .map_err(|_| OxideError::ParsePdf)?;
    document
        .get_object(stream_id)
        .and_then(Object::as_stream)
        .map_err(|_| OxideError::ParsePdf)
}

fn embedded_file_entries(document: &lopdf::Document) -> Result<Option<Vec<Object>>, OxideError> {
    let catalog = document.catalog().map_err(|_| OxideError::ParsePdf)?;
    let Ok(names_object) = catalog.get(b"Names") else {
        return Ok(None);
    };
    let names = deref_dict(document, names_object)?;
    let Ok(embedded_files_object) = names.get(b"EmbeddedFiles") else {
        return Ok(None);
    };
    let embedded_files = deref_dict(document, embedded_files_object)?;
    let entries = embedded_files
        .get(b"Names")
        .and_then(Object::as_array)
        .cloned()
        .map_err(|_| OxideError::ParsePdf)?;
    Ok(Some(entries))
}

fn embedded_files_names_id(document: &mut lopdf::Document) -> Result<lopdf::ObjectId, OxideError> {
    let names_root_id = match catalog_mut(document)?
        .get(b"Names")
        .and_then(Object::as_reference)
    {
        Ok(id) => id,
        Err(_) => {
            let id = document.add_object(Dictionary::new());
            catalog_mut(document)?.set("Names", id);
            id
        }
    };
    let names_root = document
        .get_object_mut(names_root_id)
        .and_then(Object::as_dict_mut)
        .map_err(|_| OxideError::ParsePdf)?;
    match names_root
        .get(b"EmbeddedFiles")
        .and_then(Object::as_reference)
    {
        Ok(id) => Ok(id),
        Err(_) => {
            let id = document.add_object(dictionary! {
                "Names" => Object::Array(Vec::new()),
            });
            let names_root = document
                .get_object_mut(names_root_id)
                .and_then(Object::as_dict_mut)
                .map_err(|_| OxideError::ParsePdf)?;
            names_root.set("EmbeddedFiles", id);
            Ok(id)
        }
    }
}

fn raw_bytes(artifact: &Artifact) -> &[u8] {
    match artifact {
        Artifact::Bytes(bytes) => &bytes.bytes,
        Artifact::Pdf(pdf) => &pdf.bytes,
        Artifact::Image(image) => &image.bytes,
        Artifact::Svg(svg) => &svg.bytes,
        Artifact::Text(text) => text.text.as_bytes(),
        // A parsed object tree has no inline byte view; an attachment payload is
        // always raw file content, never an in-flight parsed document.
        Artifact::PdfObject(_) => &[],
    }
}

fn required_name(options: &AttachmentEditOptions) -> Result<&str, OxideError> {
    options
        .name
        .as_deref()
        .filter(|name| !name.is_empty())
        .ok_or_else(|| OxideError::InvalidInput {
            reason: "attachment operation requires non-empty name".to_owned(),
        })
}

fn catalog_mut(document: &mut lopdf::Document) -> Result<&mut Dictionary, OxideError> {
    document.catalog_mut().map_err(|_| OxideError::ParsePdf)
}

fn deref_dict<'a>(
    document: &'a lopdf::Document,
    object: &'a Object,
) -> Result<&'a Dictionary, OxideError> {
    match object {
        Object::Dictionary(dictionary) => Ok(dictionary),
        Object::Reference(id) => document
            .get_object(*id)
            .and_then(Object::as_dict)
            .map_err(|_| OxideError::ParsePdf),
        _ => Err(OxideError::ParsePdf),
    }
}

fn pdf_string(object: &Object) -> Result<String, OxideError> {
    object
        .as_str()
        .map(|value| String::from_utf8_lossy(value).into_owned())
        .map_err(|_| OxideError::ParsePdf)
}

fn matches_pdf_string(object: &Object, expected: &str) -> bool {
    object
        .as_str()
        .is_ok_and(|value| String::from_utf8_lossy(value) == expected)
}
