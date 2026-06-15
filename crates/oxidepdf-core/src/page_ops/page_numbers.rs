use super::options::{PageNumberPosition, PageNumbersOptions};
use super::selection::selected_or_all_pages;
use crate::{
    OxideError, PdfArtifact, ResourceLimits, add_resource_dict_entry, enforce_input_bytes,
    enforce_max_pages, enforce_output_bytes, load_pdf, page_size, save_pdf,
};
use lopdf::{Object, dictionary};

/// Adds page numbers to selected PDF pages.
pub fn add_pdf_page_numbers(
    input: &[u8],
    options: &PageNumbersOptions,
) -> Result<PdfArtifact, OxideError> {
    add_pdf_page_numbers_with_limits(input, options, &ResourceLimits::default())
}

/// Adds page numbers to selected PDF pages while enforcing resource limits.
pub fn add_pdf_page_numbers_with_limits(
    input: &[u8],
    options: &PageNumbersOptions,
    limits: &ResourceLimits,
) -> Result<PdfArtifact, OxideError> {
    enforce_input_bytes(input.len(), limits)?;
    let mut document = load_pdf(input)?;
    add_page_numbers_on_document(&mut document, options, limits)?;
    let bytes = save_pdf(document)?;
    enforce_output_bytes(bytes.len(), limits)?;
    Ok(PdfArtifact {
        bytes: crate::ArtifactBytes::from_vec(bytes)?,
    })
}

/// Adds page numbers to selected pages of an already-parsed document.
pub(crate) fn add_page_numbers_on_document(
    document: &mut lopdf::Document,
    options: &PageNumbersOptions,
    limits: &ResourceLimits,
) -> Result<(), OxideError> {
    validate_page_number_options(options)?;
    let page_count = document.get_pages().len() as u32;
    enforce_max_pages(page_count as usize, limits)?;
    let selected_pages = selected_or_all_pages(options.pages.as_deref(), page_count)?;
    add_standard_font_resource(document, &selected_pages, b"OxPnF1".to_vec())?;
    let page_map = document.get_pages();
    for (index, page_number) in selected_pages.iter().enumerate() {
        let page_id = *page_map
            .get(page_number)
            .ok_or_else(|| OxideError::InvalidInput {
                reason: format!("page {page_number} is out of range"),
            })?;
        let (page_width, page_height) = page_size(document, page_id)?;
        let number =
            options
                .start
                .checked_add(index as u32)
                .ok_or_else(|| OxideError::InvalidInput {
                    reason: "page number exceeds the maximum representable value".to_owned(),
                })?;
        let label = format!("{}{}{}", options.prefix, number, options.suffix);
        let content = page_number_content(
            &label,
            page_width,
            page_height,
            options.font_size,
            options.position,
        )?;
        document
            .add_page_contents(page_id, content)
            .map_err(|_| OxideError::WritePdf)?;
    }
    Ok(())
}

fn validate_page_number_options(options: &PageNumbersOptions) -> Result<(), OxideError> {
    if options.start == 0 {
        return Err(OxideError::InvalidInput {
            reason: "page number start must be greater than zero".to_owned(),
        });
    }
    if !options.font_size.is_finite() || options.font_size <= 0.0 {
        return Err(OxideError::InvalidInput {
            reason: "page number font size must be greater than zero".to_owned(),
        });
    }
    if !options.prefix.is_ascii() || !options.suffix.is_ascii() {
        return Err(OxideError::InvalidInput {
            reason: "page number prefix and suffix must be ASCII".to_owned(),
        });
    }
    Ok(())
}

fn add_standard_font_resource(
    document: &mut lopdf::Document,
    pages: &[u32],
    resource_name: Vec<u8>,
) -> Result<(), OxideError> {
    let font_id = document.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
        "Encoding" => "WinAnsiEncoding",
    });
    let page_map = document.get_pages();
    let page_ids = pages
        .iter()
        .map(|page| {
            page_map
                .get(page)
                .copied()
                .ok_or_else(|| OxideError::InvalidInput {
                    reason: format!("page {page} is out of range"),
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    for page_id in page_ids {
        add_resource_dict_entry(
            document,
            page_id,
            b"Font",
            resource_name.clone(),
            Object::Reference(font_id),
        )?;
    }
    Ok(())
}

fn page_number_content(
    label: &str,
    page_width: f32,
    page_height: f32,
    font_size: f32,
    position: PageNumberPosition,
) -> Result<Vec<u8>, OxideError> {
    let escaped_label = pdf_literal_ascii(label)?;
    let width = label.chars().count() as f32 * font_size * 0.5;
    let margin = 36.0;
    let (x, y) = match position {
        PageNumberPosition::TopLeft => (margin, page_height - margin),
        PageNumberPosition::TopCenter => ((page_width - width) / 2.0, page_height - margin),
        PageNumberPosition::TopRight => (page_width - margin - width, page_height - margin),
        PageNumberPosition::BottomLeft => (margin, margin),
        PageNumberPosition::BottomCenter => ((page_width - width) / 2.0, margin),
        PageNumberPosition::BottomRight => (page_width - margin - width, margin),
    };
    let content = format!("q BT /OxPnF1 {font_size} Tf {x} {y} Td ({escaped_label}) Tj ET Q\n");
    Ok(content.into_bytes())
}

fn pdf_literal_ascii(value: &str) -> Result<String, OxideError> {
    if !value.is_ascii() {
        return Err(OxideError::InvalidInput {
            reason: "page number text must be ASCII".to_owned(),
        });
    }
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '(' => escaped.push_str("\\("),
            ')' => escaped.push_str("\\)"),
            '\\' => escaped.push_str("\\\\"),
            _ => escaped.push(character),
        }
    }
    Ok(escaped)
}
