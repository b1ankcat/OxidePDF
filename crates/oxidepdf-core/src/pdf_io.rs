use crate::{OxideError, ResourceLimits};

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
