#[test]
fn verify_pdf_signatures_rejects_appended_bytes_after_signed_ranges() {
    // A genuinely signed PDF that verifies as trusted.
    let pdf = fixture_pdf();
    let (certificate_path, private_key_path) = write_p256_signing_material("wrapping_attack");

    let signed_pdf = add_pdf_signature(
        pdf,
        &SignatureAddOptions {
            field_name: "Approval".to_owned(),
            certificate: certificate_path.clone(),
            private_key: private_key_path,
            contents_reserved_bytes: Some(16_384),
            appearance_field: None,
        },
        &ResourceLimits::default(),
    )
    .unwrap();

    // Sanity: the unmodified signed PDF must pass digest/signature/chain checks.
    let baseline = verify_pdf_signatures(
        &signed_pdf,
        &SignatureOptions {
            mode: SignatureMode::Verify,
            trust_anchors: Some(certificate_path.clone()),
        },
        &ResourceLimits::default(),
    )
    .unwrap();
    let baseline: SignatureVerificationReport = serde_json::from_str(&baseline.text).unwrap();
    assert!(
        baseline.signatures[0].byte_range.covers_whole_input,
        "freshly signed PDF must report full ByteRange coverage"
    );

    // Append arbitrary unsigned bytes after the signed ranges. The CMS still
    // validates over the original signed bytes, but the document now contains
    // content outside the ByteRange. This must NOT be reported as trusted.
    let mut tampered = signed_pdf.clone();
    tampered.extend_from_slice(b"\n% malicious appended incremental update\n");

    let report = verify_pdf_signatures(
        &tampered,
        &SignatureOptions {
            mode: SignatureMode::Verify,
            trust_anchors: Some(certificate_path),
        },
        &ResourceLimits::default(),
    )
    .unwrap();
    let report: SignatureVerificationReport = serde_json::from_str(&report.text).unwrap();

    assert_ne!(
        report.verdict,
        SignatureVerdict::Trusted,
        "appended unsigned bytes must not yield a trusted verdict"
    );
    assert_eq!(report.signatures.len(), 1);
    assert!(!report.signatures[0].byte_range.covers_whole_input);
    assert!(
        report.signatures[0]
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "byte_range_not_full_coverage"),
        "expected a full-coverage diagnostic, got: {:?}",
        report.signatures[0].diagnostics
    );
}
