use crate::OxideError;
use lopdf::Document;
use lopdf::xref::XrefEntry;

/// Lifts objects stored in object streams (`XrefEntry::Compressed`) into the
/// document by decoding each already-decrypted container stream.
pub(crate) fn decode_compressed_objects(
    encrypted_document: &Document,
    document: &mut Document,
) -> Result<(), OxideError> {
    use std::collections::BTreeSet;

    // Collect the container object numbers referenced by compressed entries.
    let containers = encrypted_document
        .reference_table
        .entries
        .values()
        .filter_map(|entry| match entry {
            XrefEntry::Compressed { container, .. } => Some(*container),
            _ => None,
        })
        .collect::<BTreeSet<_>>();

    for container in containers {
        let container_id = (container, 0);
        let Some(object) = document.objects.get_mut(&container_id) else {
            // Container missing from the decrypted set; nothing to expand.
            continue;
        };
        let stream = object.as_stream_mut().map_err(|_| OxideError::ParsePdf)?;
        let object_stream = lopdf::ObjectStream::new(stream).map_err(|_| OxideError::ParsePdf)?;
        for (id, decoded) in object_stream.objects {
            document.objects.entry(id).or_insert(decoded);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_compressed_objects_lifts_object_stream_contents() {
        use lopdf::xref::{Xref, XrefEntry, XrefType};
        use lopdf::{Document, Object, ObjectStream};

        // Build a valid object stream container holding two compressed objects.
        let mut builder = ObjectStream::builder().build();
        builder.add_object((10, 0), Object::Integer(42)).unwrap();
        builder
            .add_object((11, 0), Object::string_literal("inside objstm"))
            .unwrap();
        let container_stream = builder.to_stream_object().unwrap();

        // The "encrypted" document supplies the xref: object 5 is the container,
        // objects 10 and 11 are compressed inside it.
        let mut source = Document::with_version("1.5");
        let mut xref = Xref::new(12, XrefType::CrossReferenceStream);
        xref.insert(
            5,
            XrefEntry::Normal {
                offset: 0,
                generation: 0,
            },
        );
        xref.insert(
            10,
            XrefEntry::Compressed {
                container: 5,
                index: 0,
            },
        );
        xref.insert(
            11,
            XrefEntry::Compressed {
                container: 5,
                index: 1,
            },
        );
        source.reference_table = xref;

        // The decrypted document already holds the (decrypted) container object.
        let mut document = Document::with_version("1.5");
        document
            .objects
            .insert((5, 0), Object::Stream(container_stream));

        decode_compressed_objects(&source, &mut document).unwrap();

        // Both compressed objects are now first-class document objects.
        assert_eq!(document.objects.get(&(10, 0)), Some(&Object::Integer(42)));
        assert_eq!(
            document.objects.get(&(11, 0)),
            Some(&Object::string_literal("inside objstm"))
        );
    }
}
