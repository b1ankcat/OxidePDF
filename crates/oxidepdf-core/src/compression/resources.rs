use crate::OxideError;
use lopdf::{Dictionary, Object, Stream};
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn merge_duplicate_resource_streams(
    document: &mut lopdf::Document,
) -> Result<(), OxideError> {
    let mut duplicate_map = BTreeMap::new();
    let mut seen_streams = BTreeMap::new();
    for (id, object) in &document.objects {
        let Object::Stream(stream) = object else {
            continue;
        };
        let Some(subtype) = stream_subtype(stream) else {
            continue;
        };
        if subtype != b"Image" && subtype != b"Form" {
            continue;
        }
        let key = ResourceStreamKey::new(subtype.to_vec(), stream)?;
        if let Some(canonical_id) = seen_streams.get(&key) {
            duplicate_map.insert(*id, *canonical_id);
        } else {
            seen_streams.insert(key, *id);
        }
    }

    if duplicate_map.is_empty() {
        return Ok(());
    }

    for object in document.objects.values_mut() {
        remap_duplicate_references(object, &duplicate_map, 0)?;
    }
    for duplicate_id in duplicate_map.keys() {
        document.objects.remove(duplicate_id);
    }

    Ok(())
}

pub(super) fn stream_subtype(stream: &Stream) -> Option<&[u8]> {
    stream.dict.get(b"Subtype").and_then(Object::as_name).ok()
}

/// Maximum object-graph depth traversed when remapping or hashing resource
/// streams. Bounds stack usage on a crafted deeply nested PDF object graph.
const MAX_RESOURCE_DEPTH: u32 = 256;

fn remap_duplicate_references(
    object: &mut Object,
    duplicate_map: &BTreeMap<lopdf::ObjectId, lopdf::ObjectId>,
    depth: u32,
) -> Result<(), OxideError> {
    if depth >= MAX_RESOURCE_DEPTH {
        return Err(OxideError::ParsePdf);
    }
    match object {
        Object::Reference(id) => {
            if let Some(canonical_id) = duplicate_map.get(id) {
                *id = *canonical_id;
            }
        }
        Object::Array(items) => {
            for item in items {
                remap_duplicate_references(item, duplicate_map, depth + 1)?;
            }
        }
        Object::Dictionary(dictionary) => {
            for (_, value) in dictionary.iter_mut() {
                remap_duplicate_references(value, duplicate_map, depth + 1)?;
            }
        }
        Object::Stream(stream) => {
            for (_, value) in stream.dict.iter_mut() {
                remap_duplicate_references(value, duplicate_map, depth + 1)?;
            }
        }
        _ => {}
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct ResourceStreamKey {
    subtype: Vec<u8>,
    dictionary_entries: Vec<(Vec<u8>, ComparableObject)>,
    content: Vec<u8>,
}

impl ResourceStreamKey {
    fn new(subtype: Vec<u8>, stream: &Stream) -> Result<Self, OxideError> {
        let mut dictionary_entries = comparable_dictionary(&stream.dict, 0)?;
        dictionary_entries.retain(|(key, _)| key.as_slice() != b"Length");
        Ok(Self {
            subtype,
            dictionary_entries,
            content: stream.content.clone(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum ComparableObject {
    Null,
    Boolean(bool),
    Integer(i64),
    Real(u32),
    Name(Vec<u8>),
    String(Vec<u8>),
    Array(Vec<ComparableObject>),
    Dictionary(Vec<(Vec<u8>, ComparableObject)>),
    Reference(lopdf::ObjectId),
}

fn comparable_dictionary(
    dictionary: &Dictionary,
    depth: u32,
) -> Result<Vec<(Vec<u8>, ComparableObject)>, OxideError> {
    if depth >= MAX_RESOURCE_DEPTH {
        return Err(OxideError::ParsePdf);
    }
    let mut entries = dictionary
        .iter()
        .map(|(key, value)| Ok((key.clone(), comparable_object(value, depth + 1)?)))
        .collect::<Result<BTreeSet<_>, OxideError>>()?
        .into_iter()
        .collect::<Vec<_>>();
    entries.sort();
    Ok(entries)
}

fn comparable_object(object: &Object, depth: u32) -> Result<ComparableObject, OxideError> {
    if depth >= MAX_RESOURCE_DEPTH {
        return Err(OxideError::ParsePdf);
    }
    Ok(match object {
        Object::Null => ComparableObject::Null,
        Object::Boolean(value) => ComparableObject::Boolean(*value),
        Object::Integer(value) => ComparableObject::Integer(*value),
        // Normalize +0.0 and -0.0 to the same bit pattern so they dedup as equal.
        Object::Real(value) => ComparableObject::Real(normalize_real_bits(*value)),
        Object::Name(value) => ComparableObject::Name(value.clone()),
        Object::String(value, _) => ComparableObject::String(value.clone()),
        Object::Array(items) => ComparableObject::Array(
            items
                .iter()
                .map(|item| comparable_object(item, depth + 1))
                .collect::<Result<Vec<_>, OxideError>>()?,
        ),
        Object::Dictionary(dictionary) => {
            ComparableObject::Dictionary(comparable_dictionary(dictionary, depth + 1)?)
        }
        Object::Stream(stream) => {
            ComparableObject::Dictionary(comparable_dictionary(&stream.dict, depth + 1)?)
        }
        Object::Reference(id) => ComparableObject::Reference(*id),
    })
}

fn normalize_real_bits(value: f32) -> u32 {
    if value == 0.0 {
        0.0f32.to_bits()
    } else {
        value.to_bits()
    }
}
