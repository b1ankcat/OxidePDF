use crate::{
    enforce_input_bytes, enforce_max_pages, enforce_output_bytes, load_pdf, save_pdf, OxideError,
    PdfArtifact, ResourceLimits, TextArtifact,
};
use lopdf::{dictionary, Object};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AnnotationInspectOptions {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnnotationEditOptions {
    pub action: AnnotationEditAction,
    pub page: Option<u32>,
    pub id: Option<String>,
    pub text: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnnotationEditAction {
    AddText,
    Delete,
}

#[derive(Debug, Serialize)]
struct AnnotationReport {
    annotations: Vec<AnnotationEntryReport>,
}

#[derive(Debug, Serialize)]
struct AnnotationEntryReport {
    page: u32,
    id: Option<String>,
    subtype: String,
    text: Option<String>,
}

pub fn inspect_pdf_annotations(
    input: &[u8],
    _options: &AnnotationInspectOptions,
) -> Result<TextArtifact, OxideError> {
    let document = load_pdf(input)?;
    let mut annotations = Vec::new();
    for (page_number, page_id) in document.get_pages() {
        let page = document
            .get_object(page_id)
            .and_then(Object::as_dict)
            .map_err(|_| OxideError::ParsePdf)?;
        let Some(annots) = annotation_array(page)? else {
            continue;
        };
        for annot in annots {
            let dictionary = match annot {
                Object::Dictionary(dictionary) => dictionary,
                Object::Reference(id) => document
                    .get_object(*id)
                    .and_then(Object::as_dict)
                    .map_err(|_| OxideError::ParsePdf)?,
                _ => return Err(OxideError::ParsePdf),
            };
            let subtype = dictionary
                .get(b"Subtype")
                .and_then(Object::as_name)
                .map(|name| String::from_utf8_lossy(name).into_owned())
                .unwrap_or_else(|_| "Unknown".to_owned());
            let id = dictionary.get(b"NM").ok().map(pdf_string).transpose()?;
            let text = dictionary
                .get(b"Contents")
                .ok()
                .map(pdf_string)
                .transpose()?;
            annotations.push(AnnotationEntryReport {
                page: page_number,
                id,
                subtype,
                text,
            });
        }
    }
    annotations.sort_by(|left, right| {
        left.page
            .cmp(&right.page)
            .then_with(|| left.id.cmp(&right.id))
    });
    let text = serde_json::to_string_pretty(&AnnotationReport { annotations })
        .map_err(|_| OxideError::Internal)?;
    Ok(TextArtifact {
        text,
        diagnostics: Vec::new(),
    })
}

pub fn edit_pdf_annotations(
    input: &[u8],
    options: &AnnotationEditOptions,
    limits: &ResourceLimits,
) -> Result<PdfArtifact, OxideError> {
    enforce_input_bytes(input.len(), limits)?;
    let mut document = load_pdf(input)?;
    enforce_max_pages(document.get_pages().len(), limits)?;
    match options.action {
        AnnotationEditAction::AddText => add_text_annotation(&mut document, options)?,
        AnnotationEditAction::Delete => delete_annotation(&mut document, options)?,
    }
    let bytes = save_pdf(document)?;
    enforce_output_bytes(bytes.len(), limits)?;
    Ok(PdfArtifact { bytes })
}

fn add_text_annotation(
    document: &mut lopdf::Document,
    options: &AnnotationEditOptions,
) -> Result<(), OxideError> {
    let page_number = required_page(options)?;
    let page_id = page_id_for_number(document, page_number)?;
    let id = options
        .id
        .as_deref()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| OxideError::InvalidInput {
            reason: "annotation add_text requires non-empty id".to_owned(),
        })?;
    let text = options
        .text
        .as_deref()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| OxideError::InvalidInput {
            reason: "annotation add_text requires non-empty text".to_owned(),
        })?;
    let annotation_id = document.add_object(dictionary! {
        "Type" => "Annot",
        "Subtype" => "Text",
        "Rect" => Object::Array(vec![36.into(), 36.into(), 54.into(), 54.into()]),
        "NM" => Object::string_literal(id),
        "Contents" => Object::string_literal(text),
        "P" => page_id,
    });
    let page = document
        .get_object_mut(page_id)
        .and_then(Object::as_dict_mut)
        .map_err(|_| OxideError::ParsePdf)?;
    let mut annots = annotation_array(page)?.cloned().unwrap_or_default();
    annots.push(Object::Reference(annotation_id));
    page.set("Annots", annots);
    Ok(())
}

fn delete_annotation(
    document: &mut lopdf::Document,
    options: &AnnotationEditOptions,
) -> Result<(), OxideError> {
    let id = options
        .id
        .as_deref()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| OxideError::InvalidInput {
            reason: "annotation delete requires non-empty id".to_owned(),
        })?;
    let mut removed = false;
    for page_id in document.get_pages().into_values() {
        let annots = {
            let page = document
                .get_object(page_id)
                .and_then(Object::as_dict)
                .map_err(|_| OxideError::ParsePdf)?;
            let Some(annots) = annotation_array(page)? else {
                continue;
            };
            annots.clone()
        };
        let mut kept = Vec::new();
        for annot in &annots {
            let matches = match annot {
                Object::Dictionary(dictionary) => annotation_id_matches(dictionary, id),
                Object::Reference(annot_id) => document
                    .get_object(*annot_id)
                    .and_then(Object::as_dict)
                    .map(|dictionary| annotation_id_matches(dictionary, id))
                    .map_err(|_| OxideError::ParsePdf)?,
                _ => return Err(OxideError::ParsePdf),
            };
            if matches {
                removed = true;
            } else {
                kept.push(annot.clone());
            }
        }
        let page = document
            .get_object_mut(page_id)
            .and_then(Object::as_dict_mut)
            .map_err(|_| OxideError::ParsePdf)?;
        if annots.is_empty() {
            continue;
        }
        page.set("Annots", kept);
    }
    if !removed {
        return Err(OxideError::InvalidInput {
            reason: format!("annotation '{id}' not found"),
        });
    }
    Ok(())
}

fn annotation_id_matches(dictionary: &lopdf::Dictionary, id: &str) -> bool {
    dictionary
        .get(b"NM")
        .and_then(Object::as_str)
        .is_ok_and(|value| String::from_utf8_lossy(value) == id)
}

fn annotation_array(dictionary: &lopdf::Dictionary) -> Result<Option<&Vec<Object>>, OxideError> {
    match dictionary.get(b"Annots") {
        Ok(object) => object
            .as_array()
            .map(Some)
            .map_err(|_| OxideError::ParsePdf),
        Err(_) => Ok(None),
    }
}

fn required_page(options: &AnnotationEditOptions) -> Result<u32, OxideError> {
    options.page.ok_or_else(|| OxideError::InvalidInput {
        reason: "annotation operation requires page".to_owned(),
    })
}

fn page_id_for_number(
    document: &lopdf::Document,
    page: u32,
) -> Result<lopdf::ObjectId, OxideError> {
    document
        .get_pages()
        .get(&page)
        .copied()
        .ok_or_else(|| OxideError::InvalidInput {
            reason: format!("page {page} is out of range"),
        })
}

fn pdf_string(object: &Object) -> Result<String, OxideError> {
    object
        .as_str()
        .map(|value| String::from_utf8_lossy(value).into_owned())
        .map_err(|_| OxideError::ParsePdf)
}
