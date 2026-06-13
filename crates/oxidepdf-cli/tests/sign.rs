//! Integration tests for the `sign` CLI surface.
//!
//! Split out of the former monolithic `src/lib.rs` test module. These drive the
//! CLI through its public `run_with_io`/`command` entry points only.

mod common;
#[allow(unused_imports)]
use common::*;
#[allow(unused_imports)]
use oxidepdf_cli::{command, run, run_with_io};
use std::fs;

#[test]
fn verify_signatures_command_writes_json_report() {
    let dir = temp_dir("verify_signatures_command_writes_json_report");
    let input = write_signature_pdf(&dir);
    let output = dir.join("signature-report.json");
    let trust_anchors = write_test_trust_anchors(&dir);
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let code = run_with_io(
        [
            "oxidepdf",
            "pdf_sign",
            "verify",
            input.to_str().unwrap(),
            "--trust-anchors",
            trust_anchors.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
        ],
        [],
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(code, 0);
    assert_eq!(stdout, b"");
    assert_eq!(stderr, b"");
    let report: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(output).unwrap()).unwrap();
    assert_eq!(report["verdict"], "invalid");
    assert_eq!(report["trust_anchor_count"], 1);
    assert_eq!(report["signatures"][0]["field_name"], "Approval");
    assert_eq!(
        report["signatures"][0]["revocation_status"]["status"],
        "indeterminate"
    );
}

#[test]
fn sign_list_command_writes_json_report_without_trust_anchors() {
    let dir = temp_dir("sign_list_command_writes_json_report");
    let input = write_signature_pdf(&dir);
    let output = dir.join("signature-list.json");
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let code = run_with_io(
        [
            "oxidepdf",
            "pdf_sign",
            "list",
            input.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
        ],
        [],
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(code, 0);
    assert_eq!(stdout, b"");
    assert_eq!(stderr, b"");
    let report: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(output).unwrap()).unwrap();
    assert_eq!(report["signatures"][0]["field_name"], "Approval");
    assert_eq!(report["signatures"][0]["subfilter"], "adbe.pkcs7.detached");
}

#[test]
fn verify_signatures_command_without_trust_anchors_is_not_trusted() {
    let dir = temp_dir("verify_signatures_command_without_trust_anchors");
    let input = write_signature_pdf(&dir);
    let output = dir.join("signature-report.json");
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let code = run_with_io(
        [
            "oxidepdf",
            "pdf_sign",
            "verify",
            input.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
        ],
        [],
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(code, 0);
    assert_eq!(stdout, b"");
    assert_eq!(stderr, b"");
    let report: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(output).unwrap()).unwrap();
    assert_ne!(report["verdict"], "trusted");
    assert_eq!(report["trust_anchor_count"], 0);
}

#[test]
fn sign_delete_field_command_requires_destructive_for_signed_field() {
    let dir = temp_dir("sign_delete_field_command_requires_destructive");
    let input = write_signature_pdf(&dir);
    let output = dir.join("deleted.pdf");
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let code = run_with_io(
        [
            "oxidepdf",
            "pdf_sign",
            "delete-field",
            input.to_str().unwrap(),
            "--field-name",
            "Approval",
            "-o",
            output.to_str().unwrap(),
        ],
        [],
        &mut stdout,
        &mut stderr,
    );
    let stderr = String::from_utf8(stderr).unwrap();

    assert_eq!(code, 3);
    assert_eq!(stdout, b"");
    assert!(stderr.contains("signed value material"));
    assert!(!output.exists());
}

#[test]
fn sign_delete_field_command_deletes_when_destructive_is_explicit() {
    let dir = temp_dir("sign_delete_field_command_deletes");
    let input = write_signature_pdf(&dir);
    let output = dir.join("deleted.pdf");
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let code = run_with_io(
        [
            "oxidepdf",
            "pdf_sign",
            "delete-field",
            input.to_str().unwrap(),
            "--field-name",
            "Approval",
            "--destructive",
            "-o",
            output.to_str().unwrap(),
        ],
        [],
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(code, 0);
    assert_eq!(stdout, b"");
    assert_eq!(stderr, b"");
    let report_output = dir.join("report.json");
    let code = run_with_io(
        [
            "oxidepdf",
            "pdf_sign",
            "verify",
            output.to_str().unwrap(),
            "-o",
            report_output.to_str().unwrap(),
        ],
        [],
        &mut Vec::new(),
        &mut Vec::new(),
    );
    assert_eq!(code, 0);
    let report: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(report_output).unwrap()).unwrap();
    assert_eq!(report["verdict"], "indeterminate");
    assert!(report["signatures"].as_array().unwrap().is_empty());
}

