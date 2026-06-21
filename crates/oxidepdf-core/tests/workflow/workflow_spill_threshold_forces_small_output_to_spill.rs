
#[tokio::test]
async fn workflow_spill_threshold_forces_small_output_to_spill() {
    // A zero spill threshold means "spill every non-empty payload". The task
    // emits a tiny output that would normally stay inline; the workflow's
    // threshold must push it to a memory-mapped temp file.
    let workflow = workflow_from_json(
        r#"
            {
              "version": 1,
              "inputs": [{ "id": "source", "path": "input.bin" }],
              "tasks": [
                {
                  "id": "echo",
                  "op": { "pdf_edit": { "rotate_pages": { "pages": "1", "degrees": 90 } } },
                  "inputs": ["source"]
                }
              ],
              "outputs": [{ "id": "final", "from": "echo", "path": "out.bin" }],
              "limits": { "spill_threshold_bytes": 0 }
            }
            "#,
    );
    let mut store = ArtifactStore::new();
    store.insert(artifact_ref("source"), Artifact::bytes(b"input").unwrap());

    #[derive(Clone)]
    struct EchoRunner;
    impl OperatorRunner for EchoRunner {
        fn run(&self, _task: TaskSpec, _inputs: Vec<Artifact>) -> oxidepdf_core::OperatorFuture {
            Box::pin(async { Ok(Artifact::pdf(b"tiny").unwrap()) })
        }
    }

    let result = execute_workflow(&workflow, store, EchoRunner)
        .await
        .unwrap();
    let Some(Artifact::Pdf(pdf)) = result.store.get(&artifact_ref("echo")) else {
        panic!("expected a PDF artifact");
    };
    assert!(pdf.bytes.is_spilled());
}

#[tokio::test]
async fn workflow_spill_threshold_keeps_large_output_inline_when_high() {
    // A high threshold keeps an otherwise-spillable payload inline.
    let workflow = workflow_from_json(
        r#"
            {
              "version": 1,
              "inputs": [{ "id": "source", "path": "input.bin" }],
              "tasks": [
                {
                  "id": "echo",
                  "op": { "pdf_edit": { "rotate_pages": { "pages": "1", "degrees": 90 } } },
                  "inputs": ["source"]
                }
              ],
              "outputs": [{ "id": "final", "from": "echo", "path": "out.bin" }],
              "limits": { "spill_threshold_bytes": 1073741824 }
            }
            "#,
    );
    let mut store = ArtifactStore::new();
    store.insert(artifact_ref("source"), Artifact::bytes(b"input").unwrap());

    #[derive(Clone)]
    struct BigRunner;
    impl OperatorRunner for BigRunner {
        fn run(&self, _task: TaskSpec, _inputs: Vec<Artifact>) -> oxidepdf_core::OperatorFuture {
            Box::pin(async {
                // Larger than the default 64 MiB threshold, smaller than the 1 GiB
                // workflow threshold, so only the workflow threshold decides.
                Artifact::pdf(vec![0u8; 64 * 1024 * 1024 + 4096])
            })
        }
    }

    let result = execute_workflow(&workflow, store, BigRunner).await.unwrap();
    let Some(Artifact::Pdf(pdf)) = result.store.get(&artifact_ref("echo")) else {
        panic!("expected a PDF artifact");
    };
    assert!(!pdf.bytes.is_spilled());
}
