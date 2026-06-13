use crate::{
    enforce_input_bytes, enforce_max_pages, enforce_output_bytes, load_pdf, pdf_bytes, save_pdf,
    Artifact, BytesArtifact, OxideError, PdfArtifact, ResourceLimits, TextArtifact,
};
use lopdf::{dictionary, Dictionary, Object, Stream};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AttachmentInspectOptions {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttachmentExtractOptions {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttachmentEditOptions {
    pub action: AttachmentEditAction,
    pub name: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
    let document = load_pdf(input)?;
    let report = AttachmentReport {
        attachments: read_attachment_reports(&document)?,
    };
    let text = serde_json::to_string_pretty(&report).map_err(|_| OxideError::Internal)?;
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
            if inputs.len() != 2 {
                return Err(OxideError::InvalidInput {
                    reason: "attachment add requires PDF input and attachment bytes".to_owned(),
                });
            }
            let pdf = pdf_bytes(&inputs[0])?;
            let attachment = raw_bytes(&inputs[1]);
            enforce_input_bytes(pdf.len(), limits)?;
            enforce_input_bytes(attachment.len(), limits)?;
            let mut document = load_pdf(pdf)?;
            enforce_max_pages(document.get_pages().len(), limits)?;
            add_attachment(&mut document, options, attachment)?;
            let bytes = save_pdf(document)?;
            enforce_output_bytes(bytes.len(), limits)?;
            Ok(PdfArtifact {
                bytes: bytes.into(),
            })
        }
        AttachmentEditAction::Delete => {
            if inputs.len() != 1 {
                return Err(OxideError::InvalidInput {
                    reason: "attachment delete requires exactly one PDF input".to_owned(),
                });
            }
            let pdf = pdf_bytes(&inputs[0])?;
            enforce_input_bytes(pdf.len(), limits)?;
            let mut document = load_pdf(pdf)?;
            enforce_max_pages(document.get_pages().len(), limits)?;
            delete_attachment(&mut document, required_name(options)?)?;
            let bytes = save_pdf(document)?;
            enforce_output_bytes(bytes.len(), limits)?;
            Ok(PdfArtifact {
                bytes: bytes.into(),
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
    let attachment = find_attachment_stream(&document, name)?;
    enforce_output_bytes(attachment.len(), limits)?;
    Ok(BytesArtifact {
        bytes: attachment.to_vec().into(),
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
        if pair.len() != 2 {
            return Err(OxideError::ParsePdf);
        }
        if matches_pdf_string(&pair[0], name) {
            removed = true;
        } else {
            kept.extend_from_slice(pair);
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
        if pair.len() != 2 {
            return Err(OxideError::ParsePdf);
        }
        let name = pdf_string(&pair[0])?;
        let file_spec = deref_dict(document, &pair[1])?;
        let description = file_spec.get(b"Desc").ok().map(pdf_string).transpose()?;
        let data = attachment_bytes(document, file_spec)?;
        reports.push(AttachmentEntryReport {
            name,
            description,
            size: data.len(),
        });
    }
    reports.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(reports)
}

fn find_attachment_stream(document: &lopdf::Document, name: &str) -> Result<Vec<u8>, OxideError> {
    let Some(entries) = embedded_file_entries(document)? else {
        return Err(OxideError::InvalidInput {
            reason: format!("attachment '{name}' not found"),
        });
    };
    for pair in entries.chunks(2) {
        if pair.len() != 2 {
            return Err(OxideError::ParsePdf);
        }
        if matches_pdf_string(&pair[0], name) {
            let file_spec = deref_dict(document, &pair[1])?;
            return attachment_bytes(document, file_spec);
        }
    }
    Err(OxideError::InvalidInput {
        reason: format!("attachment '{name}' not found"),
    })
}

fn attachment_bytes(
    document: &lopdf::Document,
    file_spec: &Dictionary,
) -> Result<Vec<u8>, OxideError> {
    let ef = file_spec
        .get(b"EF")
        .and_then(Object::as_dict)
        .map_err(|_| OxideError::ParsePdf)?;
    let stream_id = ef
        .get(b"F")
        .and_then(Object::as_reference)
        .map_err(|_| OxideError::ParsePdf)?;
    let stream = document
        .get_object(stream_id)
        .and_then(Object::as_stream)
        .map_err(|_| OxideError::ParsePdf)?;
    stream.get_plain_content().map_err(|_| OxideError::ParsePdf)
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
