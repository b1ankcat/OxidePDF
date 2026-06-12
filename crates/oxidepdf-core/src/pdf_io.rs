use crate::{Artifact, OxideError, ResourceLimits};
use lopdf::{Dictionary, Object};
use std::collections::BTreeMap;

pub(crate) fn load_pdf(input: &[u8]) -> Result<lopdf::Document, OxideError> {
    ensure_pdf_magic(input)?;
    let document = lopdf::Document::load_mem(input).map_err(map_lopdf_read_error)?;
    if document.is_encrypted() {
        return Err(OxideError::EncryptedPdf);
    }
    if document.get_pages().is_empty() {
        return Err(OxideError::InvalidInput {
            reason: "PDF contains no pages".to_owned(),
        });
    }

    Ok(document)
}

pub(crate) fn ensure_pdf_magic(input: &[u8]) -> Result<(), OxideError> {
    if input.starts_with(b"%PDF-") {
        return Ok(());
    }

    Err(OxideError::InvalidInput {
        reason: "expected PDF input magic bytes".to_owned(),
    })
}

pub(crate) fn pdf_bytes(artifact: &Artifact) -> Result<&[u8], OxideError> {
    match artifact {
        Artifact::Pdf(pdf) => Ok(&pdf.bytes),
        Artifact::Bytes(bytes) => Ok(&bytes.bytes),
        _ => Err(OxideError::InvalidInput {
            reason: "expected PDF input artifact".to_owned(),
        }),
    }
}

pub(crate) fn save_pdf(mut document: lopdf::Document) -> Result<Vec<u8>, OxideError> {
    let mut output = Vec::new();
    document.prune_objects();
    document.renumber_objects();
    document
        .save_to(&mut output)
        .map_err(|_| OxideError::WritePdf)?;
    Ok(output)
}

pub(crate) fn map_lopdf_read_error(error: lopdf::Error) -> OxideError {
    match error {
        lopdf::Error::Decryption(_) | lopdf::Error::UnsupportedSecurityHandler(_) => {
            OxideError::EncryptedPdf
        }
        _ => OxideError::ParsePdf,
    }
}

pub(crate) fn map_pdf_extract_error(error: pdf_extract::OutputError) -> OxideError {
    let message = error.to_string().to_ascii_lowercase();
    if message.contains("encrypted")
        || message.contains("decryption")
        || message.contains("incorrect password")
        || message.contains("security handler")
    {
        OxideError::EncryptedPdf
    } else {
        OxideError::ExtractText
    }
}

pub(crate) fn enforce_input_bytes(size: usize, limits: &ResourceLimits) -> Result<(), OxideError> {
    if let Some(limit) = limits.max_input_bytes {
        if size as u64 > limit {
            return Err(resource_limit("max_input_bytes"));
        }
    }

    Ok(())
}

pub(crate) fn enforce_max_pages(pages: usize, limits: &ResourceLimits) -> Result<(), OxideError> {
    if let Some(limit) = limits.max_pages {
        if pages as u32 > limit {
            return Err(resource_limit("max_pages"));
        }
    }

    Ok(())
}

pub(crate) fn enforce_max_pixels(pixels: u64, limits: &ResourceLimits) -> Result<(), OxideError> {
    if let Some(limit) = limits.max_pixels {
        if pixels > limit {
            return Err(resource_limit("max_pixels"));
        }
    }

    Ok(())
}

pub(crate) fn enforce_output_bytes(size: usize, limits: &ResourceLimits) -> Result<(), OxideError> {
    if let Some(limit) = limits.max_output_bytes {
        if size as u64 > limit {
            return Err(resource_limit("max_output_bytes"));
        }
    }

    Ok(())
}

pub(crate) fn resource_limit(limit: impl Into<String>) -> OxideError {
    OxideError::ResourceLimitExceeded {
        limit: limit.into(),
    }
}

