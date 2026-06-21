
#[tokio::test]
async fn workflow_signature_operator_writes_json_report() {
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
    )
    .await;

    assert_eq!(code, 0);
    assert_eq!(stdout, b"");
    assert_eq!(stderr, b"");
    let report: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(output).unwrap()).unwrap();
    assert_eq!(report["verdict"], "invalid");
    assert_eq!(report["trust_anchor_count"], 1);
}

#[tokio::test]
async fn workflow_signature_operator_without_trust_anchors_is_not_trusted() {
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
    )
    .await;
    let stderr = String::from_utf8(stderr).unwrap();

    assert_eq!(code, 0);
    assert_eq!(stdout, b"");
    assert_eq!(stderr, "");
    let report: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(output).unwrap()).unwrap();
    assert_ne!(report["verdict"], "trusted");
    assert_eq!(report["trust_anchor_count"], 0);
}
