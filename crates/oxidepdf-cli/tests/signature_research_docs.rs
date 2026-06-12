use std::fs;
use std::path::PathBuf;

#[test]
fn signature_adr_records_pure_rust_route_and_unsupported_boundary() {
    let adr = read("docs/decisions/ADR-001-pdf-signatures-and-certificates.md");

    assert!(adr.contains("UnsupportedPdfFeature"));
    assert!(adr.contains("unsupported_pdf_feature"));
    assert!(adr.contains("OpenSSL"));
    assert!(adr.contains("system-native"));
    assert!(adr.contains("Signature discovery and structural checks"));
    assert!(adr.contains("CMS parsing and digest validation"));
    assert!(adr.contains("Certificate extraction and chain validation"));
    assert!(adr.contains("Timestamp validation"));
    assert!(adr.contains("PAdES policy and signing"));
}

#[test]
fn signature_adr_records_candidate_crates_and_approved_licenses() {
    let adr = read("docs/decisions/ADR-001-pdf-signatures-and-certificates.md");

    for crate_name in [
        "x509-cert",
        "cms",
        "der",
        "spki",
        "const-oid",
        "signature",
        "rustls-webpki",
        "time",
    ] {
        assert!(adr.contains(crate_name), "ADR missing {crate_name}");
    }

    assert!(adr.contains("Apache-2.0 OR MIT"));
    assert!(adr.contains("ISC"));
    assert!(adr.contains("GPL, AGPL, LGPL, or unknown licensing"));
}

#[test]
fn signature_fixture_is_research_only_and_contains_pdf_markers() {
    let fixture = read("tests/fixtures/signature-placeholder.pdf");

    assert!(fixture.starts_with("%PDF-"));
    assert!(fixture.contains("research fixture only"));
    assert!(fixture.contains("/Type /Sig"));
    assert!(fixture.contains("/SubFilter /adbe.pkcs7.detached"));
    assert!(fixture.contains("/ByteRange [0 64 192 64]"));
}

fn read(path: &str) -> String {
    fs::read_to_string(repo_root().join(path))
        .unwrap_or_else(|error| panic!("failed to read {path}: {error}"))
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}
