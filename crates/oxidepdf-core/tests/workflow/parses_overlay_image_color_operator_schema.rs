
#[tokio::test]
async fn workflow_rate_limit_delays_independent_task_start() {
    let workflow = workflow_from_json(
        r#"
            {
              "version": 1,
              "inputs": [{ "id": "source", "path": "input.pdf" }],
              "tasks": [
                {
                  "id": "first",
                  "op": { "pdf_edit": { "rotate_pages": { "pages": "1", "degrees": 90 } } },
                  "inputs": ["source"]
                },
                {
                  "id": "second",
                  "op": { "pdf_edit": { "rotate_pages": { "pages": "1", "degrees": 180 } } },
                  "inputs": ["source"]
                }
              ],
              "outputs": [
                { "id": "first_out", "from": "first", "path": "first.pdf" },
                { "id": "second_out", "from": "second", "path": "second.pdf" }
              ],
              "limits": { "rate_limit_per_second": 2 }
            }
            "#,
    );
    let mut store = ArtifactStore::new();
    store.insert(artifact_ref("source"), Artifact::bytes(b"input").unwrap());

    #[derive(Clone, Default)]
    struct TimedRunner(std::sync::Arc<std::sync::Mutex<Vec<std::time::Instant>>>);
    impl OperatorRunner for TimedRunner {
        fn run(&self, task: TaskSpec, _inputs: Vec<Artifact>) -> oxidepdf_core::OperatorFuture {
            let starts = self.0.clone();
            Box::pin(async move {
                starts.lock().unwrap().push(std::time::Instant::now());
                Artifact::bytes(task.id.as_str().as_bytes())
            })
        }
    }

    let runner = TimedRunner::default();
    execute_workflow(&workflow, store, runner.clone())
        .await
        .unwrap();

    let starts = runner.0.lock().unwrap().clone();
    assert_eq!(starts.len(), 2);
    let elapsed = starts[1].duration_since(starts[0]);
    assert!(
        elapsed >= std::time::Duration::from_millis(400),
        "expected rate-limited starts, got {elapsed:?}"
    );
}

#[tokio::test]
async fn execute_workflow_rejects_task_count_over_limit() {
    let workflow = workflow_from_json(
        r#"
            {
              "version": 1,
              "inputs": [{ "id": "source", "path": "input.pdf" }],
              "tasks": [
                {
                  "id": "first",
                  "op": { "pdf_edit": { "rotate_pages": { "pages": "1", "degrees": 90 } } },
                  "inputs": ["source"]
                },
                {
                  "id": "second",
                  "op": { "pdf_edit": { "rotate_pages": { "pages": "1", "degrees": 180 } } },
                  "inputs": ["source"]
                }
              ],
              "outputs": [{ "id": "final", "from": "second", "path": "out.pdf" }],
              "limits": { "max_tasks": 1 }
            }
            "#,
    );
    let mut store = ArtifactStore::new();
    store.insert(artifact_ref("source"), Artifact::bytes(b"input").unwrap());
    let runner = RecordingRunner::default();

    let err = execute_workflow(&workflow, store, runner.clone())
        .await
        .unwrap_err();

    assert_eq!(
        err,
        OxideError::ResourceLimitExceeded {
            limit: "max_tasks".to_owned()
        }
    );
    assert!(runner.executed().is_empty());
}

#[tokio::test]
async fn workflow_parallelism_limit_serializes_independent_tasks() {
    let workflow = workflow_from_json(
        r#"
            {
              "version": 1,
              "inputs": [{ "id": "source", "path": "input.pdf" }],
              "tasks": [
                {
                  "id": "first",
                  "op": { "pdf_edit": { "rotate_pages": { "pages": "1", "degrees": 90 } } },
                  "inputs": ["source"]
                },
                {
                  "id": "second",
                  "op": { "pdf_edit": { "rotate_pages": { "pages": "1", "degrees": 180 } } },
                  "inputs": ["source"]
                }
              ],
              "outputs": [
                { "id": "first_out", "from": "first", "path": "first.pdf" },
                { "id": "second_out", "from": "second", "path": "second.pdf" }
              ],
              "limits": { "max_parallel_tasks": 1 }
            }
            "#,
    );
    let mut store = ArtifactStore::new();
    store.insert(artifact_ref("source"), Artifact::bytes(b"input").unwrap());

    #[derive(Clone, Default)]
    struct CountingRunner {
        state: std::sync::Arc<std::sync::Mutex<(usize, usize)>>,
    }

    impl OperatorRunner for CountingRunner {
        fn run(&self, task: TaskSpec, _inputs: Vec<Artifact>) -> oxidepdf_core::OperatorFuture {
            let state = self.state.clone();
            Box::pin(async move {
                {
                    let mut state = state.lock().unwrap();
                    state.0 += 1;
                    state.1 = state.1.max(state.0);
                }
                std::thread::sleep(std::time::Duration::from_millis(40));
                {
                    let mut state = state.lock().unwrap();
                    state.0 -= 1;
                }
                Artifact::bytes(task.id.as_str().as_bytes())
            })
        }
    }

    let runner = CountingRunner::default();
    execute_workflow(&workflow, store, runner.clone())
        .await
        .unwrap();

    assert_eq!(runner.state.lock().unwrap().1, 1);
}

#[test]
fn parses_overlay_image_color_operator_schema() {
    let workflow: Workflow = serde_saphyr::from_str(
        r#"
        version: 1
        inputs:
          - id: pdf
            path: ./input.pdf
          - id: overlay
            path: ./overlay.pdf
          - id: image
            path: ./image.png
        tasks:
          - id: stamp
            op:
              pdf_edit:
                overlay:
                  kind: stamp
                  text: APPROVED
                  pages: "1"
                  opacity: 0.7
            inputs: [pdf]
          - id: overlay_pdf
            op:
              pdf_edit:
                overlay:
                  kind: pdf_page
                  pages: "1"
                  source_page: 1
            inputs: [stamp, overlay]
          - id: image_replace
            op:
              pdf_edit:
                image_edit:
                  action: replace
                  name: Im1
            inputs: [overlay_pdf, image]
          - id: color
            op:
              pdf_edit:
                color:
                  action: invert
                  pages: "1"
            inputs: [image_replace]
          - id: list_images
            op:
              pdf_inspect:
                images: {}
            inputs: [color]
        outputs:
          - id: report
            from: list_images
            path: ./images.json
        "#,
    )
    .unwrap();

    assert_eq!(workflow.tasks.len(), 5);
}

#[test]
fn parses_compression_operator_schema() {
    let workflow: Workflow = serde_saphyr::from_str(
        r#"
        version: 1
        inputs:
          - id: source
            path: ./input.pdf
        tasks:
          - id: compress
            op:
              pdf_edit:
                compression:
                  mode: lossless
            inputs: [source]
        outputs:
          - id: final
            from: compress
            path: ./output.pdf
        "#,
    )
    .unwrap();

    assert!(matches!(
        workflow.tasks[0].op,
        OperatorSpec::PdfEdit(PdfEditOptions::Compression(CompressionOptions {
            mode: CompressionMode::Lossless,
            images: None,
        }))
    ));
}
