//! Shared fixtures and helpers for the `oxidepdf-core` integration tests.
//!
//! Everything here is built strictly on the crate's public API, so it can be
//! reused from the per-module integration test files under `tests/`.

#![allow(dead_code)]

use der::{pem::LineEnding, Decode, Encode, EncodePem};
use lopdf::{dictionary, Dictionary, Object, Stream};
use oxidepdf_core::{
    Artifact, ArtifactRef, MetadataEntry, OperatorRunner, OxideError, TaskSpec, Workflow,
};
use p256::pkcs8::EncodePrivateKey;
use pdf_writer::Finish;
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;
use x509_cert::builder::Builder;

pub const A4_WIDTH: f32 = 595.0;
pub const A4_HEIGHT: f32 = 842.0;

/// Equivalent of the crate-internal `object_to_f32`, expressed via the public
/// lopdf API so the integration tests stay on supported surfaces. Matches the
/// original Integer-or-Real acceptance (lopdf's `as_float`, not `as_f32`).
fn object_to_f32(object: &Object) -> Result<f32, OxideError> {
    object.as_float().map_err(|_| OxideError::ParsePdf)
}

pub fn workflow_from_json(json: &str) -> Workflow {
    serde_json::from_str(json).unwrap()
}

pub fn artifact_ref(value: &str) -> ArtifactRef {
    serde_json::from_str(&format!("{value:?}")).unwrap()
}

pub fn write_test_trust_anchors(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "oxidepdf_core_{name}_{}_anchors.pem",
        std::process::id()
    ));
    std::fs::write(
        &path,
        include_bytes!("../../../../tests/fixtures/test-trust-anchor.txt"),
    )
    .unwrap();
    path
}

pub fn write_p256_signing_material(name: &str) -> (PathBuf, PathBuf) {
    let dir = std::env::temp_dir().join(format!(
        "oxidepdf_core_{name}_{}_signing",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let private_key_path = dir.join("signer-key.pem");
    let certificate_path = dir.join("signer-cert.pem");

    let signing_key = p256::ecdsa::SigningKey::from_bytes((&[7u8; 32]).into()).unwrap();
    let private_key_pem = signing_key
        .to_pkcs8_pem(LineEnding::LF)
        .unwrap()
        .to_string();
    let verifying_key = *signing_key.verifying_key();
    let public_key = spki::SubjectPublicKeyInfoOwned::from_key(verifying_key).unwrap();
    let subject = x509_cert::name::Name::from_str("CN=OxidePDF Test Signer,O=OxidePDF,C=US")
        .unwrap()
        .to_der()
        .unwrap();
    let subject = x509_cert::name::Name::from_der(&subject).unwrap();
    let validity = x509_cert::time::Validity::from_now(Duration::from_secs(60 * 60)).unwrap();
    let serial_number = x509_cert::serial_number::SerialNumber::from(42u32);
    let certificate = x509_cert::builder::CertificateBuilder::new(
        x509_cert::builder::Profile::Root,
        serial_number,
        validity,
        subject,
        public_key,
        &signing_key,
    )
    .unwrap()
    .build::<p256::ecdsa::DerSignature>()
    .unwrap();
    let certificate_pem = certificate.to_pem(LineEnding::LF).unwrap();

    std::fs::write(&private_key_path, private_key_pem).unwrap();
    std::fs::write(&certificate_path, certificate_pem).unwrap();
    (certificate_path, private_key_path)
}

pub fn write_empty_trust_anchors(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "oxidepdf_core_{name}_{}_anchors.pem",
        std::process::id()
    ));
    std::fs::write(&path, "not a certificate bundle\n").unwrap();
    path
}

pub fn write_invalid_trust_anchors(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "oxidepdf_core_{name}_{}_anchors.pem",
        std::process::id()
    ));
    std::fs::write(
        &path,
        "-----BEGIN CERTIFICATE-----\nMIIB\n-----END CERTIFICATE-----\n",
    )
    .unwrap();
    path
}

pub fn encrypted_pdf_with_revision(revision: i64) -> Vec<u8> {
    let mut document = lopdf::Document::load_mem(&empty_page_pdf()).unwrap();
    let encrypt_id = document.new_object_id();
    document.objects.insert(
        encrypt_id,
        Object::Dictionary(lopdf::dictionary! {
            "Filter" => "Standard",
            "V" => 1,
            "R" => revision,
            "Length" => 40,
            "P" => -4,
            "O" => Object::string_literal(vec![0u8; 32]),
            "U" => Object::string_literal(vec![0u8; 32]),
        }),
    );
    document.trailer.set("Encrypt", encrypt_id);

    let mut bytes = Vec::new();
    document.save_to(&mut bytes).unwrap();
    bytes
}

