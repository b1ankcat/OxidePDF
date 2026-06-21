
#[test]
fn parses_document_interaction_workflow_operator_schema() {
    let workflow: Workflow = serde_saphyr::from_str(
        r#"
            version: 1
            inputs:
              - id: source
                path: ./input.pdf
              - id: attachment
                path: ./note.txt
            tasks:
              - id: metadata
                op:
                  pdf_edit:
                    metadata:
                      action: set
                      entries:
                        - key: title
                          value: Quarterly Report
                        - key: author
                          value: OxidePDF
                inputs: [source]
              - id: attach
                op:
                  pdf_edit:
                    attachment:
                      action: add
                      name: note.txt
                      description: Review note
                inputs: [metadata, attachment]
              - id: inspect_forms
                op:
                  pdf_inspect:
                    forms: {}
                inputs: [attach]
            outputs:
              - id: final
                from: inspect_forms
                path: ./forms.json
            "#,
    )
    .unwrap();

    assert!(matches!(
        workflow.tasks[0].op,
        OperatorSpec::PdfEdit(PdfEditOptions::Metadata(MetadataEditOptions {
            action: MetadataEditAction::Set,
            ..
        }))
    ));
    assert!(matches!(
        workflow.tasks[2].op,
        OperatorSpec::PdfInspect(PdfInspectOptions::Forms(FormInspectOptions {}))
    ));
}

#[test]
fn missing_required_workflow_field_fails() {
    let err = serde_json::from_str::<Workflow>(
        r#"
            {
              "version": 1,
              "inputs": [],
              "tasks": [],
              "limits": {},
              "metadata": {}
            }
            "#,
    )
    .unwrap_err();

    assert!(err.to_string().contains("outputs"));
}

#[test]
fn operator_spec_rejects_multiple_operator_keys() {
    let err = serde_json::from_str::<OperatorSpec>(
        r#"
            {
              "pdf_edit": { "rotate_pages": { "pages": "1", "degrees": 90 } },
              "pdf_inspect": { "render": { "page": 1 } }
            }
            "#,
    )
    .unwrap_err();

    assert!(
        err.to_string()
            .contains("operator spec must contain exactly one operator")
    );
}

#[test]
fn operator_spec_rejects_removed_legacy_operator_keys() {
    let err = serde_json::from_str::<OperatorSpec>(
        r#"
            {
              "rotate": { "pages": "1", "degrees": 90 }
            }
            "#,
    )
    .unwrap_err();

    assert!(err.to_string().contains("unknown field"));
}

#[test]
fn error_codes_are_stable_machine_readable_values() {
    assert_eq!(
        OxideError::UnsupportedPdfFeature {
            feature: "object stream".to_owned()
        }
        .code(),
        "unsupported_pdf_feature"
    );
    assert_eq!(OxideError::EncryptedPdf.code(), "encrypted_pdf");
    assert_eq!(OxideError::IncorrectPassword.code(), "incorrect_password");
    assert_eq!(OxideError::FontResolution.code(), "font_resolution");
    assert_eq!(
        OxideError::ResourceLimitExceeded {
            limit: "max_pages".to_owned()
        }
        .code(),
        "resource_limit_exceeded"
    );
}

#[tokio::test]
async fn linear_workflow_executes_tasks_in_dependency_order() {
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
                  "inputs": ["rotate"]
                }
              ],
              "outputs": [{ "id": "final", "from": "render", "path": "out.png" }]
            }
            "#,
    );
    let mut store = ArtifactStore::new();
    store.insert(artifact_ref("source"), Artifact::bytes(b"input").unwrap());
    let runner = RecordingRunner::default();

    let result = execute_workflow(&workflow, store, runner.clone())
        .await
        .unwrap();

    assert_eq!(runner.executed(), ["rotate", "render"]);
    assert_eq!(
        result.store.get(&artifact_ref("render")),
        Some(&Artifact::bytes(b"render").unwrap())
    );
    assert_eq!(result.plan.task_order[0].as_str(), "rotate");
    assert_eq!(result.plan.task_order[1].as_str(), "render");
}

#[test]
fn execution_plan_exposes_task_index() {
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
                }
              ],
              "outputs": [{ "id": "final", "from": "rotate", "path": "out.pdf" }]
            }
            "#,
    );

    let plan = validate_workflow(&workflow).unwrap();

    // The index maps each task id to its slot in workflow.tasks, so execution
    // does not rebuild a lookup map on every run.
    let index = plan.task_index.get(&TaskId::new("rotate")).copied();
    assert_eq!(index, Some(0));
}

#[tokio::test]
async fn intermediate_artifact_evicted_after_last_consumer() {
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
                  "inputs": ["rotate"]
                }
              ],
              "outputs": [{ "id": "final", "from": "render", "path": "out.png" }]
            }
            "#,
    );
    let mut store = ArtifactStore::new();
    store.insert(artifact_ref("source"), Artifact::bytes(b"input").unwrap());
    let runner = RecordingRunner::default();

    let result = execute_workflow(&workflow, store, runner.clone())
        .await
        .unwrap();

    // "source" and "rotate" are fully consumed and not referenced by any
    // output, so they must be evicted; only the output artifact survives.
    assert_eq!(result.store.get(&artifact_ref("source")), None);
    assert_eq!(result.store.get(&artifact_ref("rotate")), None);
    assert!(result.store.get(&artifact_ref("render")).is_some());
}

#[tokio::test]
async fn output_referenced_artifact_is_not_evicted() {
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
                  "inputs": ["rotate"]
                }
              ],
              "outputs": [
                { "id": "rotated", "from": "rotate", "path": "rotated.pdf" },
                { "id": "final", "from": "render", "path": "out.png" }
              ]
            }
            "#,
    );
    let mut store = ArtifactStore::new();
    store.insert(artifact_ref("source"), Artifact::bytes(b"input").unwrap());
    let runner = RecordingRunner::default();

    let result = execute_workflow(&workflow, store, runner.clone())
        .await
        .unwrap();

    // "rotate" is consumed by "render" but is also an output, so it must stay.
    assert!(result.store.get(&artifact_ref("rotate")).is_some());
    assert!(result.store.get(&artifact_ref("render")).is_some());
}