pub(crate) fn rebuild_pages_tree(
    document: &mut lopdf::Document,
    page_ids: &[lopdf::ObjectId],
) -> Result<(), OxideError> {
    let catalog = document.catalog().map_err(|_| OxideError::ParsePdf)?;
    let pages_id = catalog
        .get(b"Pages")
        .and_then(lopdf::Object::as_reference)
        .map_err(|_| OxideError::ParsePdf)?;
    {
        let pages_dictionary = document
            .get_object_mut(pages_id)
            .and_then(lopdf::Object::as_dict_mut)
            .map_err(|_| OxideError::ParsePdf)?;
        pages_dictionary.set("Count", page_ids.len() as u32);
        pages_dictionary.set(
            "Kids",
            page_ids
                .iter()
                .copied()
                .map(lopdf::Object::Reference)
                .collect::<Vec<_>>(),
        );
    }
    for page_id in page_ids {
        let page_dictionary = document
            .get_object_mut(*page_id)
            .and_then(lopdf::Object::as_dict_mut)
            .map_err(|_| OxideError::ParsePdf)?;
        page_dictionary.set("Parent", pages_id);
    }

    Ok(())
}

pub(crate) fn merge_resource_dictionary(target: &mut Dictionary, source: &Dictionary) {
    for (key, value) in source.iter() {
        match (target.get_mut(key), value) {
            (Ok(Object::Dictionary(target_dict)), Object::Dictionary(source_dict)) => {
                merge_resource_dictionary(target_dict, source_dict);
            }
            _ => {
                target.set(key.clone(), value.clone());
            }
        }
    }
}

pub(crate) fn remap_imported_references(
    object: &mut Object,
    source: &lopdf::Document,
    target: &mut lopdf::Document,
    imported: &mut BTreeMap<lopdf::ObjectId, lopdf::ObjectId>,
) -> Result<(), OxideError> {
    match object {
        Object::Reference(source_id) => {
            let target_id = import_indirect_object(*source_id, source, target, imported)?;
            *source_id = target_id;
        }
        Object::Array(items) => {
            for item in items {
                remap_imported_references(item, source, target, imported)?;
            }
        }
        Object::Dictionary(dictionary) => {
            for (_, value) in dictionary.iter_mut() {
                remap_imported_references(value, source, target, imported)?;
            }
        }
        Object::Stream(stream) => {
            for (_, value) in stream.dict.iter_mut() {
                remap_imported_references(value, source, target, imported)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn import_indirect_object(
    source_id: lopdf::ObjectId,
    source: &lopdf::Document,
    target: &mut lopdf::Document,
    imported: &mut BTreeMap<lopdf::ObjectId, lopdf::ObjectId>,
) -> Result<lopdf::ObjectId, OxideError> {
    if let Some(target_id) = imported.get(&source_id) {
        return Ok(*target_id);
    }

    let target_id = target.new_object_id();
    imported.insert(source_id, target_id);
    let mut object = source
        .objects
        .get(&source_id)
        .cloned()
        .ok_or(OxideError::ParsePdf)?;
    remap_imported_references(&mut object, source, target, imported)?;
    target.set_object(target_id, object);
    Ok(target_id)
}

pub(crate) fn add_resource_dict_entry(
    document: &mut lopdf::Document,
    page_id: lopdf::ObjectId,
    dict_name: &[u8],
    resource_name: Vec<u8>,
    value: Object,
) -> Result<(), OxideError> {
    let resources = document
        .get_or_create_resources(page_id)
        .and_then(Object::as_dict_mut)
        .map_err(|_| OxideError::WritePdf)?;
    if !resources.has(dict_name) {
        resources.set(dict_name.to_vec(), Dictionary::new());
    }
    let dictionary = resources
        .get_mut(dict_name)
        .and_then(Object::as_dict_mut)
        .map_err(|_| OxideError::WritePdf)?;
    dictionary.set(resource_name, value);
    Ok(())
}

pub(crate) fn page_size(
    document: &lopdf::Document,
    page_id: lopdf::ObjectId,
) -> Result<(f32, f32), OxideError> {
    let page = document
        .get_object(page_id)
        .and_then(Object::as_dict)
        .map_err(|_| OxideError::ParsePdf)?;
    let media_box = page
        .get(b"MediaBox")
        .and_then(Object::as_array)
        .map_err(|_| OxideError::ParsePdf)?;
    if media_box.len() != 4 {
        return Err(OxideError::ParsePdf);
    }
    let width = object_to_f32(&media_box[2])? - object_to_f32(&media_box[0])?;
    let height = object_to_f32(&media_box[3])? - object_to_f32(&media_box[1])?;
    Ok((width, height))
}

pub(crate) fn object_to_f32(object: &Object) -> Result<f32, OxideError> {
    match object {
        Object::Integer(value) => Ok(*value as f32),
        Object::Real(value) => Ok(*value),
        _ => Err(OxideError::ParsePdf),
    }
}