pub fn pdf_with_signature_dictionary(byte_range: Vec<i64>, contents: Vec<u8>) -> Vec<u8> {
    pdf_with_signature_dictionary_and_subfilter(byte_range, contents, "adbe.pkcs7.detached")
}

pub fn pdf_with_signature_dictionary_and_subfilter(
    byte_range: Vec<i64>,
    contents: Vec<u8>,
    subfilter: &str,
) -> Vec<u8> {
    let mut document = lopdf::Document::with_version("1.7");
    let pages_id = document.new_object_id();
    let page_id = document.new_object_id();
    let sig_field_id = document.new_object_id();
    let sig_value_id = document.new_object_id();
    let acroform_id = document.new_object_id();
    let catalog_id = document.new_object_id();

    let byte_range = byte_range
        .into_iter()
        .map(lopdf::Object::Integer)
        .collect::<Vec<_>>();
    let sig_value = lopdf::dictionary! {
        "Type" => "Sig",
        "Filter" => "Adobe.PPKLite",
        "SubFilter" => subfilter,
        "ByteRange" => lopdf::Object::Array(byte_range),
        "Contents" => lopdf::Object::String(contents, lopdf::StringFormat::Hexadecimal),
    };
    document
        .objects
        .insert(sig_value_id, lopdf::Object::Dictionary(sig_value));

    let sig_field = lopdf::dictionary! {
        "Type" => "Annot",
        "Subtype" => "Widget",
        "FT" => "Sig",
        "T" => lopdf::Object::string_literal("Approval"),
        "V" => sig_value_id,
        "Rect" => lopdf::Object::Array(vec![0.into(), 0.into(), 0.into(), 0.into()]),
        "P" => page_id,
    };
    document
        .objects
        .insert(sig_field_id, lopdf::Object::Dictionary(sig_field));

    let page = lopdf::dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => lopdf::Object::Array(vec![0.into(), 0.into(), 200.into(), 200.into()]),
        "Annots" => lopdf::Object::Array(vec![sig_field_id.into()]),
    };
    document
        .objects
        .insert(page_id, lopdf::Object::Dictionary(page));

    let pages = lopdf::dictionary! {
        "Type" => "Pages",
        "Kids" => lopdf::Object::Array(vec![page_id.into()]),
        "Count" => 1,
    };
    document
        .objects
        .insert(pages_id, lopdf::Object::Dictionary(pages));

    let acroform = lopdf::dictionary! {
        "Fields" => lopdf::Object::Array(vec![sig_field_id.into()]),
    };
    document
        .objects
        .insert(acroform_id, lopdf::Object::Dictionary(acroform));

    let catalog = lopdf::dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
        "AcroForm" => acroform_id,
    };
    document
        .objects
        .insert(catalog_id, lopdf::Object::Dictionary(catalog));
    document.trailer.set("Root", catalog_id);

    let mut bytes = Vec::new();
    document.save_to(&mut bytes).unwrap();
    bytes
}

pub fn assert_page_numbers(document: &lopdf::Document, expected: &[u32]) {
    let pages = document.get_pages();
    let actual = pages.keys().copied().collect::<Vec<_>>();
    assert_eq!(actual, expected);
}

pub fn page_rotation(document: &lopdf::Document, page_number: u32) -> i64 {
    let page_id = document.get_pages().get(&page_number).copied().unwrap();
    let page = document.get_object(page_id).unwrap().as_dict().unwrap();
    page.get(b"Rotate")
        .and_then(lopdf::Object::as_i64)
        .unwrap_or(0)
}

pub fn page_optional_box(
    document: &lopdf::Document,
    page_number: u32,
    key: &[u8],
) -> Option<[f32; 4]> {
    let page_id = document.get_pages().get(&page_number).copied().unwrap();
    let page = document.get_object(page_id).unwrap().as_dict().unwrap();
    let values = page.get(key).ok()?.as_array().ok()?;
    Some([
        object_to_f32(&values[0]).unwrap(),
        object_to_f32(&values[1]).unwrap(),
        object_to_f32(&values[2]).unwrap(),
        object_to_f32(&values[3]).unwrap(),
    ])
}

