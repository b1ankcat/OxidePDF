
fn read_outline_item_page(
    document: &lopdf::Document,
    item: &Dictionary,
) -> Result<u32, OxideError> {
    let dest = item
        .get(b"Dest")
        .map_err(|_| OxideError::UnsupportedPdfFeature {
            feature: "outline item without Dest is not supported".to_owned(),
        })?;
    let dest = dest
        .as_array()
        .map_err(|_| OxideError::UnsupportedPdfFeature {
            feature: "outline destinations other than explicit page arrays are not supported"
                .to_owned(),
        })?;
    let page_id = dest
        .first()
        .ok_or_else(|| OxideError::UnsupportedPdfFeature {
            feature: "empty outline destination arrays are not supported".to_owned(),
        })?
        .as_reference()
        .map_err(|_| OxideError::UnsupportedPdfFeature {
            feature: "outline destinations without page references are not supported".to_owned(),
        })?;
    page_number_for_id(document, page_id).ok_or(OxideError::ParsePdf)
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

fn page_number_for_id(document: &lopdf::Document, page_id: lopdf::ObjectId) -> Option<u32> {
    document
        .get_pages()
        .into_iter()
        .find_map(|(page, id)| (id == page_id).then_some(page))
}

fn catalog_mut(document: &mut lopdf::Document) -> Result<&mut Dictionary, OxideError> {
    document.catalog_mut().map_err(|_| OxideError::ParsePdf)
}

fn pdf_string(object: &Object) -> Result<String, OxideError> {
    object
        .as_str()
        .map(|value| String::from_utf8_lossy(value).into_owned())
        .map_err(|_| OxideError::ParsePdf)
}
