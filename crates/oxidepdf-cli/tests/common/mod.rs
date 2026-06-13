//! Shared fixtures and assertions for the `oxidepdf-cli` integration tests.
//!
//! Built on the crate's public API plus the same external crates the original
//! in-crate tests used. Split out of the former `src/lib.rs` test module.

#![allow(dead_code)]

use der::{pem::LineEnding, Decode, Encode, EncodePem};
use lopdf::dictionary;
use oxidepdf_core::{Artifact, ImageToPdfOptions};
use p256::pkcs8::EncodePrivateKey;
use std::fs;
use std::path::Path;
use std::str::FromStr;
use std::time::Duration;
use x509_cert::builder::Builder;

pub fn temp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("oxidepdf_cli_{}_{}", name, std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

pub fn yaml_path(path: impl AsRef<std::path::Path>) -> String {
    path.as_ref().display().to_string()
}

pub fn fixture_pdf() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/test.pdf")
        .canonicalize()
        .unwrap()
}

pub fn fixture_jpg() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/test.jpg")
        .canonicalize()
        .unwrap()
}

pub fn fixture_signature_pdf() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/signature-placeholder.pdf")
        .canonicalize()
        .unwrap()
}

pub fn write_test_trust_anchors(dir: &Path) -> std::path::PathBuf {
    let path = dir.join("anchors.pem");
    fs::write(
        &path,
        include_bytes!("../../../../tests/fixtures/test-trust-anchor.txt"),
    )
    .unwrap();
    path
}

pub fn write_p256_signing_material(dir: &Path) -> (std::path::PathBuf, std::path::PathBuf) {
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

    fs::write(&private_key_path, private_key_pem).unwrap();
    fs::write(&certificate_path, certificate_pem).unwrap();
    (certificate_path, private_key_path)
}

pub fn write_signature_pdf(dir: &Path) -> std::path::PathBuf {
    let path = dir.join("signed.pdf");
    let mut document = lopdf::Document::with_version("1.7");
    let pages_id = document.new_object_id();
    let page_id = document.new_object_id();
    let sig_field_id = document.new_object_id();
    let sig_value_id = document.new_object_id();
    let acroform_id = document.new_object_id();
    let catalog_id = document.new_object_id();

    let sig_value = lopdf::dictionary! {
        "Type" => "Sig",
        "Filter" => "Adobe.PPKLite",
        "SubFilter" => "adbe.pkcs7.detached",
        "ByteRange" => lopdf::Object::Array(vec![0.into(), 64.into(), 192.into(), 64.into()]),
        "Contents" => lopdf::Object::String(vec![0x30, 0x82], lopdf::StringFormat::Hexadecimal),
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

    document.save(&path).unwrap();
    path
}

pub fn simple_svg() -> &'static [u8] {
    br##"<svg xmlns="http://www.w3.org/2000/svg" width="120" height="80">
        <rect x="10" y="10" width="100" height="60" fill="#2563eb"/>
    </svg>"##
}

pub fn image_only_pdf() -> Vec<u8> {
    oxidepdf_core::image_artifacts_to_pdf(
        &[Artifact::image(fixture_jpg_bytes())],
        &ImageToPdfOptions::default(),
        &Default::default(),
    )
    .unwrap()
    .bytes
    .to_vec()
}

pub fn fixture_jpg_bytes() -> Vec<u8> {
    fs::read(fixture_jpg()).unwrap()
}

pub fn fixture_pdf_bytes() -> Vec<u8> {
    fs::read(fixture_pdf()).unwrap()
}

pub fn pdf_page_count(path: &std::path::Path) -> usize {
    lopdf::Document::load(path).unwrap().get_pages().len()
}

pub fn pdf_page_rotation(path: &std::path::Path, page_number: u32) -> i64 {
    let document = lopdf::Document::load(path).unwrap();
    let page_id = document.get_pages().get(&page_number).copied().unwrap();
    let page = document.get_object(page_id).unwrap().as_dict().unwrap();
    page.get(b"Rotate")
        .and_then(lopdf::Object::as_i64)
        .unwrap_or(0)
}