pub fn page_box(document: &lopdf::Document, page_number: u32, key: &[u8]) -> [f32; 4] {
    page_optional_box(document, page_number, key).unwrap()
}

pub fn page_content_contains(document: &lopdf::Document, page_number: u32, operator: &str) -> bool {
    let page_id = document.get_pages().get(&page_number).copied().unwrap();
    document
        .get_page_content(page_id)
        .ok()
        .and_then(|content| lopdf::content::Content::decode(&content).ok())
        .is_some_and(|content| {
            content
                .operations
                .iter()
                .any(|operation| operation.operator == operator)
        })
}

pub fn page_rgb_operator(
    document: &lopdf::Document,
    page_number: u32,
    operator: &str,
) -> Option<[f32; 3]> {
    let page_id = document.get_pages().get(&page_number).copied().unwrap();
    let content = document.get_page_content(page_id).ok()?;
    let content = lopdf::content::Content::decode(&content).ok()?;
    content.operations.iter().find_map(|operation| {
        if operation.operator == operator && operation.operands.len() == 3 {
            Some([
                object_to_f32(&operation.operands[0]).ok()?,
                object_to_f32(&operation.operands[1]).ok()?,
                object_to_f32(&operation.operands[2]).ok()?,
            ])
        } else {
            None
        }
    })
}

pub fn pdf_with_blank_and_marked_page() -> Vec<u8> {
    let mut document = lopdf::Document::with_version("1.7");
    let pages_id = document.new_object_id();
    let blank_page_id = document.new_object_id();
    let marked_page_id = document.new_object_id();
    let marked_content_id = document.new_object_id();
    let catalog_id = document.new_object_id();

    let marked_content = lopdf::content::Content {
        operations: vec![lopdf::content::Operation::new("q", vec![])],
    }
    .encode()
    .unwrap();
    document.objects.insert(
        marked_content_id,
        Object::Stream(Stream::new(Dictionary::new(), marked_content)),
    );
    document.objects.insert(
        blank_page_id,
        Object::Dictionary(lopdf::dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => Object::Array(vec![0.into(), 0.into(), 100.into(), 100.into()]),
        }),
    );
    document.objects.insert(
        marked_page_id,
        Object::Dictionary(lopdf::dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => Object::Array(vec![0.into(), 0.into(), 100.into(), 100.into()]),
            "Contents" => marked_content_id,
        }),
    );
    document.objects.insert(
        pages_id,
        Object::Dictionary(lopdf::dictionary! {
            "Type" => "Pages",
            "Kids" => Object::Array(vec![blank_page_id.into(), marked_page_id.into()]),
            "Count" => 2,
        }),
    );
    document.objects.insert(
        catalog_id,
        Object::Dictionary(lopdf::dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        }),
    );
    document.trailer.set("Root", catalog_id);

    let mut bytes = Vec::new();
    document.save_to(&mut bytes).unwrap();
    bytes
}

pub fn pdf_with_blank_page_and_missing_resources() -> Vec<u8> {
    let mut document = lopdf::Document::with_version("1.7");
    let pages_id = document.new_object_id();
    let page_id = document.new_object_id();
    let catalog_id = document.new_object_id();

    document.objects.insert(
        page_id,
        Object::Dictionary(lopdf::dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => Object::Array(vec![0.into(), 0.into(), 100.into(), 100.into()]),
            "Resources" => Object::Reference((99, 0)),
        }),
    );
    document.objects.insert(
        pages_id,
        Object::Dictionary(lopdf::dictionary! {
            "Type" => "Pages",
            "Kids" => Object::Array(vec![page_id.into()]),
            "Count" => 1,
        }),
    );
    document.objects.insert(
        catalog_id,
        Object::Dictionary(lopdf::dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        }),
    );
    document.trailer.set("Root", catalog_id);

    let mut bytes = Vec::new();
    document.save_to(&mut bytes).unwrap();
    bytes
}

pub fn empty_page_pdf() -> Vec<u8> {
    let mut pdf = pdf_writer::Pdf::new();
    let catalog_id = pdf_writer::Ref::new(1);
    let pages_id = pdf_writer::Ref::new(2);
    let page_id = pdf_writer::Ref::new(3);

    pdf.catalog(catalog_id).pages(pages_id);
    pdf.pages(pages_id).kids([page_id]).count(1);
    let mut page = pdf.page(page_id);
    page.media_box(pdf_writer::Rect::new(0.0, 0.0, A4_WIDTH, A4_HEIGHT));
    page.parent(pages_id);
    page.finish();

    pdf.finish()
}

