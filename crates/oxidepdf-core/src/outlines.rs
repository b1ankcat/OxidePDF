use crate::{
    enforce_input_bytes, enforce_max_pages, enforce_output_bytes, load_pdf, save_pdf, OxideError,
    PdfArtifact, ResourceLimits, TextArtifact,
};
use lopdf::{dictionary, Dictionary, Object};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct OutlineInspectOptions {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutlineEditOptions {
    pub action: OutlineEditAction,
    pub tree: Option<OutlineTree>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutlineEditAction {
    Set,
    Delete,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct OutlineTree {
    pub items: Vec<OutlineItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutlineItem {
    pub title: String,
    pub page: u32,
    #[serde(default)]
    pub children: Vec<OutlineItem>,
}

pub fn inspect_pdf_outline(
    input: &[u8],
    _options: &OutlineInspectOptions,
) -> Result<TextArtifact, OxideError> {
    let document = load_pdf(input)?;
    let tree = read_outline_tree(&document)?;
    let text = serde_json::to_string_pretty(&tree).map_err(|_| OxideError::Internal)?;
    Ok(TextArtifact {
        text,
        diagnostics: Vec::new(),
    })
}

pub fn edit_pdf_outline(
    input: &[u8],
    options: &OutlineEditOptions,
    limits: &ResourceLimits,
) -> Result<PdfArtifact, OxideError> {
    enforce_input_bytes(input.len(), limits)?;
    let mut document = load_pdf(input)?;
    enforce_max_pages(document.get_pages().len(), limits)?;
    match options.action {
        OutlineEditAction::Set => {
            let tree = options
                .tree
                .as_ref()
                .ok_or_else(|| OxideError::InvalidInput {
                    reason: "outline set requires a tree".to_owned(),
                })?;
            validate_outline_tree(&document, tree)?;
            write_outline_tree(&mut document, tree)?;
        }
        OutlineEditAction::Delete => delete_outline_tree(&mut document)?,
    }
    let bytes = save_pdf(document)?;
    enforce_output_bytes(bytes.len(), limits)?;
    Ok(PdfArtifact {
        bytes: bytes.into(),
    })
}

fn read_outline_tree(document: &lopdf::Document) -> Result<OutlineTree, OxideError> {
    let catalog = document.catalog().map_err(|_| OxideError::ParsePdf)?;
    let Ok(outlines_id) = catalog.get(b"Outlines").and_then(Object::as_reference) else {
        return Ok(OutlineTree::default());
    };
    let outlines = document
        .get_object(outlines_id)
        .and_then(Object::as_dict)
        .map_err(|_| OxideError::ParsePdf)?;
    let Ok(first_id) = outlines.get(b"First").and_then(Object::as_reference) else {
        return Ok(OutlineTree::default());
    };
    Ok(OutlineTree {
        items: read_outline_siblings(document, first_id)?,
    })
}

fn read_outline_siblings(
    document: &lopdf::Document,
    mut current_id: lopdf::ObjectId,
) -> Result<Vec<OutlineItem>, OxideError> {
    let mut items = Vec::new();
    loop {
        let item = document
            .get_object(current_id)
            .and_then(Object::as_dict)
            .map_err(|_| OxideError::ParsePdf)?;
        let title = item
            .get(b"Title")
            .ok()
            .map(pdf_string)
            .transpose()?
            .unwrap_or_default();
        let page = read_outline_item_page(document, item)?;
        let children = match item.get(b"First").and_then(Object::as_reference) {
            Ok(child_id) => read_outline_siblings(document, child_id)?,
            Err(_) => Vec::new(),
        };
        items.push(OutlineItem {
            title,
            page,
            children,
        });
        match item.get(b"Next").and_then(Object::as_reference) {
            Ok(next_id) => current_id = next_id,
            Err(_) => break,
        }
    }
    Ok(items)
}

fn write_outline_tree(
    document: &mut lopdf::Document,
    tree: &OutlineTree,
) -> Result<(), OxideError> {
    delete_outline_tree(document)?;
    if tree.items.is_empty() {
        return Ok(());
    }
    let outlines_id = document.new_object_id();
    let (first, last, count) = write_outline_items(document, outlines_id, &tree.items)?;
    document.objects.insert(
        outlines_id,
        Object::Dictionary(dictionary! {
            "Type" => "Outlines",
            "First" => first,
            "Last" => last,
            "Count" => count,
        }),
    );
    catalog_mut(document)?.set("Outlines", outlines_id);
    Ok(())
}

fn write_outline_items(
    document: &mut lopdf::Document,
    parent_id: lopdf::ObjectId,
    items: &[OutlineItem],
) -> Result<(lopdf::ObjectId, lopdf::ObjectId, i64), OxideError> {
    let ids = (0..items.len())
        .map(|_| document.new_object_id())
        .collect::<Vec<_>>();
    let mut total_count = 0i64;
    for (index, item) in items.iter().enumerate() {
        let id = ids[index];
        let page_id = page_id_for_number(document, item.page)?;
        let mut dictionary = dictionary! {
            "Title" => Object::string_literal(item.title.as_str()),
            "Parent" => parent_id,
            "Dest" => Object::Array(vec![
                Object::Reference(page_id),
                Object::Name(b"Fit".to_vec()),
            ]),
        };
        if index > 0 {
            dictionary.set("Prev", ids[index - 1]);
        }
        if index + 1 < ids.len() {
            dictionary.set("Next", ids[index + 1]);
        }
        let mut item_count = 1i64;
        if !item.children.is_empty() {
            let (first, last, child_count) = write_outline_items(document, id, &item.children)?;
            dictionary.set("First", first);
            dictionary.set("Last", last);
            dictionary.set("Count", child_count);
            item_count += child_count;
        }
        total_count += item_count;
        document.objects.insert(id, Object::Dictionary(dictionary));
    }
    Ok((ids[0], *ids.last().unwrap(), total_count))
}

fn delete_outline_tree(document: &mut lopdf::Document) -> Result<(), OxideError> {
    catalog_mut(document)?.remove(b"Outlines");
    Ok(())
}

fn validate_outline_tree(document: &lopdf::Document, tree: &OutlineTree) -> Result<(), OxideError> {
    for item in &tree.items {
        validate_outline_item(document, item)?;
    }
    Ok(())
}

fn validate_outline_item(document: &lopdf::Document, item: &OutlineItem) -> Result<(), OxideError> {
    page_id_for_number(document, item.page)?;
    for child in &item.children {
        validate_outline_item(document, child)?;
    }
    Ok(())
}

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
