
#[tokio::test]
async fn pdf_operator_runner_emits_signature_verification_report() {
    let pdf = pdf_with_signature_dictionary(vec![0, 64, 192, 64], vec![0x30, 0x82]);
    let trust_anchors = write_test_trust_anchors("signature_report");
    let runner = PdfOperatorRunner::default();

    let artifact = runner
        .run(
            TaskSpec {
                id: TaskId::new("verify"),
                op: OperatorSpec::PdfSign(PdfSignOptions::Verify(SignatureOptions {
                    mode: SignatureMode::Verify,
                    trust_anchors: Some(trust_anchors),
                })),
                inputs: vec![artifact_ref("source")],
            },
            vec![Artifact::pdf(&pdf).unwrap()],
        )
        .await
        .unwrap();

    let Artifact::Text(report_text) = artifact else {
        panic!("signature verification should emit a text JSON report");
    };
    let report: SignatureVerificationReport = serde_json::from_str(&report_text.text).unwrap();
    assert_eq!(report.trust_anchor_count, 1);
    assert_eq!(report.verdict, SignatureVerdict::Invalid);
    assert_eq!(report.signatures.len(), 1);
    assert_eq!(report.signatures[0].field_name.as_deref(), Some("Approval"));
    assert_eq!(
        report.signatures[0].subfilter.as_deref(),
        Some("adbe.pkcs7.detached")
    );
    assert_eq!(
        report.signatures[0].byte_range.values,
        Some([0, 64, 192, 64])
    );
    assert!(report.signatures[0].byte_range.in_bounds);
    assert!(report.signatures[0].byte_range.ordered_non_overlapping);
    assert_eq!(report.signatures[0].byte_range.gap_len, Some(128));
    assert_eq!(
        report.signatures[0].cms_status.status,
        SignatureCheckState::Failed
    );
    assert_eq!(
        report.signatures[0].revocation_status.status,
        SignatureCheckState::Indeterminate
    );
}

#[tokio::test]
async fn pdf_operator_runner_emits_signature_list_report_without_trust_anchors() {
    let pdf = pdf_with_signature_dictionary(vec![0, 64, 192, 64], vec![0x30, 0x82]);
    let runner = PdfOperatorRunner::default();

    let artifact = runner
        .run(
            TaskSpec {
                id: TaskId::new("list"),
                op: OperatorSpec::PdfSign(PdfSignOptions::List(SignatureOptions {
                    mode: SignatureMode::List,
                    trust_anchors: None,
                })),
                inputs: vec![artifact_ref("source")],
            },
            vec![Artifact::pdf(&pdf).unwrap()],
        )
        .await
        .unwrap();

    let Artifact::Text(report_text) = artifact else {
        panic!("signature list should emit a text JSON report");
    };
    let report: SignatureListReport = serde_json::from_str(&report_text.text).unwrap();
    assert_eq!(report.signatures.len(), 1);
    assert_eq!(report.signatures[0].field_name.as_deref(), Some("Approval"));
    assert_eq!(
        report.signatures[0].subfilter.as_deref(),
        Some("adbe.pkcs7.detached")
    );
    assert_eq!(
        report.signatures[0].byte_range.values,
        Some([0, 64, 192, 64])
    );
    assert!(report.diagnostics.is_empty());
}

#[tokio::test]
async fn pdf_operator_runner_handles_extract_text_tasks() {
    let pdf = fixture_pdf();
    let runner = PdfOperatorRunner::default();

    let extracted = runner
        .run(
            TaskSpec {
                id: TaskId::new("extract"),
                op: OperatorSpec::PdfInspect(PdfInspectOptions::ExtractText(
                    ExtractTextOptions::default(),
                )),
                inputs: vec![artifact_ref("source")],
            },
            vec![Artifact::pdf(pdf).unwrap()],
        )
        .await
        .unwrap();

    let Artifact::Text(text) = extracted else {
        panic!("expected text artifact");
    };
    assert!(!text.text.trim().is_empty());
}

#[tokio::test]
async fn execute_workflow_enforces_timeout() {
    let workflow = workflow_from_json(
        r#"
            {
              "version": 1,
              "inputs": [{ "id": "source", "path": "input.bin" }],
              "tasks": [
                {
                  "id": "slow",
                  "op": { "pdf_edit": { "merge": {} } },
                  "inputs": ["source"]
                }
              ],
              "outputs": [{ "id": "final", "from": "slow", "path": "out.bin" }],
              "limits": { "timeout_ms": 1 }
            }
            "#,
    );
    let mut store = ArtifactStore::new();
    store.insert(artifact_ref("source"), Artifact::bytes(b"input").unwrap());
    let runner = SlowRunner;

    let err = execute_workflow(&workflow, store, runner.clone())
        .await
        .unwrap_err();

    assert_eq!(
        err,
        OxideError::ResourceLimitExceeded {
            limit: "timeout_ms".to_owned()
        }
    );
}

#[tokio::test]
async fn execute_workflow_timeout_stops_downstream_tasks() {
    let workflow = workflow_from_json(
        r#"
            {
              "version": 1,
              "inputs": [{ "id": "source", "path": "input.bin" }],
              "tasks": [
                {
                  "id": "slow",
                  "op": { "pdf_edit": { "merge": {} } },
                  "inputs": ["source"]
                },
                {
                  "id": "downstream",
                  "op": { "pdf_edit": { "merge": {} } },
                  "inputs": ["slow"]
                }
              ],
              "outputs": [{ "id": "final", "from": "downstream", "path": "out.bin" }],
              "limits": { "timeout_ms": 20 }
            }
            "#,
    );
    let mut store = ArtifactStore::new();
    store.insert(artifact_ref("source"), Artifact::bytes(b"input").unwrap());

    #[derive(Clone)]
    struct SlowRecordingRunner {
        executed: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
        barrier: std::sync::Arc<std::sync::Barrier>,
    }
    impl OperatorRunner for SlowRecordingRunner {
        fn run(&self, task: TaskSpec, _inputs: Vec<Artifact>) -> oxidepdf_core::OperatorFuture {
            let executed = self.executed.clone();
            let barrier = self.barrier.clone();
            Box::pin(async move {
                executed.lock().unwrap().push(task.id.as_str().to_owned());
                barrier.wait();
                std::thread::sleep(std::time::Duration::from_millis(50));
                Artifact::bytes(task.id.as_str().as_bytes())
            })
        }
    }

    let runner = SlowRecordingRunner {
        executed: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
        barrier: std::sync::Arc::new(std::sync::Barrier::new(2)),
    };
    let release = runner.barrier.clone();
    let keep_running = tokio::spawn(async move {
        tokio::task::spawn_blocking(move || release.wait())
            .await
            .unwrap();
    });
    let err = execute_workflow(&workflow, store, runner.clone())
        .await
        .unwrap_err();
    keep_running.await.unwrap();

    assert_eq!(
        err,
        OxideError::ResourceLimitExceeded {
            limit: "timeout_ms".to_owned()
        }
    );
    assert_eq!(runner.executed.lock().unwrap().as_slice(), ["slow"]);
}