pub fn pdf_with_media_box(width: i64, height: i64) -> Vec<u8> {
    let mut document = lopdf::Document::load_mem(&empty_page_pdf()).unwrap();
    let page_id = *document.get_pages().get(&1).unwrap();
    document
        .get_object_mut(page_id)
        .unwrap()
        .as_dict_mut()
        .unwrap()
        .set(
            "MediaBox",
            Object::Array(vec![0.into(), 0.into(), width.into(), height.into()]),
        );

    let mut bytes = Vec::new();
    document.save_to(&mut bytes).unwrap();
    bytes
}

pub fn pdf_with_unreferenced_stream_object() -> Vec<u8> {
    let mut document = lopdf::Document::load_mem(&empty_page_pdf()).unwrap();
    let unused_id = document.new_object_id();
    document.objects.insert(
        unused_id,
        Object::Stream(Stream::new(Dictionary::new(), b"unused".to_vec())),
    );

    let mut bytes = Vec::new();
    document.save_to(&mut bytes).unwrap();
    bytes
}

pub fn pdf_with_large_plain_content_stream() -> Vec<u8> {
    let content = b"0 0 0 rg\n0 0 100 100 re f\n".repeat(64);
    pdf_with_content_stream(Stream::new(Dictionary::new(), content))
}

pub fn pdf_with_unsupported_filtered_stream() -> Vec<u8> {
    let mut stream = Stream::new(Dictionary::new(), b"not jpeg data".to_vec());
    stream.dict.set("Filter", "DCTDecode");
    pdf_with_content_stream(stream)
}

pub fn pdf_with_content_stream(stream: Stream) -> Vec<u8> {
    let mut document = lopdf::Document::with_version("1.7");
    let pages_id = document.new_object_id();
    let page_id = document.new_object_id();
    let content_id = document.new_object_id();
    let catalog_id = document.new_object_id();

    document.objects.insert(content_id, Object::Stream(stream));
    document.objects.insert(
        page_id,
        Object::Dictionary(lopdf::dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => Object::Array(vec![0.into(), 0.into(), 100.into(), 100.into()]),
            "Contents" => content_id,
        }),
    );
    document.objects.insert(
        pages_id,
        Object::Dictionary(lopdf::dictionary! {
            "Type" => "Pages",
            "Kids" => Object::Array(vec![page_id.into()]),
            "Count" => 1,
        }),
    );
    document.objects.insert(
        catalog_id,
        Object::Dictionary(lopdf::dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        }),
    );
    document.trailer.set("Root", catalog_id);

    let mut bytes = Vec::new();
    document.save_to(&mut bytes).unwrap();
    bytes
}

pub fn pdf_with_duplicate_image_resources() -> Vec<u8> {
    let mut document = lopdf::Document::load_mem(&empty_page_pdf()).unwrap();
    let page_id = *document.get_pages().get(&1).unwrap();
    let left_id = document.add_object(test_image_stream());
    let right_id = document.add_object(test_image_stream());

    document
        .get_object_mut(page_id)
        .unwrap()
        .as_dict_mut()
        .unwrap()
        .set(
            "Resources",
            Object::Dictionary(lopdf::dictionary! {
                "XObject" => Object::Dictionary(lopdf::dictionary! {
                    "Left" => left_id,
                    "Right" => right_id,
                }),
            }),
        );

    let mut bytes = Vec::new();
    document.save_to(&mut bytes).unwrap();
    bytes
}

pub fn test_image_stream() -> Stream {
    Stream::new(
        lopdf::dictionary! {
            "Type" => "XObject",
            "Subtype" => "Image",
            "Width" => 1,
            "Height" => 3,
            "ColorSpace" => "DeviceRGB",
            "BitsPerComponent" => 8,
        },
        b"rgbpixel!".to_vec(),
    )
}

pub fn first_page_content_stream(document: &lopdf::Document) -> &Stream {
    let page_id = *document.get_pages().get(&1).unwrap();
    let content_id = document
        .get_dictionary(page_id)
        .unwrap()
        .get(b"Contents")
        .unwrap()
        .as_reference()
        .unwrap();
    document
        .get_object(content_id)
        .unwrap()
        .as_stream()
        .unwrap()
}

