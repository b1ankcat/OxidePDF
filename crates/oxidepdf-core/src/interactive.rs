use crate::{
    attachments::remove_embedded_files, enforce_input_bytes, enforce_max_pages,
    enforce_output_bytes, load_pdf, save_pdf, OxideError, PdfArtifact, ResourceLimits,
};
use lopdf::{Dictionary, Object};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct InteractiveRemovalOptions {
    pub annotations: bool,
    pub forms: bool,
    pub actions: bool,
    pub javascript: bool,
    pub embedded_files: bool,
}

pub fn remove_pdf_interactive_elements(
    input: &[u8],
    options: &InteractiveRemovalOptions,
    limits: &ResourceLimits,
) -> Result<PdfArtifact, OxideError> {
    enforce_input_bytes(input.len(), limits)?;
    let mut document = load_pdf(input)?;
    enforce_max_pages(document.get_pages().len(), limits)?;
    if options.annotations {
        remove_annotations(&mut document)?;
    }
    if options.forms {
        remove_acroform(&mut document)?;
    }
    if options.actions {
        remove_actions(&mut document)?;
    }
    if options.javascript {
        remove_javascript(&mut document)?;
    }
    if options.embedded_files {
        remove_embedded_files(&mut document)?;
    }
    let bytes = save_pdf(document)?;
    enforce_output_bytes(bytes.len(), limits)?;
    Ok(PdfArtifact {
        bytes: bytes.into(),
    })
}

pub(crate) fn remove_annotations(document: &mut lopdf::Document) -> Result<(), OxideError> {
    for page_id in document.get_pages().into_values() {
        document
            .get_object_mut(page_id)
            .and_then(Object::as_dict_mut)
            .map_err(|_| OxideError::ParsePdf)?
            .remove(b"Annots");
    }
    Ok(())
}

pub(crate) fn remove_acroform(document: &mut lopdf::Document) -> Result<(), OxideError> {
    catalog_mut(document)?.remove(b"AcroForm");
    remove_annotations(document)
}

fn remove_actions(document: &mut lopdf::Document) -> Result<(), OxideError> {
    remove_key_from_dictionary_recursively(&mut document.trailer, b"OpenAction");
    remove_key_from_dictionary_recursively(&mut document.trailer, b"AA");
    for object in document.objects.values_mut() {
        remove_key_recursively(object, b"A");
        remove_key_recursively(object, b"AA");
        remove_key_recursively(object, b"OpenAction");
    }
    Ok(())
}

fn remove_javascript(document: &mut lopdf::Document) -> Result<(), OxideError> {
    let catalog = catalog_mut(document)?;
    if let Ok(names_object) = catalog.get_mut(b"Names") {
        let names = names_object
            .as_dict_mut()
            .map_err(|_| OxideError::ParsePdf)?;
        names.remove(b"JavaScript");
    }
    for object in document.objects.values_mut() {
        remove_javascript_actions(object);
    }
    Ok(())
}

fn remove_key_recursively(object: &mut Object, key: &[u8]) {
    match object {
        Object::Dictionary(dictionary) => {
            dictionary.remove(key);
            for (_, value) in dictionary.iter_mut() {
                remove_key_recursively(value, key);
            }
        }
        Object::Array(items) => {
            for item in items {
                remove_key_recursively(item, key);
            }
        }
        Object::Stream(stream) => {
            stream.dict.remove(key);
            for (_, value) in stream.dict.iter_mut() {
                remove_key_recursively(value, key);
            }
        }
        _ => {}
    }
}

fn remove_key_from_dictionary_recursively(dictionary: &mut Dictionary, key: &[u8]) {
    dictionary.remove(key);
    for (_, value) in dictionary.iter_mut() {
        remove_key_recursively(value, key);
    }
}

fn remove_javascript_actions(object: &mut Object) {
    match object {
        Object::Dictionary(dictionary) => {
            let is_javascript = dictionary
                .get(b"S")
                .and_then(Object::as_name)
                .is_ok_and(|name| name == b"JavaScript");
            if is_javascript {
                dictionary.remove(b"S");
                dictionary.remove(b"JS");
                dictionary.remove(b"JavaScript");
                return;
            }
            for (_, value) in dictionary.iter_mut() {
                remove_javascript_actions(value);
            }
        }
        Object::Array(items) => {
            for item in items {
                remove_javascript_actions(item);
            }
        }
        Object::Stream(stream) => {
            for (_, value) in stream.dict.iter_mut() {
                remove_javascript_actions(value);
            }
        }
        _ => {}
    }
}

fn catalog_mut(document: &mut lopdf::Document) -> Result<&mut Dictionary, OxideError> {
    document.catalog_mut().map_err(|_| OxideError::ParsePdf)
}
