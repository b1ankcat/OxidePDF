#[allow(unused_imports)]
use common::*;
#[allow(unused_imports)]
use oxidepdf_core::*;

#[test]
fn workflow_schema_version_starts_at_one() {
    assert_eq!(WORKFLOW_SCHEMA_VERSION, 1);
}

#[test]
fn parses_example_json_workflow() {
    let workflow: Workflow = serde_json::from_str(
        r#"
            {
              "version": 1,
              "inputs": [
                { "id": "source", "path": "./input.pdf" }
              ],
              "tasks": [
                {
                  "id": "rotate_pages",
                  "op": {
                    "pdf_edit": {
                      "rotate_pages": {
                        "pages": "1,3-5",
                        "degrees": 90
                      }
                    }
                  },
                  "inputs": ["source"]
                }
              ],
              "outputs": [
                { "id": "final", "from": "rotate_pages", "path": "./output.pdf" }
              ],
              "limits": {
                "max_input_bytes": 524288000,
                "max_pages": 5000,
                "max_pixels": 160000000
              },
              "metadata": {
                "title": "Example workflow"
              }
            }
            "#,
    )
    .unwrap();

    assert_eq!(workflow.version, WorkflowVersion::V1);
    assert_eq!(workflow.inputs[0].id.as_str(), "source");
    assert_eq!(workflow.tasks[0].id.as_str(), "rotate_pages");
    assert!(matches!(
        workflow.tasks[0].op,
        OperatorSpec::PdfEdit(PdfEditOptions::RotatePages(RotateOptions {
            degrees: 90,
            ..
        }))
    ));
    assert_eq!(workflow.outputs[0].from.as_str(), "rotate_pages");
}

#[test]
fn parses_example_yaml_workflow() {
    let workflow: Workflow = serde_saphyr::from_str(
        r#"
            version: 1
            inputs:
              - id: source
                path: ./input.pdf
            tasks:
              - id: rotate_pages
                op:
                  pdf_edit:
                    rotate_pages:
                      pages: "1,3-5"
                      degrees: 90
                inputs: [source]
              - id: stamp
                op:
                  pdf_edit:
                    watermark:
                      kind: text
                      text: Confidential
                      opacity: 0.18
                      position: center
                inputs: [rotate_pages]
            outputs:
              - id: final
                from: stamp
                path: ./output.pdf
            limits:
              max_input_bytes: 524288000
              max_pages: 5000
              max_pixels: 160000000
            metadata:
              title: Example workflow
            "#,
    )
    .unwrap();

    assert_eq!(workflow.version, WorkflowVersion::V1);
    assert_eq!(workflow.tasks.len(), 2);
    assert!(matches!(
        workflow.tasks[1].op,
        OperatorSpec::PdfEdit(PdfEditOptions::Watermark(WatermarkOptions {
            kind: WatermarkKind::Text,
            ..
        }))
    ));
}

#[test]
fn parses_signature_workflow_operator_schema() {
    let workflow: Workflow = serde_saphyr::from_str(
        r#"
            version: 1
            inputs:
              - id: source
                path: ./signed.pdf
            tasks:
              - id: verify
                op:
                  pdf_sign:
                    verify:
                      mode: verify
                      trust_anchors: ./anchors.pem
                inputs: [source]
            outputs:
              - id: final
                from: verify
                path: ./report.json
            "#,
    )
    .unwrap();

    assert!(matches!(
        workflow.tasks[0].op,
        OperatorSpec::PdfSign(PdfSignOptions::Verify(SignatureOptions {
            mode: SignatureMode::Verify,
            trust_anchors: Some(_),
        }))
    ));
    match &workflow.tasks[0].op {
        OperatorSpec::PdfSign(PdfSignOptions::Verify(options)) => {
            assert_eq!(
                options.trust_anchors.as_deref(),
                Some(std::path::Path::new("./anchors.pem"))
            );
        }
        _ => unreachable!("asserted signature operator above"),
    }
}

#[test]
fn parses_signature_list_workflow_operator_schema() {
    let workflow: Workflow = serde_saphyr::from_str(
        r#"
            version: 1
            inputs:
              - id: source
                path: ./signed.pdf
            tasks:
              - id: list
                op:
                  pdf_sign:
                    list:
                      mode: list
                inputs: [source]
            outputs:
              - id: final
                from: list
                path: ./report.json
            "#,
    )
    .unwrap();

    assert!(matches!(
        workflow.tasks[0].op,
        OperatorSpec::PdfSign(PdfSignOptions::List(SignatureOptions {
            mode: SignatureMode::List,
            trust_anchors: None,
        }))
    ));
}

#[test]
fn parses_signature_mutation_workflow_operator_schema() {
    let workflow: Workflow = serde_saphyr::from_str(
        r#"
            version: 1
            inputs:
              - id: source
                path: ./unsigned.pdf
            tasks:
              - id: add_sig
                op:
                  pdf_sign:
                    add:
                      field_name: Approval
                      certificate: ./cert.pem
                      private_key: ./key.pem
                      contents_reserved_bytes: 8192
                inputs: [source]
              - id: delete_sig
                op:
                  pdf_sign:
                    delete_field:
                      field_name: Approval
                      destructive: true
                inputs: [add_sig]
              - id: timestamp
                op:
                  pdf_sign:
                    timestamp:
                      token: ./timestamp.tsr
                inputs: [delete_sig]
            outputs:
              - id: final
                from: timestamp
                path: ./report.json
            "#,
    )
    .unwrap();

    assert!(matches!(
        workflow.tasks[0].op,
        OperatorSpec::PdfSign(PdfSignOptions::Add(SignatureAddOptions { .. }))
    ));
    assert!(matches!(
        workflow.tasks[1].op,
        OperatorSpec::PdfSign(PdfSignOptions::DeleteField(SignatureDeleteFieldOptions {
            destructive: true,
            ..
        }))
    ));
    assert!(matches!(
        workflow.tasks[2].op,
        OperatorSpec::PdfSign(PdfSignOptions::Timestamp(TimestampAddOptions {
            token: Some(_),
            tsa_url: None,
        }))
    ));
}

#[test]
fn parses_compare_workflow_operator_schema() {
    let workflow: Workflow = serde_saphyr::from_str(
        r#"
            version: 1
            inputs:
              - id: left
                path: ./left.pdf
              - id: right
                path: ./right.pdf
            tasks:
              - id: compare
                op:
                  pdf_compare:
                    report:
                      include_text: false
                inputs: [left, right]
            outputs:
              - id: final
                from: compare
                path: ./report.json
            "#,
    )
    .unwrap();

    assert!(matches!(
        workflow.tasks[0].op,
        OperatorSpec::PdfCompare(PdfCompareOptions::Report(CompareOptions {
            include_text: false,
            ..
        }))
    ));
}
