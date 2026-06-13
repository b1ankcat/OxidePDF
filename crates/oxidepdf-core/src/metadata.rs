use crate::{
    enforce_input_bytes, enforce_max_pages, enforce_output_bytes, load_pdf, save_pdf, OxideError,
    PdfArtifact, ResourceLimits, TextArtifact,
};
use lopdf::{Dictionary, Object};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MetadataInspectOptions {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MetadataEditOptions {
    pub action: MetadataEditAction,
    #[serde(default)]
    pub entries: Vec<MetadataEntry>,
    #[serde(default)]
    pub keys: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetadataEditAction {
    Set,
    Delete,
    Validate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MetadataEntry {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Serialize)]
struct MetadataReport {
    valid: bool,
    entries: BTreeMap<String, String>,
}

pub fn inspect_pdf_metadata(
    input: &[u8],
    _options: &MetadataInspectOptions,
) -> Result<TextArtifact, OxideError> {
    let document = load_pdf(input)?;
    let report = MetadataReport {
        valid: true,
        entries: read_metadata_entries(&document)?,
    };
    let text = serde_json::to_string_pretty(&report).map_err(|_| OxideError::Internal)?;
    Ok(TextArtifact {
        text,
        diagnostics: Vec::new(),
    })
}

pub fn edit_pdf_metadata(
    input: &[u8],
    options: &MetadataEditOptions,
    limits: &ResourceLimits,
) -> Result<PdfArtifact, OxideError> {
    enforce_input_bytes(input.len(), limits)?;
    let mut document = load_pdf(input)?;
    edit_metadata_on_document(&mut document, options, limits)?;
    let bytes = save_pdf(document)?;
    enforce_output_bytes(bytes.len(), limits)?;
    Ok(PdfArtifact {
        bytes: bytes.into(),
    })
}

/// Applies a metadata edit to an already-parsed document.
pub(crate) fn edit_metadata_on_document(
    document: &mut lopdf::Document,
    options: &MetadataEditOptions,
    limits: &ResourceLimits,
) -> Result<(), OxideError> {
    enforce_max_pages(document.get_pages().len(), limits)?;
    match options.action {
        MetadataEditAction::Set => {
            let mut entries = read_metadata_entries(document)?;
            for entry in &options.entries {
                validate_metadata_key(&entry.key)?;
                entries.insert(normalized_metadata_key(&entry.key), entry.value.clone());
            }
            write_metadata_entries(document, &entries);
        }
        MetadataEditAction::Delete => {
            let mut entries = read_metadata_entries(document)?;
            for key in &options.keys {
                validate_metadata_key(key)?;
                entries.remove(&normalized_metadata_key(key));
            }
            write_metadata_entries(document, &entries);
        }
        MetadataEditAction::Validate => {
            read_metadata_entries(document)?;
        }
    }
    Ok(())
}

fn read_metadata_entries(
    document: &lopdf::Document,
) -> Result<BTreeMap<String, String>, OxideError> {
    let mut entries = BTreeMap::new();
    if let Ok(info_id) = document.trailer.get(b"Info").and_then(Object::as_reference) {
        let info = document
            .get_object(info_id)
            .and_then(Object::as_dict)
            .map_err(|_| OxideError::ParsePdf)?;
        for (key, value) in info.iter() {
            let Some(rendered) = metadata_value(value) else {
                continue;
            };
            entries.insert(
                normalized_metadata_key(&String::from_utf8_lossy(key)),
                rendered,
            );
        }
    }
    Ok(entries)
}

fn write_metadata_entries(document: &mut lopdf::Document, entries: &BTreeMap<String, String>) {
    if entries.is_empty() {
        document.trailer.remove(b"Info");
        return;
    }
    let mut info = Dictionary::new();
    for (key, value) in entries {
        info.set(
            metadata_pdf_key(key),
            Object::string_literal(value.as_str()),
        );
    }
    let info_id = document.add_object(info);
    document.trailer.set("Info", info_id);
}

fn validate_metadata_key(key: &str) -> Result<(), OxideError> {
    let normalized = normalized_metadata_key(key);
    if normalized.is_empty() || !normalized.bytes().all(|byte| byte.is_ascii_alphanumeric()) {
        return Err(OxideError::InvalidInput {
            reason: format!("invalid metadata key '{key}'"),
        });
    }
    Ok(())
}

fn normalized_metadata_key(key: &str) -> String {
    let mut chars = key.trim_start_matches('/').chars();
    match chars.next() {
        Some(first) => first.to_ascii_lowercase().to_string() + chars.as_str(),
        None => String::new(),
    }
}

fn metadata_pdf_key(key: &str) -> Vec<u8> {
    let mut chars = key.chars();
    match chars.next() {
        Some(first) => (first.to_ascii_uppercase().to_string() + chars.as_str()).into_bytes(),
        None => Vec::new(),
    }
}

fn pdf_string(object: &Object) -> Result<String, OxideError> {
    object
        .as_str()
        .map(|value| String::from_utf8_lossy(value).into_owned())
        .map_err(|_| OxideError::ParsePdf)
}

/// Renders an Info-dictionary value for reporting. Text strings are decoded and
/// names (e.g. `/Trapped` → `True`/`False`/`Unknown`) are rendered as their
/// name text. Other object kinds are skipped rather than failing the whole
/// document.
fn metadata_value(object: &Object) -> Option<String> {
    match object {
        Object::String(..) => pdf_string(object).ok(),
        Object::Name(name) => Some(String::from_utf8_lossy(name).into_owned()),
        _ => None,
    }
}
