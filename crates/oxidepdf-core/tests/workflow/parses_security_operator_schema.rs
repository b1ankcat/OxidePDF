
#[test]
fn parses_security_operator_schema() {
    let workflow: Workflow = serde_saphyr::from_str(
        r#"
        version: 1
        inputs:
          - id: source
            path: ./input.pdf
        tasks:
          - id: encrypt
            op:
              pdf_security:
                encrypt:
                  owner_password: owner-pass
                  user_password: user-pass
                  algorithm: aes256
                  permissions:
                    print: true
                    modify: false
                    copy: false
                    annotate: false
                    fill_forms: true
                    accessibility: true
                    assemble: false
                    high_quality_print: true
            inputs: [source]
          - id: decrypt
            op:
              pdf_security:
                decrypt:
                  password: user-pass
            inputs: [encrypt]
          - id: permissions
            op:
              pdf_security:
                permissions_get:
                  password: owner-pass
            inputs: [encrypt]
        outputs:
          - id: final
            from: decrypt
            path: ./output.pdf
        "#,
    )
    .unwrap();

    assert!(matches!(
        workflow.tasks[0].op,
        OperatorSpec::PdfSecurity(PdfSecurityOptions::Encrypt(SecurityEncryptOptions {
            algorithm: EncryptionAlgorithm::Aes256,
            ..
        }))
    ));
    assert!(matches!(
        workflow.tasks[1].op,
        OperatorSpec::PdfSecurity(PdfSecurityOptions::Decrypt(_))
    ));
    assert!(matches!(
        workflow.tasks[2].op,
        OperatorSpec::PdfSecurity(PdfSecurityOptions::PermissionsGet(_))
    ));
}

#[tokio::test]
async fn pdf_operator_runner_handles_page_editing_tasks() {
    let pdf = fixture_pdf();
    let runner = PdfOperatorRunner::default();

    let merged = runner
        .run(
            TaskSpec {
                id: TaskId::new("merge"),
                op: OperatorSpec::PdfEdit(PdfEditOptions::Merge(MergeOptions {})),
                inputs: vec![artifact_ref("a"), artifact_ref("b")],
            },
            vec![Artifact::pdf(pdf).unwrap(), Artifact::pdf(pdf).unwrap()],
        )
        .await
        .unwrap();

    // Object-level operators emit a parsed document; it must serialize back to
    // a valid PDF at the output boundary.
    assert!(matches!(merged, Artifact::PdfObject(_)));
    let bytes = merged.output_bytes().unwrap();
    assert!(bytes.starts_with(b"%PDF-"));
}

#[tokio::test]
async fn pdf_operator_runner_enforces_output_size_limit() {
    let pdf = fixture_pdf();
    let runner = PdfOperatorRunner::with_limits(ResourceLimits {
        max_output_bytes: Some(1),
        ..ResourceLimits::default()
    });

    let err = runner
        .run(
            TaskSpec {
                id: TaskId::new("split"),
                op: OperatorSpec::PdfEdit(PdfEditOptions::KeepPages(SplitOptions {
                    pages: "1".to_owned(),
                })),
                inputs: vec![artifact_ref("source")],
            },
            vec![Artifact::pdf(pdf).unwrap()],
        )
        .await
        .unwrap_err();

    assert_eq!(
        err,
        OxideError::ResourceLimitExceeded {
            limit: "max_output_bytes".to_owned()
        }
    );
}

#[tokio::test]
async fn object_level_operator_emits_parsed_pdf_object() {
    // A migrated page operator returns a parsed object tree, not serialized
    // bytes, so a downstream operator can consume it without re-parsing.
    let pdf = fixture_pdf();
    let runner = PdfOperatorRunner::default();

    let artifact = runner
        .run(
            TaskSpec {
                id: TaskId::new("keep"),
                op: OperatorSpec::PdfEdit(PdfEditOptions::KeepPages(SplitOptions {
                    pages: "1".to_owned(),
                })),
                inputs: vec![artifact_ref("source")],
            },
            vec![Artifact::pdf(pdf).unwrap()],
        )
        .await
        .unwrap();

    assert!(matches!(artifact, Artifact::PdfObject(_)));

    // The object artifact feeds straight into another object-level operator.
    let chained = runner
        .run(
            TaskSpec {
                id: TaskId::new("extract"),
                op: OperatorSpec::PdfEdit(PdfEditOptions::ExtractPages(PageSelectionOptions {
                    pages: "1".to_owned(),
                })),
                inputs: vec![artifact_ref("keep")],
            },
            vec![artifact],
        )
        .await
        .unwrap();
    assert!(matches!(chained, Artifact::PdfObject(_)));
}

#[tokio::test]
async fn inspect_operator_consumes_object_artifact_without_materializing_bytes() {
    let pdf = fixture_pdf();
    let runner = PdfOperatorRunner::default();

    let object_artifact = runner
        .run(
            TaskSpec {
                id: TaskId::new("rotate"),
                op: OperatorSpec::PdfEdit(PdfEditOptions::RotatePages(RotateOptions {
                    pages: "1".to_owned(),
                    degrees: 90,
                })),
                inputs: vec![artifact_ref("source")],
            },
            vec![Artifact::pdf(pdf).unwrap()],
        )
        .await
        .unwrap();
    assert!(matches!(object_artifact, Artifact::PdfObject(_)));

    let inspected = runner
        .run(
            TaskSpec {
                id: TaskId::new("metadata"),
                op: OperatorSpec::PdfInspect(PdfInspectOptions::Metadata(
                    MetadataInspectOptions::default(),
                )),
                inputs: vec![artifact_ref("rotate")],
            },
            vec![object_artifact],
        )
        .await
        .unwrap();

    let Artifact::Text(report_text) = inspected else {
        panic!("metadata inspect should emit a text JSON report");
    };
    let report: serde_json::Value = serde_json::from_str(&report_text.text).unwrap();
    assert_eq!(report["valid"], true);
    assert!(report["entries"].is_object());
}

#[tokio::test]
async fn object_artifact_inputs_are_checked_against_input_size_limit() {
    let pdf = fixture_pdf();
    let unrestricted_runner = PdfOperatorRunner::default();

    let object_artifact = unrestricted_runner
        .run(
            TaskSpec {
                id: TaskId::new("rotate"),
                op: OperatorSpec::PdfEdit(PdfEditOptions::RotatePages(RotateOptions {
                    pages: "1".to_owned(),
                    degrees: 90,
                })),
                inputs: vec![artifact_ref("source")],
            },
            vec![Artifact::pdf(pdf).unwrap()],
        )
        .await
        .unwrap();
    assert!(matches!(object_artifact, Artifact::PdfObject(_)));

    let limited_runner = PdfOperatorRunner::with_limits(ResourceLimits {
        max_input_bytes: Some(1),
        ..ResourceLimits::default()
    });
    let err = limited_runner
        .run(
            TaskSpec {
                id: TaskId::new("metadata"),
                op: OperatorSpec::PdfInspect(PdfInspectOptions::Metadata(
                    MetadataInspectOptions::default(),
                )),
                inputs: vec![artifact_ref("rotate")],
            },
            vec![object_artifact],
        )
        .await
        .unwrap_err();

    assert_eq!(
        err,
        OxideError::ResourceLimitExceeded {
            limit: "max_input_bytes".to_owned()
        }
    );
}