#[test]
fn timestamp_add_command_requires_token_or_tsa_url() {
    let dir = temp_dir("timestamp_add_command_requires_source");
    let input = write_signature_pdf(&dir);
    let output = dir.join("timestamp-report.json");
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let code = run_with_io(
        [
            "oxidepdf",
            "pdf_sign",
            "timestamp",
            input.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
        ],
        [],
        &mut stdout,
        &mut stderr,
    );
    let stderr = String::from_utf8(stderr).unwrap();

    assert_eq!(code, 3);
    assert_eq!(stdout, b"");
    assert!(stderr.contains("exactly one of tsa_url or token"));
}

#[test]
fn sign_add_command_writes_signed_pdf() {
    let dir = temp_dir("sign_add_command_writes_signed_pdf");
    let input = fixture_pdf();
    let output = dir.join("new-signed.pdf");
    let (cert, key) = write_p256_signing_material(&dir);
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let code = run_with_io(
        [
            "oxidepdf",
            "pdf_sign",
            "add",
            input.to_str().unwrap(),
            "--field-name",
            "Approval",
            "--certificate",
            cert.to_str().unwrap(),
            "--private-key",
            key.to_str().unwrap(),
            "--contents-reserved-bytes",
            "16384",
            "-o",
            output.to_str().unwrap(),
        ],
        [],
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(code, 0);
    assert_eq!(stdout, b"");
    assert_eq!(stderr, b"");
    let report_output = dir.join("signature-report.json");
    let code = run_with_io(
        [
            "oxidepdf",
            "pdf_sign",
            "verify",
            output.to_str().unwrap(),
            "--trust-anchors",
            cert.to_str().unwrap(),
            "-o",
            report_output.to_str().unwrap(),
        ],
        [],
        &mut Vec::new(),
        &mut Vec::new(),
    );
    assert_eq!(code, 0);
    let report: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(report_output).unwrap()).unwrap();
    assert_eq!(report["signatures"][0]["digest_status"]["status"], "passed");
    assert_eq!(
        report["signatures"][0]["signature_status"]["status"],
        "passed"
    );
    assert_eq!(
        report["signatures"][0]["certificate_chain_status"]["status"],
        "passed"
    );
}

#[test]
fn workflow_signature_operator_writes_json_report() {
    let dir = temp_dir("workflow_signature_operator_writes_json_report");
    let input = write_signature_pdf(&dir);
    let workflow = dir.join("workflow.yaml");
    let output = dir.join("signature-report.json");
    let trust_anchors = write_test_trust_anchors(&dir);
    fs::write(
        &workflow,
        format!(
            r#"
            version: 1
            inputs:
              - id: source
                path: {}
            tasks:
              - id: verify
                op:
                  pdf_sign:
                    verify:
                      mode: verify
                      trust_anchors: {}
                inputs: [source]
            outputs:
              - id: final
                from: verify
                path: {}
            "#,
            yaml_path(&input),
            yaml_path(&trust_anchors),
            yaml_path(&output)
        ),
    )
    .unwrap();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let code = run_with_io(
        ["oxidepdf", "run", "--workflow", workflow.to_str().unwrap()],
        [],
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(code, 0);
    assert_eq!(stdout, b"");
    assert_eq!(stderr, b"");
    let report: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(output).unwrap()).unwrap();
    assert_eq!(report["verdict"], "invalid");
    assert_eq!(report["trust_anchor_count"], 1);
}

#[test]
fn workflow_signature_operator_without_trust_anchors_is_not_trusted() {
    let dir = temp_dir("workflow_signature_operator_without_trust_anchors");
    let input = write_signature_pdf(&dir);
    let workflow = dir.join("workflow.yaml");
    let output = dir.join("signature-report.json");
    fs::write(
        &workflow,
        format!(
            r#"
            version: 1
            inputs:
              - id: source
                path: {}
            tasks:
              - id: verify
                op:
                  pdf_sign:
                    verify:
                      mode: verify
                inputs: [source]
            outputs:
              - id: final
                from: verify
                path: {}
            "#,
            yaml_path(&input),
            yaml_path(&output)
        ),
    )
    .unwrap();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let code = run_with_io(
        ["oxidepdf", "run", "--workflow", workflow.to_str().unwrap()],
        [],
        &mut stdout,
        &mut stderr,
    );
    let stderr = String::from_utf8(stderr).unwrap();

    assert_eq!(code, 0);
    assert_eq!(stdout, b"");
    assert_eq!(stderr, "");
    let report: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(output).unwrap()).unwrap();
    assert_ne!(report["verdict"], "trusted");
    assert_eq!(report["trust_anchor_count"], 0);
}
