use crate::{
    enforce_input_bytes, enforce_max_pages, enforce_output_bytes, interactive::remove_acroform,
    load_pdf, save_pdf, OxideError, PdfArtifact, ResourceLimits, TextArtifact,
};
use lopdf::Object;
use serde::{Deserialize, Serialize};

const READ_ONLY_FLAG: i64 = 1;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FormInspectOptions {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FormFillOptions {
    pub fields: Vec<FormFieldValue>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FormFieldValue {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Serialize)]
struct FormReport {
    fields: Vec<FormFieldReport>,
}

#[derive(Debug, Serialize)]
struct FormFieldReport {
    name: String,
    value: Option<String>,
    readonly: bool,
}

pub fn inspect_pdf_forms(
    input: &[u8],
    _options: &FormInspectOptions,
) -> Result<TextArtifact, OxideError> {
    let document = load_pdf(input)?;
    let report = FormReport {
        fields: collect_form_fields(&document)?,
    };
    let text = serde_json::to_string_pretty(&report).map_err(|_| OxideError::Internal)?;
    Ok(TextArtifact {
        text,
        diagnostics: Vec::new(),
    })
}

pub fn fill_pdf_form(
    input: &[u8],
    options: &FormFillOptions,
    limits: &ResourceLimits,
) -> Result<PdfArtifact, OxideError> {
    enforce_input_bytes(input.len(), limits)?;
    let mut document = load_pdf(input)?;
    enforce_max_pages(document.get_pages().len(), limits)?;
    reject_xfa(&document)?;
    for field in &options.fields {
        if field.name.is_empty() {
            return Err(OxideError::InvalidInput {
                reason: "form field name must not be empty".to_owned(),
            });
        }
        fill_field(&mut document, field)?;
    }
    let bytes = save_pdf(document)?;
    enforce_output_bytes(bytes.len(), limits)?;
    Ok(PdfArtifact {
        bytes: bytes.into(),
    })
}

pub fn unlock_pdf_form_readonly(
    input: &[u8],
    limits: &ResourceLimits,
) -> Result<PdfArtifact, OxideError> {
    enforce_input_bytes(input.len(), limits)?;
    let mut document = load_pdf(input)?;
    enforce_max_pages(document.get_pages().len(), limits)?;
    reject_xfa(&document)?;
    let field_ids = acroform_field_ids(&document)?;
    for field_id in field_ids {
        unlock_field_readonly(&mut document, field_id)?;
    }
    let bytes = save_pdf(document)?;
    enforce_output_bytes(bytes.len(), limits)?;
    Ok(PdfArtifact {
        bytes: bytes.into(),
    })
}

pub fn remove_pdf_forms(input: &[u8], limits: &ResourceLimits) -> Result<PdfArtifact, OxideError> {
    enforce_input_bytes(input.len(), limits)?;
    let mut document = load_pdf(input)?;
    enforce_max_pages(document.get_pages().len(), limits)?;
    remove_acroform(&mut document)?;
    let bytes = save_pdf(document)?;
    enforce_output_bytes(bytes.len(), limits)?;
    Ok(PdfArtifact {
        bytes: bytes.into(),
    })
}

fn collect_form_fields(document: &lopdf::Document) -> Result<Vec<FormFieldReport>, OxideError> {
    let mut fields = Vec::new();
    for field_id in acroform_field_ids(document)? {
        let field = document
            .get_object(field_id)
            .and_then(Object::as_dict)
            .map_err(|_| OxideError::ParsePdf)?;
        let Some(name) = field.get(b"T").ok().map(pdf_string).transpose()? else {
            continue;
        };
        let value = field.get(b"V").ok().map(pdf_string).transpose()?;
        let flags = field.get(b"Ff").and_then(Object::as_i64).unwrap_or(0);
        fields.push(FormFieldReport {
            name,
            value,
            readonly: flags & READ_ONLY_FLAG != 0,
        });
    }
    fields.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(fields)
}

fn fill_field(document: &mut lopdf::Document, value: &FormFieldValue) -> Result<(), OxideError> {
    let field_id = find_field_id(document, &value.name)?;
    let field = document
        .get_object_mut(field_id)
        .and_then(Object::as_dict_mut)
        .map_err(|_| OxideError::ParsePdf)?;
    let field_type = field.get(b"FT").and_then(Object::as_name).map_err(|_| {
        OxideError::UnsupportedPdfFeature {
            feature: format!("form field '{}' without supported field type", value.name),
        }
    })?;
    if field_type != b"Tx" {
        return Err(OxideError::UnsupportedPdfFeature {
            feature: format!("form field '{}' type is not text", value.name),
        });
    }
    field.set("V", Object::string_literal(value.value.as_str()));
    field.remove(b"AP");
    Ok(())
}

fn unlock_field_readonly(
    document: &mut lopdf::Document,
    field_id: lopdf::ObjectId,
) -> Result<(), OxideError> {
    let field = document
        .get_object_mut(field_id)
        .and_then(Object::as_dict_mut)
        .map_err(|_| OxideError::ParsePdf)?;
    let flags = field.get(b"Ff").and_then(Object::as_i64).unwrap_or(0);
    field.set("Ff", flags & !READ_ONLY_FLAG);
    Ok(())
}

fn reject_xfa(document: &lopdf::Document) -> Result<(), OxideError> {
    let Some(acroform) = acroform(document)? else {
        return Ok(());
    };
    if acroform.has(b"XFA") {
        return Err(OxideError::UnsupportedPdfFeature {
            feature: "XFA forms are not supported".to_owned(),
        });
    }
    Ok(())
}

fn find_field_id(document: &lopdf::Document, name: &str) -> Result<lopdf::ObjectId, OxideError> {
    for field_id in acroform_field_ids(document)? {
        let field = document
            .get_object(field_id)
            .and_then(Object::as_dict)
            .map_err(|_| OxideError::ParsePdf)?;
        if field
            .get(b"T")
            .is_ok_and(|object| pdf_string(object).is_ok_and(|field_name| field_name == name))
        {
            return Ok(field_id);
        }
    }
    Err(OxideError::InvalidInput {
        reason: format!("form field '{name}' not found"),
    })
}

fn acroform_field_ids(document: &lopdf::Document) -> Result<Vec<lopdf::ObjectId>, OxideError> {
    let Some(acroform) = acroform(document)? else {
        return Ok(Vec::new());
    };
    let fields = acroform
        .get(b"Fields")
        .and_then(Object::as_array)
        .map_err(|_| OxideError::ParsePdf)?;
    fields
        .iter()
        .map(|field| field.as_reference().map_err(|_| OxideError::ParsePdf))
        .collect()
}

fn acroform(document: &lopdf::Document) -> Result<Option<&lopdf::Dictionary>, OxideError> {
    let catalog = document.catalog().map_err(|_| OxideError::ParsePdf)?;
    let Ok(object) = catalog.get(b"AcroForm") else {
        return Ok(None);
    };
    match object {
        Object::Dictionary(dictionary) => Ok(Some(dictionary)),
        Object::Reference(id) => document
            .get_dictionary(*id)
            .map(Some)
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
