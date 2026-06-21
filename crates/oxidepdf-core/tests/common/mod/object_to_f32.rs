use der::{Decode, Encode, EncodePem, pem::LineEnding};
use lopdf::{Dictionary, Object, Stream, dictionary};
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
    std::fs::write(&path, test_trust_anchor_pem()).unwrap();
    path.canonicalize().unwrap()
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

fn test_trust_anchor_pem() -> String {
    let signing_key = p256::ecdsa::SigningKey::from_bytes((&[9u8; 32]).into()).unwrap();
    let verifying_key = *signing_key.verifying_key();
    let public_key = spki::SubjectPublicKeyInfoOwned::from_key(verifying_key).unwrap();
    let subject = x509_cert::name::Name::from_str("CN=OxidePDF Test Trust Anchor,O=OxidePDF,C=US")
        .unwrap()
        .to_der()
        .unwrap();
    let subject = x509_cert::name::Name::from_der(&subject).unwrap();
    let validity = x509_cert::time::Validity::from_now(Duration::from_secs(60 * 60)).unwrap();
    let serial_number = x509_cert::serial_number::SerialNumber::from(99u32);
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

    certificate.to_pem(LineEnding::LF).unwrap()
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
