
#[test]
fn verify_pdf_signatures_reports_malformed_byte_range_as_invalid() {
    let pdf = pdf_with_signature_dictionary(vec![0, 64, 32, 64], vec![0x30, 0x82]);
    let trust_anchors = write_test_trust_anchors("malformed_byte_range_report");

    let report = verify_pdf_signatures(
        &pdf,
        &SignatureOptions {
            mode: SignatureMode::Verify,
            trust_anchors: Some(trust_anchors),
        },
        &ResourceLimits::default(),
    )
    .unwrap();
    let report: SignatureVerificationReport = serde_json::from_str(&report.text).unwrap();

    assert_eq!(report.verdict, SignatureVerdict::Invalid);
    assert_eq!(report.signatures.len(), 1);
    assert!(
        report.signatures[0]
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "byte_range_not_ordered")
    );
}

#[test]
fn verify_pdf_signatures_reports_unknown_subfilter_as_unsupported() {
    let pdf = pdf_with_signature_dictionary_and_subfilter(
        vec![0, 64, 192, 64],
        vec![0x30, 0x82],
        "adbe.x509.rsa_sha1",
    );
    let trust_anchors = write_test_trust_anchors("unknown_subfilter_report");

    let report = verify_pdf_signatures(
        &pdf,
        &SignatureOptions {
            mode: SignatureMode::Verify,
            trust_anchors: Some(trust_anchors),
        },
        &ResourceLimits::default(),
    )
    .unwrap();
    let report: SignatureVerificationReport = serde_json::from_str(&report.text).unwrap();

    assert_eq!(report.verdict, SignatureVerdict::Unsupported);
    assert_eq!(report.signatures.len(), 1);
    assert!(
        report.signatures[0]
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "unsupported_subfilter")
    );
}

#[test]
fn signature_research_scanner_finds_signature_markers() {
    let pdf = fixture_signature_pdf();

    let report = inspect_pdf_signature_markers_for_research(pdf).unwrap();

    assert_eq!(report.signature_dictionary_count, 1);
    assert_eq!(report.subfilters, vec!["adbe.pkcs7.detached"]);
    assert_eq!(report.byte_ranges.len(), 1);
    assert_eq!(report.byte_ranges[0].first_start, 0);
    assert_eq!(report.byte_ranges[0].first_len, 64);
    assert_eq!(report.byte_ranges[0].second_start, 192);
    assert_eq!(report.byte_ranges[0].second_len, 64);
    assert!(report.byte_ranges[0].in_bounds);
    assert!(report.byte_ranges[0].ordered_non_overlapping);
    assert_eq!(report.byte_ranges[0].gap_len, Some(128));
    assert_eq!(report.byte_ranges[0].covered_len, Some(128));
}

#[test]
fn signature_research_scanner_reports_out_of_bounds_byte_range() {
    let pdf = b"%PDF-1.7\n1 0 obj\n<< /Type /Sig /SubFilter /ETSI.CAdES.detached /ByteRange [0 5 999 10] >>\nendobj\n%%EOF";

    let report = inspect_pdf_signature_markers_for_research(pdf).unwrap();

    assert_eq!(report.signature_dictionary_count, 1);
    assert_eq!(report.subfilters, vec!["ETSI.CAdES.detached"]);
    assert_eq!(report.byte_ranges.len(), 1);
    assert!(!report.byte_ranges[0].in_bounds);
    assert!(report.byte_ranges[0].ordered_non_overlapping);
    assert_eq!(report.byte_ranges[0].covered_len, Some(15));
}

#[test]
fn signature_research_scanner_ignores_malformed_byte_range() {
    let pdf = b"%PDF-1.7\n1 0 obj\n<< /Type /Sig /ByteRange [0 nope 10 5] >>\nendobj\n%%EOF";

    let report = inspect_pdf_signature_markers_for_research(pdf).unwrap();

    assert_eq!(report.signature_dictionary_count, 1);
    assert!(report.byte_ranges.is_empty());
}

#[test]
fn signature_research_scanner_rejects_non_pdf_magic_bytes() {
    let err = inspect_pdf_signature_markers_for_research(b"not a pdf").unwrap_err();

    assert!(matches!(err, OxideError::InvalidInput { .. }));
    assert!(err.to_string().contains("expected PDF"));
}
