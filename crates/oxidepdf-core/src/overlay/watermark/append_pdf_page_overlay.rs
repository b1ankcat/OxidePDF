
fn append_pdf_page_overlay(
    target: &mut lopdf::Document,
    pages: &[u32],
    source: &[u8],
    source_page: Option<u32>,
    settings: WatermarkSettings,
) -> Result<(), OxideError> {
    let source = load_pdf(source)?;
    let source_page = source_page.unwrap_or(1);
    let source_page_id =
        *source
            .get_pages()
            .get(&source_page)
            .ok_or_else(|| OxideError::InvalidInput {
                reason: format!("source page {source_page} is out of range"),
            })?;
    let (width, height) = page_size(&source, source_page_id)?;
    let content = source
        .get_page_content(source_page_id)
        .map_err(|_| OxideError::ParsePdf)?;
    let resources = imported_page_resources(&source, target, source_page_id)?;
    let form_id = target.add_object(form_xobject(width, height, resources, content));
    append_xobject_watermark(
        target,
        pages,
        form_id,
        width,
        height,
        b"OxPdfOverlay".to_vec(),
        settings,
    )
}

fn svg_form_xobject(
    target: &mut lopdf::Document,
    tree: &svg2pdf::usvg::Tree,
    width: f32,
    height: f32,
) -> Result<lopdf::ObjectId, OxideError> {
    let conversion_options = svg2pdf::ConversionOptions {
        embed_text: false,
        ..svg2pdf::ConversionOptions::default()
    };
    let bytes = svg2pdf::to_pdf(tree, conversion_options, svg2pdf::PageOptions::default())
        .map_err(|_| OxideError::WritePdf)?;
    let source = lopdf::Document::load_mem(&bytes).map_err(|_| OxideError::ParsePdf)?;
    let page_id = source
        .get_pages()
        .into_values()
        .next()
        .ok_or(OxideError::ParsePdf)?;
    let content = source
        .get_page_content(page_id)
        .map_err(|_| OxideError::ParsePdf)?;
    let resources = imported_page_resources(&source, target, page_id)?;

    let mut dict = dictionary! {
        "Type" => "XObject",
        "Subtype" => "Form",
        "BBox" => Object::Array(vec![
            Object::Real(0.0),
            Object::Real(0.0),
            Object::Real(width),
            Object::Real(height),
        ]),
        "Matrix" => Object::Array(vec![
            Object::Real(1.0),
            Object::Real(0.0),
            Object::Real(0.0),
            Object::Real(1.0),
            Object::Real(0.0),
            Object::Real(0.0),
        ]),
    };
    dict.set("Resources", resources);
    Ok(target.add_object(Stream::new(dict, content)))
}

fn form_xobject(width: f32, height: f32, resources: Dictionary, content: Vec<u8>) -> Stream {
    let mut dict = dictionary! {
        "Type" => "XObject",
        "Subtype" => "Form",
        "BBox" => Object::Array(vec![
            Object::Real(0.0),
            Object::Real(0.0),
            Object::Real(width),
            Object::Real(height),
        ]),
        "Matrix" => Object::Array(vec![
            Object::Real(1.0),
            Object::Real(0.0),
            Object::Real(0.0),
            Object::Real(1.0),
            Object::Real(0.0),
            Object::Real(0.0),
        ]),
    };
    dict.set("Resources", resources);
    Stream::new(dict, content)
}