pub fn duplicate_image_resource_ids(
    document: &lopdf::Document,
) -> (lopdf::ObjectId, lopdf::ObjectId) {
    let resources = page_resources(document, 1);
    let xobjects = resources.get(b"XObject").unwrap().as_dict().unwrap();
    (
        xobjects.get(b"Left").unwrap().as_reference().unwrap(),
        xobjects.get(b"Right").unwrap().as_reference().unwrap(),
    )
}

pub fn metadata_entries<const N: usize>(entries: [(&str, &str); N]) -> Vec<MetadataEntry> {
    entries
        .into_iter()
        .map(|(key, value)| MetadataEntry {
            key: key.to_owned(),
            value: value.to_owned(),
        })
        .collect()
}

pub fn pdf_with_text_form_field(readonly: bool) -> Vec<u8> {
    let mut document = lopdf::Document::with_version("1.7");
    let pages_id = document.new_object_id();
    let page_id = document.new_object_id();
    let field_id = document.new_object_id();
    let acroform_id = document.new_object_id();
    let catalog_id = document.new_object_id();
    let flags = if readonly { 1 } else { 0 };

    document.objects.insert(
        field_id,
        Object::Dictionary(lopdf::dictionary! {
            "FT" => "Tx",
            "T" => Object::string_literal("customer"),
            "V" => Object::string_literal(""),
            "Ff" => flags,
            "Rect" => Object::Array(vec![10.into(), 10.into(), 120.into(), 30.into()]),
            "P" => page_id,
        }),
    );
    document.objects.insert(
        page_id,
        Object::Dictionary(lopdf::dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => Object::Array(vec![0.into(), 0.into(), 200.into(), 200.into()]),
            "Annots" => Object::Array(vec![field_id.into()]),
        }),
    );
    document.objects.insert(
        pages_id,
        Object::Dictionary(lopdf::dictionary! {
            "Type" => "Pages",
            "Kids" => Object::Array(vec![page_id.into()]),
            "Count" => 1,
        }),
    );
    document.objects.insert(
        acroform_id,
        Object::Dictionary(lopdf::dictionary! {
            "Fields" => Object::Array(vec![field_id.into()]),
        }),
    );
    document.objects.insert(
        catalog_id,
        Object::Dictionary(lopdf::dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
            "AcroForm" => acroform_id,
        }),
    );
    document.trailer.set("Root", catalog_id);

    let mut bytes = Vec::new();
    document.save_to(&mut bytes).unwrap();
    bytes
}

pub fn pdf_with_named_outline_destination() -> Vec<u8> {
    let mut document = lopdf::Document::load_mem(&empty_page_pdf()).unwrap();
    let outline_id = document.new_object_id();
    let item_id = document.new_object_id();
    document.objects.insert(
        item_id,
        Object::Dictionary(lopdf::dictionary! {
            "Title" => Object::string_literal("Named destination"),
            "Parent" => outline_id,
            "Dest" => Object::Name(b"named-destination".to_vec()),
        }),
    );
    document.objects.insert(
        outline_id,
        Object::Dictionary(lopdf::dictionary! {
            "Type" => "Outlines",
            "First" => item_id,
            "Last" => item_id,
            "Count" => 1,
        }),
    );
    document
        .catalog_mut()
        .unwrap()
        .set("Outlines", Object::Reference(outline_id));

    let mut bytes = Vec::new();
    document.save_to(&mut bytes).unwrap();
    bytes
}

pub fn pdf_with_cyclic_outline() -> Vec<u8> {
    let mut document = lopdf::Document::load_mem(&empty_page_pdf()).unwrap();
    let page_id = *document.get_pages().get(&1).unwrap();
    let outline_id = document.new_object_id();
    let first_id = document.new_object_id();
    let second_id = document.new_object_id();
    let explicit_dest = || {
        Object::Array(vec![
            Object::Reference(page_id),
            Object::Name(b"Fit".to_vec()),
        ])
    };
    document.objects.insert(
        first_id,
        Object::Dictionary(lopdf::dictionary! {
            "Title" => Object::string_literal("First"),
            "Parent" => outline_id,
            "Dest" => explicit_dest(),
            "Next" => Object::Reference(second_id),
        }),
    );
    // `second` points its `Next` back at `first`, forming a sibling cycle that
    // would loop forever without the visited-set guard.
    document.objects.insert(
        second_id,
        Object::Dictionary(lopdf::dictionary! {
            "Title" => Object::string_literal("Second"),
            "Parent" => outline_id,
            "Dest" => explicit_dest(),
            "Next" => Object::Reference(first_id),
        }),
    );
    document.objects.insert(
        outline_id,
        Object::Dictionary(lopdf::dictionary! {
            "Type" => "Outlines",
            "First" => first_id,
            "Last" => second_id,
            "Count" => 2,
        }),
    );
    document
        .catalog_mut()
        .unwrap()
        .set("Outlines", Object::Reference(outline_id));

    let mut bytes = Vec::new();
    document.save_to(&mut bytes).unwrap();
    bytes
}

