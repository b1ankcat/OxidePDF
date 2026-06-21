
#[test]
fn dag_workflow_topologically_sorts_before_execution() {
    let workflow = workflow_from_json(
        r#"
            {
              "version": 1,
              "inputs": [{ "id": "source", "path": "input.pdf" }],
              "tasks": [
                {
                  "id": "left",
                  "op": { "pdf_edit": { "rotate_pages": { "pages": "1", "degrees": 90 } } },
                  "inputs": ["source"]
                },
                {
                  "id": "right",
                  "op": { "pdf_edit": { "rotate_pages": { "pages": "1", "degrees": 180 } } },
                  "inputs": ["source"]
                },
                {
                  "id": "join",
                  "op": { "pdf_edit": { "merge": {} } },
                  "inputs": ["left", "right"]
                }
              ],
              "outputs": [{ "id": "final", "from": "join", "path": "out.pdf" }]
            }
            "#,
    );

    let plan = validate_workflow(&workflow).unwrap();
    let left = plan
        .task_order
        .iter()
        .position(|id| id.as_str() == "left")
        .unwrap();
    let right = plan
        .task_order
        .iter()
        .position(|id| id.as_str() == "right")
        .unwrap();
    let join = plan
        .task_order
        .iter()
        .position(|id| id.as_str() == "join")
        .unwrap();

    assert!(left < join);
    assert!(right < join);

    // The plan also exposes parallel layers: the two independent rotations sit
    // in the first layer, and the merge that depends on both sits alone in the
    // second.
    assert_eq!(plan.layers.len(), 2);
    assert_eq!(plan.layers[0].len(), 2);
    assert_eq!(plan.layers[1].len(), 1);
    let join_index = plan
        .task_index
        .get(&oxidepdf_core::TaskId::new("join"))
        .copied()
        .unwrap();
    assert_eq!(plan.layers[1][0], join_index);
}

#[test]
fn cyclic_workflow_fails_validation() {
    let workflow = workflow_from_json(
        r#"
            {
              "version": 1,
              "inputs": [],
              "tasks": [
                {
                  "id": "a",
                  "op": { "pdf_edit": { "merge": {} } },
                  "inputs": ["b"]
                },
                {
                  "id": "b",
                  "op": { "pdf_edit": { "merge": {} } },
                  "inputs": ["a"]
                }
              ],
              "outputs": [{ "id": "final", "from": "b", "path": "out.pdf" }]
            }
            "#,
    );

    let err = validate_workflow(&workflow).unwrap_err();

    assert!(matches!(err, OxideError::InvalidWorkflow { .. }));
    assert!(err.to_string().contains("cycle"));
}

#[test]
fn missing_artifact_reference_fails_validation() {
    let workflow = workflow_from_json(
        r#"
            {
              "version": 1,
              "inputs": [{ "id": "source", "path": "input.pdf" }],
              "tasks": [
                {
                  "id": "rotate",
                  "op": { "pdf_edit": { "rotate_pages": { "pages": "1", "degrees": 90 } } },
                  "inputs": ["missing"]
                }
              ],
              "outputs": [{ "id": "final", "from": "rotate", "path": "out.pdf" }]
            }
            "#,
    );

    let err = validate_workflow(&workflow).unwrap_err();

    assert!(matches!(err, OxideError::InvalidWorkflow { .. }));
    assert!(err.to_string().contains("missing"));
}

#[test]
fn output_identifier_cannot_be_used_as_task_input() {
    let workflow = workflow_from_json(
        r#"
            {
              "version": 1,
              "inputs": [{ "id": "source", "path": "input.pdf" }],
              "tasks": [
                {
                  "id": "rotate",
                  "op": { "pdf_edit": { "rotate_pages": { "pages": "1", "degrees": 90 } } },
                  "inputs": ["source"]
                },
                {
                  "id": "render",
                  "op": { "pdf_inspect": { "render": { "page": 1, "format": "png", "scale": 1.0 } } },
                  "inputs": ["final"]
                }
              ],
              "outputs": [{ "id": "final", "from": "rotate", "path": "out.png" }]
            }
            "#,
    );

    let err = validate_workflow(&workflow).unwrap_err();

    assert!(matches!(err, OxideError::InvalidWorkflow { .. }));
    assert!(err.to_string().contains("missing artifact 'final'"));
}

