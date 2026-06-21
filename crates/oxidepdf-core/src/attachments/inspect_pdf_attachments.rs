use crate::{
    Artifact, BytesArtifact, OxideError, PdfArtifact, ResourceLimits, TextArtifact,
    default_inspect_limits, enforce_input_bytes, enforce_max_pages, enforce_output_bytes, load_pdf,
    load_pdf_with_limits, pdf_bytes, save_pdf,
};
use lopdf::{Dictionary, Object, Stream, dictionary};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct AttachmentInspectOptions {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AttachmentExtractOptions {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AttachmentEditOptions {
    pub action: AttachmentEditAction,
    pub name: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AttachmentEditAction {
    Add,
    Delete,
}

#[derive(Debug, Serialize)]
struct AttachmentReport {
    attachments: Vec<AttachmentEntryReport>,
}

#[derive(Debug, Serialize)]
struct AttachmentEntryReport {
    name: String,
    description: Option<String>,
    size: usize,
}

pub fn inspect_pdf_attachments(
    input: &[u8],
    _options: &AttachmentInspectOptions,
) -> Result<TextArtifact, OxideError> {
    inspect_pdf_attachments_with_limits(input, _options, &default_inspect_limits())
}

pub fn inspect_pdf_attachments_with_limits(
    input: &[u8],
    _options: &AttachmentInspectOptions,
    limits: &ResourceLimits,
) -> Result<TextArtifact, OxideError> {
    let document = load_pdf_with_limits(input, limits)?;
    inspect_attachments_on_document(&document, limits)
}

pub(crate) fn inspect_attachments_on_document(
    document: &lopdf::Document,
    limits: &ResourceLimits,
) -> Result<TextArtifact, OxideError> {
    let report = AttachmentReport {
        attachments: read_attachment_reports(document)?,
    };
    let text = serde_json::to_string_pretty(&report).map_err(|_| OxideError::Internal)?;
    enforce_output_bytes(text.len(), limits)?;
    Ok(TextArtifact {
        text,
        diagnostics: Vec::new(),
    })
}

pub fn edit_pdf_attachment_artifacts(
    inputs: &[Artifact],
    options: &AttachmentEditOptions,
    limits: &ResourceLimits,
) -> Result<PdfArtifact, OxideError> {
    match options.action {
        AttachmentEditAction::Add => {
            let [pdf, attachment] = inputs else {
                return Err(OxideError::InvalidInput {
                    reason: "attachment add requires PDF input and attachment bytes".to_owned(),
                });
            };
            let pdf = pdf_bytes(pdf)?;
            let attachment = raw_bytes(attachment);
            enforce_input_bytes(pdf.len(), limits)?;
            enforce_input_bytes(attachment.len(), limits)?;
            let mut document = load_pdf(pdf)?;
            enforce_max_pages(document.get_pages().len(), limits)?;
            add_attachment(&mut document, options, attachment)?;
            let bytes = save_pdf(document)?;
            enforce_output_bytes(bytes.len(), limits)?;
            Ok(PdfArtifact {
                bytes: crate::ArtifactBytes::from_vec(bytes)?,
            })
        }
        AttachmentEditAction::Delete => {
            let [pdf] = inputs else {
                return Err(OxideError::InvalidInput {
                    reason: "attachment delete requires exactly one PDF input".to_owned(),
                });
            };
            let pdf = pdf_bytes(pdf)?;
            enforce_input_bytes(pdf.len(), limits)?;
            let mut document = load_pdf(pdf)?;
            enforce_max_pages(document.get_pages().len(), limits)?;
            delete_attachment(&mut document, required_name(options)?)?;
            let bytes = save_pdf(document)?;
            enforce_output_bytes(bytes.len(), limits)?;
            Ok(PdfArtifact {
                bytes: crate::ArtifactBytes::from_vec(bytes)?,
            })
        }
    }
}

pub fn extract_pdf_attachment(
    input: &[u8],
    name: &str,
    limits: &ResourceLimits,
) -> Result<BytesArtifact, OxideError> {
    enforce_input_bytes(input.len(), limits)?;
    let document = load_pdf(input)?;
    let attachment = find_attachment_stream(&document, name, limits)?;
    enforce_output_bytes(attachment.len(), limits)?;
    Ok(BytesArtifact {
        bytes: crate::ArtifactBytes::from_vec(attachment)?,
    })
}

pub(crate) fn remove_embedded_files(document: &mut lopdf::Document) -> Result<(), OxideError> {
    let catalog = catalog_mut(document)?;
    if let Ok(names_object) = catalog.get_mut(b"Names") {
        let names = names_object
            .as_dict_mut()
            .map_err(|_| OxideError::ParsePdf)?;
        names.remove(b"EmbeddedFiles");
    }
    Ok(())
}

fn add_attachment(
    document: &mut lopdf::Document,
    options: &AttachmentEditOptions,
    bytes: &[u8],
) -> Result<(), OxideError> {
    let name = required_name(options)?.to_owned();
    let file_stream_id = document.add_object(Stream::new(
        dictionary! {
            "Type" => "EmbeddedFile",
            "Params" => Object::Dictionary(dictionary! {
                "Size" => bytes.len() as i64,
            }),
        },
        bytes.to_vec(),
    ));
    let mut file_spec = dictionary! {
        "Type" => "Filespec",
        "F" => Object::string_literal(name.as_str()),
        "UF" => Object::string_literal(name.as_str()),
        "EF" => Object::Dictionary(dictionary! {
            "F" => file_stream_id,
        }),
    };
    if let Some(description) = &options.description {
        file_spec.set("Desc", Object::string_literal(description.as_str()));
    }
    let file_spec_id = document.add_object(file_spec);
    let names_id = embedded_files_names_id(document)?;
    let names = document
        .get_object_mut(names_id)
        .and_then(Object::as_dict_mut)
        .map_err(|_| OxideError::ParsePdf)?;
    let mut entries = names
        .get(b"Names")
        .and_then(Object::as_array)
        .cloned()
        .unwrap_or_default();
    entries.retain(|object| !matches_pdf_string(object, &name));
    entries.push(Object::string_literal(name.as_str()));
    entries.push(Object::Reference(file_spec_id));
    names.set("Names", entries);
    Ok(())
}

fn delete_attachment(document: &mut lopdf::Document, name: &str) -> Result<(), OxideError> {
    let names_id = embedded_files_names_id(document)?;
    let names = document
        .get_object_mut(names_id)
        .and_then(Object::as_dict_mut)
        .map_err(|_| OxideError::ParsePdf)?;
    let entries = names
        .get(b"Names")
        .and_then(Object::as_array)
        .cloned()
        .unwrap_or_default();
    let mut kept = Vec::new();
    let mut removed = false;
    for pair in entries.chunks(2) {
        let (entry_name, file_spec) = attachment_name_pair(pair)?;
        if matches_pdf_string(entry_name, name) {
            removed = true;
        } else {
            kept.push(entry_name.clone());
            kept.push(file_spec.clone());
        }
    }
    if !removed {
        return Err(OxideError::InvalidInput {
            reason: format!("attachment '{name}' not found"),
        });
    }
    names.set("Names", kept);
    Ok(())
}

fn read_attachment_reports(
    document: &lopdf::Document,
) -> Result<Vec<AttachmentEntryReport>, OxideError> {
    let Some(entries) = embedded_file_entries(document)? else {
        return Ok(Vec::new());
    };
    let mut reports = Vec::new();
    for pair in entries.chunks(2) {
        let (name_object, file_spec_object) = attachment_name_pair(pair)?;
        let name = pdf_string(name_object)?;
        let file_spec = deref_dict(document, file_spec_object)?;
        let description = file_spec.get(b"Desc").ok().map(pdf_string).transpose()?;
        let size = attachment_reported_size(document, file_spec)?;
        reports.push(AttachmentEntryReport {
            name,
            description,
            size,
        });
    }
    reports.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(reports)
}