pub fn pdf_with_checkbox_form_field() -> Vec<u8> {
    let mut document = lopdf::Document::with_version("1.7");
    let pages_id = document.new_object_id();
    let page_id = document.new_object_id();
    let field_id = document.new_object_id();
    let acroform_id = document.new_object_id();
    let catalog_id = document.new_object_id();

    // A checkbox stores its value as a name (e.g. /Yes or /Off), not a string.
    document.objects.insert(
        field_id,
        Object::Dictionary(lopdf::dictionary! {
            "FT" => "Btn",
            "T" => Object::string_literal("subscribe"),
            "V" => Object::Name(b"Yes".to_vec()),
            "Rect" => Object::Array(vec![10.into(), 10.into(), 30.into(), 30.into()]),
            "P" => page_id,
        }),
    );
    document.objects.insert(
        page_id,
        Object::Dictionary(lopdf::dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => Object::Array(vec![0.into(), 0.into(), 200.into(), 200.into()]),
            "Annots" => Object::Array(vec![field_id.into()]),
        }),
    );
    document.objects.insert(
        pages_id,
        Object::Dictionary(lopdf::dictionary! {
            "Type" => "Pages",
            "Kids" => Object::Array(vec![page_id.into()]),
            "Count" => 1,
        }),
    );
    document.objects.insert(
        acroform_id,
        Object::Dictionary(lopdf::dictionary! {
            "Fields" => Object::Array(vec![field_id.into()]),
        }),
    );
    document.objects.insert(
        catalog_id,
        Object::Dictionary(lopdf::dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
            "AcroForm" => acroform_id,
        }),
    );
    document.trailer.set("Root", catalog_id);

    let mut bytes = Vec::new();
    document.save_to(&mut bytes).unwrap();
    bytes
}

pub fn pdf_with_named_info_value() -> Vec<u8> {
    let mut document = lopdf::Document::load_mem(&empty_page_pdf()).unwrap();
    // /Trapped is a name-valued Info entry; a string-only reader rejects it.
    let info_id = document.add_object(Object::Dictionary(lopdf::dictionary! {
        "Title" => Object::string_literal("Sample"),
        "Trapped" => Object::Name(b"True".to_vec()),
    }));
    document.trailer.set("Info", info_id);

    let mut bytes = Vec::new();
    document.save_to(&mut bytes).unwrap();
    bytes
}

pub fn pdf_with_malformed_names_tree() -> Vec<u8> {
    let mut document = lopdf::Document::load_mem(&empty_page_pdf()).unwrap();
    document
        .catalog_mut()
        .unwrap()
        .set("Names", Object::string_literal("malformed names tree"));

    let mut bytes = Vec::new();
    document.save_to(&mut bytes).unwrap();
    bytes
}

pub fn pdf_with_malformed_annotation_array() -> Vec<u8> {
    let mut document = lopdf::Document::load_mem(&empty_page_pdf()).unwrap();
    let page_id = *document.get_pages().get(&1).unwrap();
    document
        .get_object_mut(page_id)
        .unwrap()
        .as_dict_mut()
        .unwrap()
        .set("Annots", Object::string_literal("malformed annotations"));

    let mut bytes = Vec::new();
    document.save_to(&mut bytes).unwrap();
    bytes
}

pub fn pdf_with_malformed_acroform() -> Vec<u8> {
    let mut document = lopdf::Document::load_mem(&empty_page_pdf()).unwrap();
    document
        .catalog_mut()
        .unwrap()
        .set("AcroForm", Object::string_literal("malformed acroform"));

    let mut bytes = Vec::new();
    document.save_to(&mut bytes).unwrap();
    bytes
}