pub fn pdf_page_box(path: &std::path::Path, page_number: u32, key: &[u8]) -> [f32; 4] {
    let document = lopdf::Document::load(path).unwrap();
    let page_id = document.get_pages().get(&page_number).copied().unwrap();
    let page = document.get_object(page_id).unwrap().as_dict().unwrap();
    let values = page.get(key).unwrap().as_array().unwrap();
    [
        pdf_object_to_f32(&values[0]),
        pdf_object_to_f32(&values[1]),
        pdf_object_to_f32(&values[2]),
        pdf_object_to_f32(&values[3]),
    ]
}

pub fn pdf_object_to_f32(object: &lopdf::Object) -> f32 {
    match object {
        lopdf::Object::Integer(value) => *value as f32,
        lopdf::Object::Real(value) => *value,
        other => panic!("unexpected page box value: {other:?}"),
    }
}

pub fn pdf_page_xobject_count(path: &std::path::Path, page_number: u32) -> usize {
    let document = lopdf::Document::load(path).unwrap();
    let page_id = document.get_pages().get(&page_number).copied().unwrap();
    let page = document.get_object(page_id).unwrap().as_dict().unwrap();
    let resources = page.get(b"Resources").unwrap().as_dict().unwrap();
    resources
        .get(b"XObject")
        .and_then(lopdf::Object::as_dict)
        .map(|dictionary| dictionary.len())
        .unwrap_or(0)
}

pub fn pdf_page_content_contains(path: &std::path::Path, page_number: u32, expected: &str) -> bool {
    let document = lopdf::Document::load(path).unwrap();
    let page_id = document.get_pages().get(&page_number).copied().unwrap();
    String::from_utf8_lossy(&document.get_page_content(page_id).unwrap()).contains(expected)
}

