use der::{Decode, Encode, EncodePem, pem::LineEnding};
use lopdf::dictionary;
use oxidepdf_core::{Artifact, ImageToPdfOptions};
use p256::pkcs8::EncodePrivateKey;
use std::fs;
use std::path::Path;
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use x509_cert::builder::Builder;

static FIXTURE_COUNTER: AtomicU64 = AtomicU64::new(0);

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
    write_fixture_file("test.pdf", generated_fixture_pdf())
}

pub fn fixture_jpg() -> std::path::PathBuf {
    write_fixture_file("test.jpg", generated_fixture_jpg())
}

pub fn fixture_signature_pdf() -> std::path::PathBuf {
    write_fixture_file("signature-placeholder.pdf", generated_signature_pdf())
}

pub fn write_test_trust_anchors(dir: &Path) -> std::path::PathBuf {
    let path = dir.join("anchors.pem");
    fs::write(&path, test_trust_anchor_pem()).unwrap();
    path.canonicalize().unwrap()
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
        &[Artifact::image(fixture_jpg_bytes()).unwrap()],
        &ImageToPdfOptions::default(),
        &Default::default(),
    )
    .unwrap()
    .bytes
    .to_vec()
}

pub fn fixture_jpg_bytes() -> Vec<u8> {
    generated_fixture_jpg()
}

pub fn fixture_pdf_bytes() -> Vec<u8> {
    generated_fixture_pdf()
}

fn write_fixture_file(name: &str, bytes: Vec<u8>) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("oxidepdf_cli_fixtures_{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let counter = FIXTURE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = dir.join(format!("{counter}_{name}"));
    fs::write(&path, bytes).unwrap();
    path.canonicalize().unwrap()
}