pub fn pdf_with_malformed_xobject_resources() -> Vec<u8> {
    let mut document = lopdf::Document::load_mem(&empty_page_pdf()).unwrap();
    let page_id = *document.get_pages().get(&1).unwrap();
    let page = document
        .get_object_mut(page_id)
        .unwrap()
        .as_dict_mut()
        .unwrap();
    page.set(
        "Resources",
        Object::Dictionary(lopdf::dictionary! {
            "XObject" => Object::string_literal("malformed xobject dictionary"),
        }),
    );

    let mut bytes = Vec::new();
    document.save_to(&mut bytes).unwrap();
    bytes
}

pub fn pdf_with_rgb_fill_content() -> Vec<u8> {
    let mut document = lopdf::Document::with_version("1.7");
    let pages_id = document.new_object_id();
    let page_id = document.new_object_id();
    let content_id = document.new_object_id();
    let catalog_id = document.new_object_id();
    let content = lopdf::content::Content {
        operations: vec![
            lopdf::content::Operation::new(
                "rg",
                vec![Object::Real(1.0), Object::Real(0.0), Object::Real(0.0)],
            ),
            lopdf::content::Operation::new(
                "re",
                vec![
                    Object::Integer(0),
                    Object::Integer(0),
                    Object::Integer(100),
                    Object::Integer(100),
                ],
            ),
            lopdf::content::Operation::new("f", Vec::new()),
        ],
    }
    .encode()
    .unwrap();
    document.objects.insert(
        content_id,
        Object::Stream(lopdf::Stream::new(lopdf::Dictionary::new(), content)),
    );
    document.objects.insert(
        page_id,
        Object::Dictionary(lopdf::dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => Object::Array(vec![0.into(), 0.into(), 100.into(), 100.into()]),
            "Contents" => content_id,
        }),
    );
    document.objects.insert(
        pages_id,
        Object::Dictionary(lopdf::dictionary! {
            "Type" => "Pages",
            "Kids" => Object::Array(vec![page_id.into()]),
            "Count" => 1,
        }),
    );
    document.objects.insert(
        catalog_id,
        Object::Dictionary(lopdf::dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        }),
    );
    document.trailer.set("Root", catalog_id);

    let mut bytes = Vec::new();
    document.save_to(&mut bytes).unwrap();
    bytes
}

pub fn pdf_with_xfa_form() -> Vec<u8> {
    let mut document = lopdf::Document::load_mem(&pdf_with_text_form_field(false)).unwrap();
    let catalog = document.catalog().unwrap();
    let acroform_id = catalog.get(b"AcroForm").unwrap().as_reference().unwrap();
    document
        .get_object_mut(acroform_id)
        .unwrap()
        .as_dict_mut()
        .unwrap()
        .set("XFA", Object::string_literal("xfa packet"));
    let mut bytes = Vec::new();
    document.save_to(&mut bytes).unwrap();
    bytes
}

pub fn page_resources(document: &lopdf::Document, page_number: u32) -> Dictionary {
    let page_id = document.get_pages().get(&page_number).copied().unwrap();
    let resources = document
        .get_dictionary(page_id)
        .unwrap()
        .get(b"Resources")
        .unwrap();
    match resources {
        Object::Dictionary(dictionary) => dictionary.clone(),
        Object::Reference(id) => document.get_dictionary(*id).unwrap().clone(),
        other => panic!("unexpected resources object: {other:?}"),
    }
}

pub fn page_content_contains_operator(
    document: &lopdf::Document,
    page_number: u32,
    operator: &str,
) -> bool {
    let page_id = document.get_pages().get(&page_number).copied().unwrap();
    document
        .get_page_contents(page_id)
        .into_iter()
        .filter_map(|content_id| document.get_object(content_id).ok())
        .filter_map(|object| object.as_stream().ok())
        .filter_map(|stream| lopdf::content::Content::decode(&stream.content).ok())
        .flat_map(|content| content.operations)
        .any(|operation| operation.operator == operator)
}

pub fn page_xobject_subtypes(document: &lopdf::Document, page_number: u32) -> Vec<Vec<u8>> {
    let resources = page_resources(document, page_number);
    let Ok(xobjects) = resources.get(b"XObject").and_then(Object::as_dict) else {
        return Vec::new();
    };
    xobjects
        .iter()
        .filter_map(|(_, object)| object.as_reference().ok())
        .filter_map(|id| document.get_object(id).ok())
        .filter_map(|object| object.as_stream().ok())
        .filter_map(|stream| stream.dict.get(b"Subtype").and_then(Object::as_name).ok())
        .map(|name| name.to_vec())
        .collect()
}

