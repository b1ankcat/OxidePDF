use super::crop_box_object;
use super::options::{BookletOptions, NUpOptions};
use crate::{
    OxideError, PdfArtifact, ResourceLimits, enforce_input_bytes, enforce_max_pages,
    enforce_output_bytes, load_pdf, merge_resource_dictionary, page_size,
    remap_imported_references, save_pdf,
};
use lopdf::{Dictionary, Object, Stream, dictionary};
use std::collections::BTreeMap;

/// Lays multiple source pages on each output page.
pub fn nup_pdf_pages(input: &[u8], options: &NUpOptions) -> Result<PdfArtifact, OxideError> {
    nup_pdf_pages_with_limits(input, options, &ResourceLimits::default())
}

/// Lays multiple source pages on each output page while enforcing resource limits.
pub fn nup_pdf_pages_with_limits(
    input: &[u8],
    options: &NUpOptions,
    limits: &ResourceLimits,
) -> Result<PdfArtifact, OxideError> {
    validate_nup_options(options)?;
    enforce_input_bytes(input.len(), limits)?;
    let source = load_pdf(input)?;
    let page_ids = source.get_pages().into_values().collect::<Vec<_>>();
    enforce_max_pages(page_ids.len(), limits)?;
    let layout = page_layout_from_first_page(&source, &page_ids)?;
    let slots_per_page = (options.columns * options.rows) as usize;
    let output_count = page_ids.len().div_ceil(slots_per_page);
    enforce_max_pages(output_count, limits)?;
    let order = (0..page_ids.len()).collect::<Vec<_>>();

    let bytes = impose_pages(
        &source,
        &page_ids,
        &order,
        layout,
        options.columns,
        options.rows,
    )?;
    enforce_output_bytes(bytes.len(), limits)?;
    Ok(PdfArtifact {
        bytes: crate::ArtifactBytes::from_vec(bytes)?,
    })
}

/// Arranges pages for booklet printing.
pub fn booklet_pdf_pages(
    input: &[u8],
    options: &BookletOptions,
) -> Result<PdfArtifact, OxideError> {
    booklet_pdf_pages_with_limits(input, options, &ResourceLimits::default())
}

/// Arranges pages for booklet printing while enforcing resource limits.
pub fn booklet_pdf_pages_with_limits(
    input: &[u8],
    _options: &BookletOptions,
    limits: &ResourceLimits,
) -> Result<PdfArtifact, OxideError> {
    enforce_input_bytes(input.len(), limits)?;
    let source = load_pdf(input)?;
    let page_ids = source.get_pages().into_values().collect::<Vec<_>>();
    enforce_max_pages(page_ids.len(), limits)?;
    if page_ids.len() < 2 {
        return Err(OxideError::InvalidInput {
            reason: "booklet requires at least two pages".to_owned(),
        });
    }
    let layout = page_layout_from_first_page(&source, &page_ids)?;
    let sheet_count = page_ids.len().div_ceil(4);
    enforce_max_pages(sheet_count * 2, limits)?;
    let order = booklet_page_order(page_ids.len());

    let bytes = impose_pages(&source, &page_ids, &order, layout, 2, 1)?;
    enforce_output_bytes(bytes.len(), limits)?;
    Ok(PdfArtifact {
        bytes: crate::ArtifactBytes::from_vec(bytes)?,
    })
}

#[derive(Debug, Clone, Copy)]
struct PageLayout {
    output_width: f32,
    output_height: f32,
}

fn validate_nup_options(options: &NUpOptions) -> Result<(), OxideError> {
    if options.columns == 0 || options.rows == 0 {
        return Err(OxideError::InvalidInput {
            reason: "nup columns and rows must be greater than zero".to_owned(),
        });
    }
    if options.columns > 8 || options.rows > 8 {
        return Err(OxideError::InvalidInput {
            reason: "nup columns and rows must be 8 or less".to_owned(),
        });
    }
    Ok(())
}

fn page_layout_from_first_page(
    document: &lopdf::Document,
    page_ids: &[lopdf::ObjectId],
) -> Result<PageLayout, OxideError> {
    let first_page = page_ids.first().copied().ok_or(OxideError::ParsePdf)?;
    let (source_width, source_height) = page_size(document, first_page)?;
    if !source_width.is_finite()
        || !source_height.is_finite()
        || source_width <= 0.0
        || source_height <= 0.0
    {
        return Err(OxideError::ParsePdf);
    }

    Ok(PageLayout {
        output_width: source_width,
        output_height: source_height,
    })
}

fn booklet_page_order(page_count: usize) -> Vec<usize> {
    let padded_count = page_count.div_ceil(4) * 4;
    let mut order = Vec::with_capacity(padded_count);
    for sheet in 0..(padded_count / 4) {
        let left_front = padded_count - sheet * 2 - 1;
        let right_front = sheet * 2;
        let left_back = sheet * 2 + 1;
        let right_back = padded_count - sheet * 2 - 2;
        order.extend([left_front, right_front, left_back, right_back]);
    }
    order
}

fn impose_pages(
    source: &lopdf::Document,
    page_ids: &[lopdf::ObjectId],
    order: &[usize],
    layout: PageLayout,
    columns: u32,
    rows: u32,
) -> Result<Vec<u8>, OxideError> {
    let mut target = lopdf::Document::with_version("1.7");
    let catalog_id = target.new_object_id();
    let pages_id = target.new_object_id();
    let mut output_page_ids = Vec::new();
    let mut imported = BTreeMap::new();
    let slots_per_page = (columns * rows) as usize;

    for chunk in order.chunks(slots_per_page) {
        let page_id = target.new_object_id();
        let content_id = target.new_object_id();
        let mut resources = Dictionary::new();
        let mut xobjects = Dictionary::new();
        let mut operations = Vec::new();

        for (slot, source_index) in chunk.iter().enumerate() {
            let Some(&source_page_id) = page_ids.get(*source_index) else {
                continue;
            };
            let (source_width, source_height) = page_size(source, source_page_id)?;
            let xobject_id =
                page_form_xobject_from_source(source, &mut target, source_page_id, &mut imported)?;
            let resource_name = format!("OxPg{slot}").into_bytes();
            xobjects.set(resource_name.clone(), Object::Reference(xobject_id));
            operations.extend(imposed_page_operations(
                &resource_name,
                slot,
                columns,
                rows,
                layout,
                source_width,
                source_height,
            )?);
        }

        resources.set("XObject", Object::Dictionary(xobjects));
        let content = lopdf::content::Content { operations }
            .encode()
            .map_err(|_| OxideError::WritePdf)?;
        target.objects.insert(
            content_id,
            Object::Stream(Stream::new(Dictionary::new(), content)),
        );
        target.objects.insert(
            page_id,
            Object::Dictionary(dictionary! {
                "Type" => "Page",
                "Parent" => pages_id,
                "MediaBox" => crop_box_object([0.0, 0.0, layout.output_width, layout.output_height]),
                "CropBox" => crop_box_object([0.0, 0.0, layout.output_width, layout.output_height]),
                "Resources" => Object::Dictionary(resources),
                "Contents" => Object::Reference(content_id),
            }),
        );
        output_page_ids.push(page_id);
    }

    target.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => Object::Array(output_page_ids.iter().copied().map(Object::Reference).collect()),
            "Count" => output_page_ids.len() as u32,
        }),
    );
    target.objects.insert(
        catalog_id,
        Object::Dictionary(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        }),
    );
    target.trailer.set("Root", catalog_id);
    save_pdf(target)
}