#[test]
fn duplicate_artifact_identifiers_fail_validation() {
    let workflow = workflow_from_json(
        r#"
            {
              "version": 1,
              "inputs": [{ "id": "source", "path": "input.pdf" }],
              "tasks": [
                {
                  "id": "source",
                  "op": { "pdf_edit": { "rotate_pages": { "pages": "1", "degrees": 90 } } },
                  "inputs": ["source"]
                }
              ],
              "outputs": [{ "id": "final", "from": "source", "path": "out.pdf" }]
            }
            "#,
    );

    let err = validate_workflow(&workflow).unwrap_err();

    assert!(matches!(err, OxideError::InvalidWorkflow { .. }));
    assert!(err.to_string().contains("duplicate"));
}

#[tokio::test]
async fn task_failure_stops_downstream_execution() {
    let workflow = workflow_from_json(
        r#"
            {
              "version": 1,
              "inputs": [{ "id": "source", "path": "input.pdf" }],
              "tasks": [
                {
                  "id": "fail",
                  "op": { "pdf_edit": { "rotate_pages": { "pages": "1", "degrees": 90 } } },
                  "inputs": ["source"]
                },
                {
                  "id": "after",
                  "op": { "pdf_inspect": { "render": { "page": 1, "format": "png", "scale": 1.0 } } },
                  "inputs": ["fail"]
                }
              ],
              "outputs": [{ "id": "final", "from": "after", "path": "out.png" }]
            }
            "#,
    );
    let mut store = ArtifactStore::new();
    store.insert(artifact_ref("source"), Artifact::bytes(b"input").unwrap());
    let expected = OxideError::InvalidInput {
        reason: "runner failed".to_owned(),
    };
    let runner = RecordingRunner::with_failure("fail", expected.clone());

    let err = execute_workflow(&workflow, store, runner.clone())
        .await
        .unwrap_err();

    assert_eq!(err, expected);
    assert_eq!(runner.executed(), ["fail"]);
}

#[tokio::test]
async fn workflow_retries_failed_task_before_succeeding() {
    let workflow = workflow_from_json(
        r#"
            {
              "version": 1,
              "inputs": [{ "id": "source", "path": "input.pdf" }],
              "tasks": [
                {
                  "id": "flaky",
                  "op": { "pdf_edit": { "rotate_pages": { "pages": "1", "degrees": 90 } } },
                  "inputs": ["source"]
                }
              ],
              "outputs": [{ "id": "final", "from": "flaky", "path": "out.pdf" }],
              "limits": { "retry_attempts": 1 }
            }
            "#,
    );
    let mut store = ArtifactStore::new();
    store.insert(artifact_ref("source"), Artifact::bytes(b"input").unwrap());

    #[derive(Clone, Default)]
    struct FlakyRunner(std::sync::Arc<std::sync::Mutex<usize>>);
    impl OperatorRunner for FlakyRunner {
        fn run(&self, task: TaskSpec, _inputs: Vec<Artifact>) -> oxidepdf_core::OperatorFuture {
            let attempts = self.0.clone();
            Box::pin(async move {
                let mut attempts = attempts.lock().unwrap();
                *attempts += 1;
                if *attempts == 1 {
                    return Err(OxideError::InvalidInput {
                        reason: "transient failure".to_owned(),
                    });
                }
                Artifact::bytes(task.id.as_str().as_bytes())
            })
        }
    }

    let runner = FlakyRunner::default();
    let result = execute_workflow(&workflow, store, runner.clone())
        .await
        .unwrap();

    assert_eq!(*runner.0.lock().unwrap(), 2);
    assert_eq!(
        result.store.get(&artifact_ref("flaky")),
        Some(&Artifact::bytes(b"flaky").unwrap())
    );
}