pub fn page_xobject_count(document: &lopdf::Document, page_number: u32) -> usize {
    let resources = page_resources(document, page_number);
    resources
        .get(b"XObject")
        .and_then(Object::as_dict)
        .map(|dictionary| dictionary.len())
        .unwrap_or(0)
}

pub fn page_content_text_contains(
    document: &lopdf::Document,
    page_number: u32,
    expected: &str,
) -> bool {
    let page_id = document.get_pages().get(&page_number).copied().unwrap();
    String::from_utf8_lossy(&document.get_page_content(page_id).unwrap()).contains(expected)
}

pub fn page_form_xobject_operators(document: &lopdf::Document, page_number: u32) -> Vec<String> {
    let resources = page_resources(document, page_number);
    let Ok(xobjects) = resources.get(b"XObject").and_then(Object::as_dict) else {
        return Vec::new();
    };
    let mut operators = Vec::new();
    let mut seen = BTreeSet::new();
    for (_, object) in xobjects.iter() {
        if let Ok(id) = object.as_reference() {
            collect_form_xobject_operators(document, id, &mut seen, &mut operators);
        }
    }
    operators
}

fn collect_form_xobject_operators(
    document: &lopdf::Document,
    object_id: lopdf::ObjectId,
    seen: &mut BTreeSet<lopdf::ObjectId>,
    operators: &mut Vec<String>,
) {
    if !seen.insert(object_id) {
        return;
    }
    let Ok(stream) = document
        .get_object(object_id)
        .and_then(lopdf::Object::as_stream)
    else {
        return;
    };
    if stream
        .dict
        .get(b"Subtype")
        .and_then(lopdf::Object::as_name)
        .ok()
        != Some(b"Form".as_slice())
    {
        return;
    }
    if let Ok(content) = stream.get_plain_content() {
        if let Ok(content) = lopdf::content::Content::decode(&content) {
            operators.extend(
                content
                    .operations
                    .into_iter()
                    .map(|operation| operation.operator),
            );
        }
    }
    let Ok(resources) = stream.dict.get(b"Resources").and_then(Object::as_dict) else {
        return;
    };
    let Ok(xobjects) = resources.get(b"XObject").and_then(Object::as_dict) else {
        return;
    };
    for (_, object) in xobjects.iter() {
        if let Ok(id) = object.as_reference() {
            collect_form_xobject_operators(document, id, seen, operators);
        }
    }
}

pub fn simple_svg() -> &'static [u8] {
    br##"<svg xmlns="http://www.w3.org/2000/svg" width="120" height="80">
            <rect x="10" y="10" width="100" height="60" fill="#16a34a"/>
        </svg>"##
}

#[derive(Default)]
pub struct RecordingRunner {
    executed: std::sync::Mutex<Vec<String>>,
    fail_on: Option<&'static str>,
    error: std::sync::Mutex<Option<OxideError>>,
}

impl RecordingRunner {
    pub fn with_failure(fail_on: &'static str, error: OxideError) -> Self {
        Self {
            executed: std::sync::Mutex::new(Vec::new()),
            fail_on: Some(fail_on),
            error: std::sync::Mutex::new(Some(error)),
        }
    }

    pub fn executed(&self) -> Vec<String> {
        self.executed.lock().unwrap().clone()
    }
}

impl OperatorRunner for RecordingRunner {
    fn run(&self, task: &TaskSpec, _inputs: &[Artifact]) -> Result<Artifact, OxideError> {
        self.executed
            .lock()
            .unwrap()
            .push(task.id.as_str().to_owned());
        if self.fail_on == Some(task.id.as_str()) {
            return Err(self.error.lock().unwrap().take().unwrap());
        }

        Ok(Artifact::bytes(task.id.as_str().as_bytes()))
    }
}

pub struct SlowRunner;

impl OperatorRunner for SlowRunner {
    fn run(&self, _task: &TaskSpec, _inputs: &[Artifact]) -> Result<Artifact, OxideError> {
        std::thread::sleep(std::time::Duration::from_millis(5));
        Ok(Artifact::bytes(b"finished"))
    }
}
