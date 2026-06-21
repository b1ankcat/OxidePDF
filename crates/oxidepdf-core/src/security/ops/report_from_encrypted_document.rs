
fn report_from_encrypted_document(document: &Document) -> Result<PermissionReport, OxideError> {
    let encrypted = document
        .get_encrypted()
        .map_err(|_| OxideError::EncryptedPdf)?;
    let version = encrypted.get(b"V").ok().and_then(object_i64);
    let revision = encrypted.get(b"R").ok().and_then(object_i64);
    let key_length_bits = encrypted.get(b"Length").ok().and_then(object_i64);
    let permissions_bits = encrypted.get(b"P").ok().and_then(object_i64);
    let handler = encrypted
        .get(b"Filter")
        .ok()
        .and_then(|object| object.as_name().ok())
        .map(|name| String::from_utf8_lossy(name).into_owned());

    Ok(PermissionReport {
        encrypted: true,
        handler,
        version,
        revision,
        key_length_bits,
        permissions_bits,
        permissions: permissions_bits
            .map(PermissionPolicy::from_bits)
            .unwrap_or_default(),
    })
}

fn encryption_revision(document: &Document) -> Result<i64, OxideError> {
    document
        .get_encrypted()
        .ok()
        .and_then(|dict| dict.get(b"R").ok())
        .and_then(object_i64)
        .ok_or_else(|| OxideError::UnsupportedPdfFeature {
            feature: "encrypted PDF without Standard Security Handler revision".to_owned(),
        })
}

fn object_i64(object: &Object) -> Option<i64> {
    match object {
        Object::Integer(value) => Some(*value),
        _ => None,
    }
}

fn save_security_pdf(
    mut document: Document,
    limits: &ResourceLimits,
) -> Result<PdfArtifact, OxideError> {
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .map_err(|_| OxideError::WritePdf)?;
    enforce_output_bytes(bytes.len(), limits)?;
    Ok(PdfArtifact {
        bytes: crate::ArtifactBytes::from_vec(bytes)?,
    })
}

fn normalize_plain_document(mut document: Document) -> Result<Document, OxideError> {
    document.prune_objects();
    document.renumber_objects();
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .map_err(|_| OxideError::WritePdf)?;
    Document::load_mem(&bytes).map_err(|_| OxideError::ParsePdf)
}

fn load_decrypted_document(
    input: &[u8],
    encrypted_document: &Document,
    password: &str,
) -> Result<Document, OxideError> {
    encrypted_document
        .authenticate_password(password)
        .map_err(map_lopdf_security_error)?;
    let state =
        EncryptionState::decode(encrypted_document, password).map_err(map_lopdf_security_error)?;
    let encryption_object_id = encrypted_document
        .trailer
        .get(b"Encrypt")
        .ok()
        .and_then(|object| object.as_reference().ok());

    let mut document = encrypted_document.clone();
    document.objects.clear();

    // Decrypt every directly-addressed (uncompressed) object first. Object
    // stream containers are themselves Normal stream objects, so this also
    // decrypts the containers we need below.
    for (object_id, offset, generation) in normal_object_offsets(encrypted_document) {
        if Some(object_id) == encryption_object_id {
            continue;
        }
        let Some(raw) = raw_indirect_object(input, offset) else {
            return Err(OxideError::ParsePdf);
        };
        let mut object = parse_single_indirect_object(&raw, object_id, generation)?;
        lopdf::encryption::decrypt_object(&state, object_id, &mut object)
            .map_err(|error| map_lopdf_security_error(lopdf::Error::Decryption(error)))?;
        document.objects.insert(object_id, object);
    }

    // Now expand any compressed objects. Per PDF spec, objects inside an object
    // stream are not individually encrypted — the encryption applies to the
    // container stream as a whole, which the loop above already decrypted. So we
    // decode each container in place and lift its objects into the document.
    decode_compressed_objects(encrypted_document, &mut document)?;

    document.trailer.remove(b"Encrypt");
    document.encryption_state = None;
    Ok(document)
}

fn normal_object_offsets(document: &Document) -> Vec<(lopdf::ObjectId, usize, u16)> {
    document
        .reference_table
        .entries
        .iter()
        .filter_map(|(&id, entry)| match *entry {
            XrefEntry::Normal { offset, generation } => {
                Some(((id, generation), offset as usize, generation))
            }
            _ => None,
        })
        .collect()
}

fn raw_indirect_object(input: &[u8], offset: usize) -> Option<Vec<u8>> {
    let slice = input.get(offset..)?;
    let end = slice
        .windows(b"endobj".len())
        .position(|window| window == b"endobj")?
        + b"endobj".len();
    Some(slice[..end].to_vec())
}

fn parse_single_indirect_object(
    raw: &[u8],
    object_id: lopdf::ObjectId,
    generation: u16,
) -> Result<Object, OxideError> {
    // Wrap the raw object bytes in the minimal valid PDF envelope required by
    // lopdf's load_mem. Layout (bytes):
    //   [0..9)   %PDF-1.7\n          (9-byte header)
    //   [9..end) <raw object bytes>   object_offset = 9
    //   \n
    //   xref table at xref_start = 9 + raw.len() + 1
    // The xref entry points to offset 9 where the object begins. This layout
    // is tight: any change to what is prepended must update object_offset.
    let header = b"%PDF-1.7\n";
    let object_offset = header.len();
    let xref_start = object_offset + raw.len() + 1;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(header);
    bytes.extend_from_slice(raw);
    bytes.extend_from_slice(b"\n");
    bytes.extend_from_slice(
        format!(
            "xref\n0 1\n0000000000 65535 f \n{} 1\n{:010} {:05} n \ntrailer\n<< /Size {} >>\nstartxref\n{}\n%%EOF\n",
            object_id.0,
            object_offset,
            generation,
            object_id.0 + 1,
            xref_start,
        )
        .as_bytes(),
    );
    let document = Document::load_mem(&bytes).map_err(|_| OxideError::ParsePdf)?;
    document
        .objects
        .get(&object_id)
        .cloned()
        .ok_or(OxideError::ParsePdf)
}

fn map_lopdf_security_error(error: lopdf::Error) -> OxideError {
    match error {
        lopdf::Error::Decryption(lopdf::encryption::DecryptionError::IncorrectPassword) => {
            OxideError::IncorrectPassword
        }
        lopdf::Error::Decryption(lopdf::encryption::DecryptionError::UnsupportedRevision) => {
            OxideError::UnsupportedPdfFeature {
                feature: "unsupported Standard Security Handler revision".to_owned(),
            }
        }
        lopdf::Error::Decryption(lopdf::encryption::DecryptionError::UnsupportedVersion) => {
            OxideError::UnsupportedPdfFeature {
                feature: "unsupported Standard Security Handler version".to_owned(),
            }
        }
        lopdf::Error::UnsupportedSecurityHandler(_) => OxideError::UnsupportedPdfFeature {
            feature: "unsupported security handler".to_owned(),
        },
        lopdf::Error::AlreadyEncrypted => OxideError::EncryptedPdf,
        lopdf::Error::NotEncrypted => OxideError::InvalidInput {
            reason: "PDF is not encrypted".to_owned(),
        },
        lopdf::Error::Decryption(_) => OxideError::InvalidInput {
            reason: "PDF decryption failed".to_owned(),
        },
        _ => OxideError::ParsePdf,
    }
}

fn unsupported_rc4() -> OxideError {
    OxideError::UnsupportedPdfFeature {
        feature: "RC4 Standard Security Handler".to_owned(),
    }
}