pub fn pdf_rgb_operator(
    path: &std::path::Path,
    page_number: u32,
    operator: &str,
) -> Option<[f32; 3]> {
    let document = lopdf::Document::load(path).unwrap();
    let page_id = document.get_pages().get(&page_number).copied().unwrap();
    let content = document.get_page_content(page_id).ok()?;
    let content = lopdf::content::Content::decode(&content).ok()?;
    content.operations.iter().find_map(|operation| {
        if operation.operator == operator && operation.operands.len() == 3 {
            Some([
                pdf_object_to_f32(&operation.operands[0]),
                pdf_object_to_f32(&operation.operands[1]),
                pdf_object_to_f32(&operation.operands[2]),
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
        lopdf::Object::Stream(lopdf::Stream::new(lopdf::Dictionary::new(), marked_content)),
    );
    document.objects.insert(
        blank_page_id,
        lopdf::Object::Dictionary(lopdf::dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => lopdf::Object::Array(vec![0.into(), 0.into(), 100.into(), 100.into()]),
        }),
    );
    document.objects.insert(
        marked_page_id,
        lopdf::Object::Dictionary(lopdf::dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => lopdf::Object::Array(vec![0.into(), 0.into(), 100.into(), 100.into()]),
            "Contents" => marked_content_id,
        }),
    );
    document.objects.insert(
        pages_id,
        lopdf::Object::Dictionary(lopdf::dictionary! {
            "Type" => "Pages",
            "Kids" => lopdf::Object::Array(vec![blank_page_id.into(), marked_page_id.into()]),
            "Count" => 2,
        }),
    );
    document.objects.insert(
        catalog_id,
        lopdf::Object::Dictionary(lopdf::dictionary! {
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
    let mut document = lopdf::Document::with_version("1.7");
    let pages_id = document.new_object_id();
    let page_id = document.new_object_id();
    let catalog_id = document.new_object_id();
    document.objects.insert(
        page_id,
        lopdf::Object::Dictionary(lopdf::dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => lopdf::Object::Array(vec![0.into(), 0.into(), 595.into(), 842.into()]),
        }),
    );
    document.objects.insert(
        pages_id,
        lopdf::Object::Dictionary(lopdf::dictionary! {
            "Type" => "Pages",
            "Kids" => lopdf::Object::Array(vec![page_id.into()]),
            "Count" => 1,
        }),
    );
    document.objects.insert(
        catalog_id,
        lopdf::Object::Dictionary(lopdf::dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        }),
    );
    document.trailer.set("Root", catalog_id);

    let mut bytes = Vec::new();
    document.save_to(&mut bytes).unwrap();
    bytes
}

pub fn form_pdf(readonly: bool) -> Vec<u8> {
    let mut document = lopdf::Document::with_version("1.7");
    let pages_id = document.new_object_id();
    let page_id = document.new_object_id();
    let field_id = document.new_object_id();
    let acroform_id = document.new_object_id();
    let catalog_id = document.new_object_id();
    let flags = if readonly { 1 } else { 0 };

    document.objects.insert(
        field_id,
        lopdf::Object::Dictionary(lopdf::dictionary! {
            "FT" => "Tx",
            "T" => lopdf::Object::string_literal("customer"),
            "V" => lopdf::Object::string_literal(""),
            "Ff" => flags,
            "Rect" => lopdf::Object::Array(vec![10.into(), 10.into(), 120.into(), 30.into()]),
            "P" => page_id,
        }),
    );
    document.objects.insert(
        page_id,
        lopdf::Object::Dictionary(lopdf::dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => lopdf::Object::Array(vec![0.into(), 0.into(), 200.into(), 200.into()]),
            "Annots" => lopdf::Object::Array(vec![field_id.into()]),
        }),
    );
    document.objects.insert(
        pages_id,
        lopdf::Object::Dictionary(lopdf::dictionary! {
            "Type" => "Pages",
            "Kids" => lopdf::Object::Array(vec![page_id.into()]),
            "Count" => 1,
        }),
    );
    document.objects.insert(
        acroform_id,
        lopdf::Object::Dictionary(lopdf::dictionary! {
            "Fields" => lopdf::Object::Array(vec![field_id.into()]),
        }),
    );
    document.objects.insert(
        catalog_id,
        lopdf::Object::Dictionary(lopdf::dictionary! {
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
                vec![
                    lopdf::Object::Real(1.0),
                    lopdf::Object::Real(0.0),
                    lopdf::Object::Real(0.0),
                ],
            ),
            lopdf::content::Operation::new("re", vec![0.into(), 0.into(), 100.into(), 100.into()]),
            lopdf::content::Operation::new("f", Vec::new()),
        ],
    }
    .encode()
    .unwrap();
    document.objects.insert(
        content_id,
        lopdf::Object::Stream(lopdf::Stream::new(lopdf::Dictionary::new(), content)),
    );
    document.objects.insert(
        page_id,
        lopdf::Object::Dictionary(lopdf::dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => lopdf::Object::Array(vec![0.into(), 0.into(), 100.into(), 100.into()]),
            "Contents" => content_id,
        }),
    );
    document.objects.insert(
        pages_id,
        lopdf::Object::Dictionary(lopdf::dictionary! {
            "Type" => "Pages",
            "Kids" => lopdf::Object::Array(vec![page_id.into()]),
            "Count" => 1,
        }),
    );
    document.objects.insert(
        catalog_id,
        lopdf::Object::Dictionary(lopdf::dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        }),
    );
    document.trailer.set("Root", catalog_id);

    let mut bytes = Vec::new();
    document.save_to(&mut bytes).unwrap();
    bytes
}

pub fn page_has_content_operator(path: &std::path::Path, page_number: u32, operator: &str) -> bool {
    let document = lopdf::Document::load(path).unwrap();
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
