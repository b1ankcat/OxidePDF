use super::*;
use lopdf::{dictionary, Dictionary, Object, Stream};
use pdf_writer::Finish;
use std::collections::BTreeSet;
use std::path::PathBuf;

const A4_WIDTH: f32 = 595.0;
const A4_HEIGHT: f32 = 842.0;

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
    let workflow: Workflow = serde_yaml::from_str(
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
    let workflow: Workflow = serde_yaml::from_str(
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
fn parses_document_interaction_workflow_operator_schema() {
    let workflow: Workflow = serde_yaml::from_str(
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

    assert!(err
        .to_string()
        .contains("operator spec must contain exactly one operator"));
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

#[test]
fn linear_workflow_executes_tasks_in_dependency_order() {
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
    store.insert(artifact_ref("source"), Artifact::bytes(b"input"));
    let mut runner = RecordingRunner::default();

    let result = execute_workflow(&workflow, store, &mut runner).unwrap();

    assert_eq!(runner.executed, ["rotate", "render"]);
    assert_eq!(
        result.store.get(&artifact_ref("render")),
        Some(&Artifact::bytes(b"render"))
    );
    assert_eq!(result.plan.task_order[0].as_str(), "rotate");
    assert_eq!(result.plan.task_order[1].as_str(), "render");
}

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

#[test]
fn task_failure_stops_downstream_execution() {
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
    store.insert(artifact_ref("source"), Artifact::bytes(b"input"));
    let expected = OxideError::InvalidInput {
        reason: "runner failed".to_owned(),
    };
    let mut runner = RecordingRunner {
        fail_on: Some("fail"),
        error: Some(expected.clone()),
        ..RecordingRunner::default()
    };

    let err = execute_workflow(&workflow, store, &mut runner).unwrap_err();

    assert_eq!(err, expected);
    assert_eq!(runner.executed, ["fail"]);
}

#[test]
fn merge_pdf_artifacts_combines_pages() {
    let pdf = include_bytes!("../../../tests/test.pdf");

    let merged = merge_pdf_artifacts(&[Artifact::pdf(pdf), Artifact::pdf(pdf)]).unwrap();
    let document = lopdf::Document::load_mem(&merged.bytes).unwrap();

    assert_eq!(document.get_pages().len(), 6);
}

#[test]
fn merge_pdf_artifacts_enforces_input_and_page_limits() {
    let pdf = include_bytes!("../../../tests/test.pdf");

    let input_err = merge_pdf_artifacts_with_limits(
        &[Artifact::pdf(pdf), Artifact::pdf(pdf)],
        &ResourceLimits {
            max_input_bytes: Some(1),
            ..ResourceLimits::default()
        },
    )
    .unwrap_err();
    assert_eq!(
        input_err,
        OxideError::ResourceLimitExceeded {
            limit: "max_input_bytes".to_owned()
        }
    );

    let page_err = merge_pdf_artifacts_with_limits(
        &[Artifact::pdf(pdf), Artifact::pdf(pdf)],
        &ResourceLimits {
            max_pages: Some(5),
            ..ResourceLimits::default()
        },
    )
    .unwrap_err();
    assert_eq!(
        page_err,
        OxideError::ResourceLimitExceeded {
            limit: "max_pages".to_owned()
        }
    );
}

#[test]
fn split_pdf_keeps_only_selected_pages() {
    let pdf = include_bytes!("../../../tests/test.pdf");

    let split = split_pdf(pdf, "2-3").unwrap();
    let document = lopdf::Document::load_mem(&split.bytes).unwrap();

    assert_eq!(document.get_pages().len(), 2);
    assert_page_numbers(&document, &[1, 2]);
}

#[test]
fn split_pdf_enforces_resource_limits() {
    let pdf = include_bytes!("../../../tests/test.pdf");

    let err = split_pdf_with_limits(
        pdf,
        "1",
        &ResourceLimits {
            max_pages: Some(2),
            ..ResourceLimits::default()
        },
    )
    .unwrap_err();

    assert_eq!(
        err,
        OxideError::ResourceLimitExceeded {
            limit: "max_pages".to_owned()
        }
    );
}

#[test]
fn reorder_pdf_rearranges_pages() {
    let pdf = include_bytes!("../../../tests/test.pdf");

    let reordered = reorder_pdf(pdf, "3,1,2").unwrap();
    let document = lopdf::Document::load_mem(&reordered.bytes).unwrap();

    assert_eq!(document.get_pages().len(), 3);
    assert_page_numbers(&document, &[1, 2, 3]);
}

#[test]
fn rotate_pdf_updates_page_rotation() {
    let pdf = include_bytes!("../../../tests/test.pdf");

    let rotated = rotate_pdf(pdf, "1-2", 90).unwrap();
    let document = lopdf::Document::load_mem(&rotated.bytes).unwrap();

    assert_eq!(page_rotation(&document, 1), 90);
    assert_eq!(page_rotation(&document, 2), 90);
    assert_eq!(page_rotation(&document, 3), 0);
}

#[test]
fn rotate_pdf_rejects_non_pdf_magic_bytes() {
    let err =
        rotate_pdf_with_limits(b"not a pdf", "1", 90, &ResourceLimits::default()).unwrap_err();

    assert!(matches!(err, OxideError::InvalidInput { .. }));
    assert!(err.to_string().contains("expected PDF"));
}

#[test]
fn delete_pdf_pages_removes_selected_pages() {
    let pdf = include_bytes!("../../../tests/test.pdf");

    let deleted = delete_pdf_pages(pdf, "2").unwrap();
    let document = lopdf::Document::load_mem(&deleted.bytes).unwrap();

    assert_eq!(document.get_pages().len(), 2);
}

#[test]
fn delete_pdf_pages_rejects_deleting_every_page() {
    let pdf = include_bytes!("../../../tests/test.pdf");

    let err = delete_pdf_pages(pdf, "1-3").unwrap_err();

    assert!(matches!(err, OxideError::InvalidInput { .. }));
    assert!(err.to_string().contains("leave at least one page"));
}

#[test]
fn extract_pdf_pages_keeps_selected_order() {
    let pdf = include_bytes!("../../../tests/test.pdf");

    let extracted = extract_pdf_pages(pdf, "3,1").unwrap();
    let document = lopdf::Document::load_mem(&extracted.bytes).unwrap();

    assert_eq!(document.get_pages().len(), 2);
    assert_page_numbers(&document, &[1, 2]);
}

#[test]
fn delete_blank_pdf_pages_uses_object_level_blank_detection() {
    let pdf = pdf_with_blank_and_marked_page();

    let deleted = delete_blank_pdf_pages(&pdf, &DeleteBlankPagesOptions::default()).unwrap();
    let document = lopdf::Document::load_mem(&deleted.bytes).unwrap();

    assert_eq!(document.get_pages().len(), 1);
}

#[test]
fn delete_blank_pdf_pages_rejects_unresolved_resource_reference() {
    let pdf = pdf_with_blank_page_and_missing_resources();

    let err = delete_blank_pdf_pages(&pdf, &DeleteBlankPagesOptions::default()).unwrap_err();

    assert!(matches!(err, OxideError::ParsePdf));
}

#[test]
fn crop_pdf_pages_sets_crop_box_on_selected_pages() {
    let pdf = include_bytes!("../../../tests/test.pdf");

    let cropped = crop_pdf_pages(
        pdf,
        &CropPagesOptions {
            pages: Some("1".to_owned()),
            left: 10.0,
            bottom: 20.0,
            right: 300.0,
            top: 400.0,
        },
    )
    .unwrap();
    let document = lopdf::Document::load_mem(&cropped.bytes).unwrap();

    assert_eq!(
        page_box(&document, 1, b"CropBox"),
        [10.0, 20.0, 300.0, 400.0]
    );
    assert!(page_optional_box(&document, 2, b"CropBox").is_none());
}

#[test]
fn scale_pdf_pages_scales_page_box_and_content_stream() {
    let pdf = include_bytes!("../../../tests/test.pdf");

    let scaled = scale_pdf_pages(
        pdf,
        &ScalePagesOptions {
            pages: Some("1".to_owned()),
            factor: 0.5,
        },
    )
    .unwrap();
    let document = lopdf::Document::load_mem(&scaled.bytes).unwrap();
    let media_box = page_box(&document, 1, b"MediaBox");

    assert_eq!(media_box[2], 306.0);
    assert!(page_content_contains(&document, 1, "cm"));
    assert_eq!(page_box(&document, 2, b"MediaBox")[2], 612.0);
}

#[test]
fn pdf_to_single_page_combines_pages_into_one_tall_page() {
    let pdf = include_bytes!("../../../tests/test.pdf");

    let single = pdf_to_single_page(pdf, &SinglePageOptions::default()).unwrap();
    let document = lopdf::Document::load_mem(&single.bytes).unwrap();
    let media_box = page_box(&document, 1, b"MediaBox");

    assert_eq!(document.get_pages().len(), 1);
    assert_eq!(media_box[2], 612.0);
    assert_eq!(media_box[3], 2376.0);
}

#[test]
fn nup_pdf_pages_places_source_pages_as_xobjects() {
    let pdf = include_bytes!("../../../tests/test.pdf");

    let nup = nup_pdf_pages(
        pdf,
        &NUpOptions {
            columns: 2,
            rows: 2,
        },
    )
    .unwrap();
    let document = lopdf::Document::load_mem(&nup.bytes).unwrap();

    assert_eq!(document.get_pages().len(), 1);
    assert_eq!(page_xobject_count(&document, 1), 3);
    assert_eq!(
        page_box(&document, 1, b"MediaBox"),
        [0.0, 0.0, 612.0, 792.0]
    );
    assert!(page_content_contains_operator(&document, 1, "Do"));
}

#[test]
fn nup_pdf_pages_rejects_zero_columns() {
    let pdf = include_bytes!("../../../tests/test.pdf");

    let err = nup_pdf_pages(
        pdf,
        &NUpOptions {
            columns: 0,
            rows: 2,
        },
    )
    .unwrap_err();

    assert!(matches!(err, OxideError::InvalidInput { .. }));
    assert!(err.to_string().contains("greater than zero"));
}

#[test]
fn booklet_pdf_pages_outputs_two_up_imposed_pages() {
    let pdf = include_bytes!("../../../tests/test.pdf");

    let booklet = booklet_pdf_pages(pdf, &BookletOptions::default()).unwrap();
    let document = lopdf::Document::load_mem(&booklet.bytes).unwrap();

    assert_eq!(document.get_pages().len(), 2);
    assert_eq!(page_xobject_count(&document, 1), 1);
    assert_eq!(page_xobject_count(&document, 2), 2);
    assert!(page_content_contains_operator(&document, 1, "Do"));
}

#[test]
fn add_pdf_page_numbers_writes_selected_page_content() {
    let pdf = include_bytes!("../../../tests/test.pdf");

    let numbered = add_pdf_page_numbers(
        pdf,
        &PageNumbersOptions {
            pages: Some("2-3".to_owned()),
            start: 7,
            prefix: "p".to_owned(),
            suffix: String::new(),
            font_size: 10.0,
            position: PageNumberPosition::BottomRight,
        },
    )
    .unwrap();
    let document = lopdf::Document::load_mem(&numbered.bytes).unwrap();

    assert!(!page_content_text_contains(&document, 1, "p7"));
    assert!(page_content_text_contains(&document, 2, "p7"));
    assert!(page_content_text_contains(&document, 3, "p8"));
}

#[test]
fn add_pdf_page_numbers_rejects_non_ascii_text() {
    let pdf = include_bytes!("../../../tests/test.pdf");

    let err = add_pdf_page_numbers(
        pdf,
        &PageNumbersOptions {
            prefix: "第".to_owned(),
            ..PageNumbersOptions::default()
        },
    )
    .unwrap_err();

    assert!(matches!(err, OxideError::InvalidInput { .. }));
    assert!(err.to_string().contains("ASCII"));
}

#[test]
fn metadata_set_delete_validate_and_report_are_deterministic() {
    let pdf = empty_page_pdf();
    let edited = edit_pdf_metadata(
        &pdf,
        &MetadataEditOptions {
            action: MetadataEditAction::Set,
            entries: metadata_entries([("title", "Quarterly Report"), ("author", "OxidePDF")]),
            keys: Vec::new(),
        },
        &ResourceLimits::default(),
    )
    .unwrap();

    let report = inspect_pdf_metadata(&edited.bytes, &MetadataInspectOptions::default()).unwrap();
    let report: serde_json::Value = serde_json::from_str(&report.text).unwrap();
    assert_eq!(report["valid"], true);
    assert_eq!(report["entries"]["title"], "Quarterly Report");
    assert_eq!(report["entries"]["author"], "OxidePDF");

    let deleted = edit_pdf_metadata(
        &edited.bytes,
        &MetadataEditOptions {
            action: MetadataEditAction::Delete,
            entries: Vec::new(),
            keys: vec!["author".to_owned()],
        },
        &ResourceLimits::default(),
    )
    .unwrap();
    let deleted_again = edit_pdf_metadata(
        &deleted.bytes,
        &MetadataEditOptions {
            action: MetadataEditAction::Delete,
            entries: Vec::new(),
            keys: vec!["author".to_owned()],
        },
        &ResourceLimits::default(),
    )
    .unwrap();
    assert_eq!(
        inspect_pdf_metadata(&deleted.bytes, &MetadataInspectOptions::default())
            .unwrap()
            .text,
        inspect_pdf_metadata(&deleted_again.bytes, &MetadataInspectOptions::default())
            .unwrap()
            .text
    );
}

#[test]
fn outline_set_get_and_delete_are_stable() {
    let pdf = empty_page_pdf();
    let outline = OutlineTree {
        items: vec![OutlineItem {
            title: "Chapter 1".to_owned(),
            page: 1,
            children: vec![OutlineItem {
                title: "Section 1.1".to_owned(),
                page: 1,
                children: Vec::new(),
            }],
        }],
    };

    let edited = edit_pdf_outline(
        &pdf,
        &OutlineEditOptions {
            action: OutlineEditAction::Set,
            tree: Some(outline.clone()),
        },
        &ResourceLimits::default(),
    )
    .unwrap();
    let report = inspect_pdf_outline(&edited.bytes, &OutlineInspectOptions::default()).unwrap();
    let report: OutlineTree = serde_json::from_str(&report.text).unwrap();
    assert_eq!(report, outline);

    let deleted = edit_pdf_outline(
        &edited.bytes,
        &OutlineEditOptions {
            action: OutlineEditAction::Delete,
            tree: None,
        },
        &ResourceLimits::default(),
    )
    .unwrap();
    let report = inspect_pdf_outline(&deleted.bytes, &OutlineInspectOptions::default()).unwrap();
    let report: OutlineTree = serde_json::from_str(&report.text).unwrap();
    assert!(report.items.is_empty());
}

#[test]
fn outline_inspection_rejects_unsupported_destinations_without_defaulting_page() {
    let err = inspect_pdf_outline(
        &pdf_with_named_outline_destination(),
        &OutlineInspectOptions::default(),
    )
    .unwrap_err();

    assert!(matches!(err, OxideError::UnsupportedPdfFeature { .. }));
    assert!(err.to_string().contains("outline"));
}

#[test]
fn attachments_add_list_extract_and_delete() {
    let pdf = empty_page_pdf();
    let edited = edit_pdf_attachment_artifacts(
        &[Artifact::pdf(&pdf), Artifact::bytes(b"attachment bytes")],
        &AttachmentEditOptions {
            action: AttachmentEditAction::Add,
            name: Some("note.txt".to_owned()),
            description: Some("Review note".to_owned()),
        },
        &ResourceLimits::default(),
    )
    .unwrap();

    let report = inspect_pdf_attachments(&edited.bytes, &AttachmentInspectOptions::default())
        .unwrap()
        .text;
    let report: serde_json::Value = serde_json::from_str(&report).unwrap();
    assert_eq!(report["attachments"][0]["name"], "note.txt");
    assert_eq!(report["attachments"][0]["description"], "Review note");
    assert_eq!(report["attachments"][0]["size"], 16);

    let extracted =
        extract_pdf_attachment(&edited.bytes, "note.txt", &ResourceLimits::default()).unwrap();
    assert_eq!(extracted.bytes, b"attachment bytes");

    let deleted = edit_pdf_attachment_artifacts(
        &[Artifact::pdf(&edited.bytes)],
        &AttachmentEditOptions {
            action: AttachmentEditAction::Delete,
            name: Some("note.txt".to_owned()),
            description: None,
        },
        &ResourceLimits::default(),
    )
    .unwrap();
    let report = inspect_pdf_attachments(&deleted.bytes, &AttachmentInspectOptions::default())
        .unwrap()
        .text;
    let report: serde_json::Value = serde_json::from_str(&report).unwrap();
    assert!(report["attachments"].as_array().unwrap().is_empty());
}

#[test]
fn attachment_inspection_reports_malformed_names_tree() {
    let err = inspect_pdf_attachments(
        &pdf_with_malformed_names_tree(),
        &AttachmentInspectOptions::default(),
    )
    .unwrap_err();

    assert!(matches!(err, OxideError::ParsePdf));
}

#[test]
fn annotation_inspection_reports_malformed_annotation_array() {
    let err = inspect_pdf_annotations(
        &pdf_with_malformed_annotation_array(),
        &AnnotationInspectOptions::default(),
    )
    .unwrap_err();

    assert!(matches!(err, OxideError::ParsePdf));
}

#[test]
fn annotations_add_list_delete_and_interactive_removal_are_selective() {
    let pdf = empty_page_pdf();
    let annotated = edit_pdf_annotations(
        &pdf,
        &AnnotationEditOptions {
            action: AnnotationEditAction::AddText,
            page: Some(1),
            id: Some("review-note".to_owned()),
            text: Some("Review this page".to_owned()),
        },
        &ResourceLimits::default(),
    )
    .unwrap();
    let report = inspect_pdf_annotations(&annotated.bytes, &AnnotationInspectOptions::default())
        .unwrap()
        .text;
    let report: serde_json::Value = serde_json::from_str(&report).unwrap();
    assert_eq!(report["annotations"][0]["id"], "review-note");
    assert_eq!(report["annotations"][0]["text"], "Review this page");

    let removed = remove_pdf_interactive_elements(
        &annotated.bytes,
        &InteractiveRemovalOptions {
            annotations: true,
            forms: false,
            actions: false,
            javascript: false,
            embedded_files: false,
        },
        &ResourceLimits::default(),
    )
    .unwrap();
    let report = inspect_pdf_annotations(&removed.bytes, &AnnotationInspectOptions::default())
        .unwrap()
        .text;
    let report: serde_json::Value = serde_json::from_str(&report).unwrap();
    assert!(report["annotations"].as_array().unwrap().is_empty());
}

#[test]
fn interactive_removal_reports_malformed_names_tree() {
    let err = remove_pdf_interactive_elements(
        &pdf_with_malformed_names_tree(),
        &InteractiveRemovalOptions {
            annotations: false,
            forms: false,
            actions: false,
            javascript: true,
            embedded_files: false,
        },
        &ResourceLimits::default(),
    )
    .unwrap_err();

    assert!(matches!(err, OxideError::ParsePdf));
}

#[test]
fn form_inspection_reports_malformed_acroform() {
    let err = inspect_pdf_forms(
        &pdf_with_malformed_acroform(),
        &FormInspectOptions::default(),
    )
    .unwrap_err();

    assert!(matches!(err, OxideError::ParsePdf));
}

#[test]
fn forms_fill_unlock_and_remove_without_appearance_fallback() {
    let pdf = pdf_with_text_form_field(true);
    let filled = fill_pdf_form(
        &pdf,
        &FormFillOptions {
            fields: vec![FormFieldValue {
                name: "customer".to_owned(),
                value: "Ada".to_owned(),
            }],
        },
        &ResourceLimits::default(),
    )
    .unwrap();
    let report = inspect_pdf_forms(&filled.bytes, &FormInspectOptions::default())
        .unwrap()
        .text;
    let report: serde_json::Value = serde_json::from_str(&report).unwrap();
    assert_eq!(report["fields"][0]["name"], "customer");
    assert_eq!(report["fields"][0]["value"], "Ada");
    assert_eq!(report["fields"][0]["readonly"], true);

    let unlocked = unlock_pdf_form_readonly(&filled.bytes, &ResourceLimits::default()).unwrap();
    let report = inspect_pdf_forms(&unlocked.bytes, &FormInspectOptions::default())
        .unwrap()
        .text;
    let report: serde_json::Value = serde_json::from_str(&report).unwrap();
    assert_eq!(report["fields"][0]["readonly"], false);

    let removed = remove_pdf_forms(&unlocked.bytes, &ResourceLimits::default()).unwrap();
    let report = inspect_pdf_forms(&removed.bytes, &FormInspectOptions::default())
        .unwrap()
        .text;
    let report: serde_json::Value = serde_json::from_str(&report).unwrap();
    assert!(report["fields"].as_array().unwrap().is_empty());

    let xfa_err = fill_pdf_form(
        &pdf_with_xfa_form(),
        &FormFillOptions {
            fields: vec![FormFieldValue {
                name: "customer".to_owned(),
                value: "Ada".to_owned(),
            }],
        },
        &ResourceLimits::default(),
    )
    .unwrap_err();
    assert!(matches!(xfa_err, OxideError::UnsupportedPdfFeature { .. }));
    assert!(xfa_err.to_string().contains("XFA"));
}

#[test]
fn parses_overlay_image_color_operator_schema() {
    let workflow: Workflow = serde_yaml::from_str(
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
    let workflow: Workflow = serde_yaml::from_str(
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

#[test]
fn parses_security_operator_schema() {
    let workflow: Workflow = serde_yaml::from_str(
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

#[test]
fn overlay_pdf_page_and_signature_appearance_are_visual_only() {
    let pdf = empty_page_pdf();
    let overlay = include_bytes!("../../../tests/test.pdf");
    let overlaid = overlay_pdf_artifacts(
        &[Artifact::pdf(&pdf), Artifact::pdf(overlay)],
        &OverlayOptions {
            kind: OverlayKind::PdfPage,
            text: None,
            font: None,
            font_path: None,
            font_size: None,
            opacity: Some(1.0),
            rotation: None,
            position: Some("center".to_owned()),
            pages: Some("1".to_owned()),
            scale: Some(0.5),
            rasterize: false,
            source_page: Some(1),
        },
        &ResourceLimits::default(),
    )
    .unwrap();
    let document = lopdf::Document::load_mem(&overlaid.bytes).unwrap();
    assert!(page_resources(&document, 1).has(b"XObject"));
    assert!(page_content_contains_operator(&document, 1, "Do"));

    let appearance = overlay_pdf_artifacts(
        &[Artifact::pdf(&pdf)],
        &OverlayOptions {
            kind: OverlayKind::SignatureAppearance,
            text: Some("Ada Lovelace".to_owned()),
            font: Some("Helvetica".to_owned()),
            font_path: None,
            font_size: Some(24.0),
            opacity: Some(1.0),
            rotation: None,
            position: Some("bottom_right".to_owned()),
            pages: Some("1".to_owned()),
            scale: None,
            rasterize: false,
            source_page: None,
        },
        &ResourceLimits::default(),
    )
    .unwrap();
    let trust_anchors = write_test_trust_anchors("signature_appearance_report");
    let report = verify_pdf_signatures(
        &appearance.bytes,
        &SignatureOptions {
            mode: SignatureMode::Verify,
            trust_anchors: Some(trust_anchors),
        },
        &ResourceLimits::default(),
    )
    .unwrap();
    let report: serde_json::Value = serde_json::from_str(&report.text).unwrap();
    assert!(report["signatures"].as_array().unwrap().is_empty());
}

#[test]
fn image_resources_list_add_replace_delete_and_extract() {
    let image = include_bytes!("../../../tests/test.jpg");
    let pdf = image_artifacts_to_pdf(
        &[Artifact::image(image)],
        &ImageToPdfOptions {
            layout: Some("original_size".to_owned()),
        },
        &ResourceLimits::default(),
    )
    .unwrap();

    let report = inspect_pdf_images(&pdf.bytes, &ImageInspectOptions::default())
        .unwrap()
        .text;
    let report: serde_json::Value = serde_json::from_str(&report).unwrap();
    assert_eq!(report["images"][0]["name"], "Im1");

    let extracted = extract_pdf_image(
        &pdf.bytes,
        &ImageExtractOptions {
            name: "Im1".to_owned(),
        },
        &ResourceLimits::default(),
    )
    .unwrap();
    assert!(!extracted.bytes.is_empty());

    let added = edit_pdf_images_artifacts(
        &[Artifact::pdf(&empty_page_pdf()), Artifact::image(image)],
        &ImageEditOptions {
            action: ImageEditAction::Add,
            name: Some("Logo".to_owned()),
            page: Some(1),
        },
        &ResourceLimits::default(),
    )
    .unwrap();
    let added_doc = lopdf::Document::load_mem(&added.bytes).unwrap();
    assert!(page_resources(&added_doc, 1).has(b"XObject"));

    let replaced = edit_pdf_images_artifacts(
        &[Artifact::pdf(&added.bytes), Artifact::image(image)],
        &ImageEditOptions {
            action: ImageEditAction::Replace,
            name: Some("Logo".to_owned()),
            page: None,
        },
        &ResourceLimits::default(),
    )
    .unwrap();
    let report = inspect_pdf_images(&replaced.bytes, &ImageInspectOptions::default())
        .unwrap()
        .text;
    let report: serde_json::Value = serde_json::from_str(&report).unwrap();
    assert_eq!(report["images"][0]["name"], "Logo");

    let deleted = edit_pdf_images_artifacts(
        &[Artifact::pdf(&replaced.bytes)],
        &ImageEditOptions {
            action: ImageEditAction::Delete,
            name: Some("Logo".to_owned()),
            page: None,
        },
        &ResourceLimits::default(),
    )
    .unwrap();
    let report = inspect_pdf_images(&deleted.bytes, &ImageInspectOptions::default())
        .unwrap()
        .text;
    let report: serde_json::Value = serde_json::from_str(&report).unwrap();
    assert!(report["images"].as_array().unwrap().is_empty());
}

#[test]
fn compress_pdf_lossless_prunes_and_preserves_pages() {
    let pdf = pdf_with_unreferenced_stream_object();
    let before = lopdf::Document::load_mem(&pdf).unwrap();
    assert_eq!(before.get_pages().len(), 1);
    assert_eq!(before.objects.len(), 4);

    let compressed = compress_pdf(
        &pdf,
        &CompressionOptions::default(),
        &ResourceLimits::default(),
    )
    .unwrap();
    let after = lopdf::Document::load_mem(&compressed.bytes).unwrap();

    assert_eq!(after.get_pages().len(), 1);
    assert_eq!(after.objects.len(), 3);
    assert!(compressed.bytes.starts_with(b"%PDF-"));
}

#[test]
fn compress_pdf_recompresses_plain_content_streams() {
    let pdf = pdf_with_large_plain_content_stream();
    let before = lopdf::Document::load_mem(&pdf).unwrap();
    let before_stream = first_page_content_stream(&before);
    assert!(!before_stream.dict.has(b"Filter"));

    let compressed = compress_pdf(
        &pdf,
        &CompressionOptions::default(),
        &ResourceLimits::default(),
    )
    .unwrap();
    let after = lopdf::Document::load_mem(&compressed.bytes).unwrap();
    let after_stream = first_page_content_stream(&after);

    assert_eq!(
        after_stream.dict.get(b"Filter").unwrap().as_name().unwrap(),
        b"FlateDecode"
    );
    assert_eq!(
        after_stream.get_plain_content().unwrap(),
        before_stream.content
    );
}

#[test]
fn compress_pdf_merges_duplicate_image_resources_without_reencoding() {
    let pdf = pdf_with_duplicate_image_resources();
    let before = lopdf::Document::load_mem(&pdf).unwrap();
    let (left_before, right_before) = duplicate_image_resource_ids(&before);
    assert_ne!(left_before, right_before);

    let compressed = compress_pdf(
        &pdf,
        &CompressionOptions::default(),
        &ResourceLimits::default(),
    )
    .unwrap();
    let after = lopdf::Document::load_mem(&compressed.bytes).unwrap();
    let (left_after, right_after) = duplicate_image_resource_ids(&after);

    assert_eq!(after.get_pages().len(), 1);
    assert_eq!(left_after, right_after);
    let image_stream = after.get_object(left_after).unwrap().as_stream().unwrap();
    assert_eq!(image_stream.content, b"rgbpixel!");
    assert!(!image_stream.dict.has(b"Filter"));
}

#[test]
fn compress_pdf_lossless_keeps_jpeg_image_streams() {
    let image = include_bytes!("../../../tests/test.jpg");
    let pdf = image_artifacts_to_pdf(
        &[Artifact::image(image)],
        &ImageToPdfOptions {
            layout: Some("original_size".to_owned()),
        },
        &ResourceLimits::default(),
    )
    .unwrap();

    let compressed = compress_pdf(
        &pdf.bytes,
        &CompressionOptions::default(),
        &ResourceLimits::default(),
    )
    .unwrap();
    let document = lopdf::Document::load_mem(&compressed.bytes).unwrap();

    assert_eq!(document.get_pages().len(), 1);
}

#[test]
fn compress_pdf_rejects_lossless_image_options() {
    let err = compress_pdf(
        &empty_page_pdf(),
        &CompressionOptions {
            mode: CompressionMode::Lossless,
            images: Some(CompressionImageOptions {
                quality: Some(80),
                max_width: None,
                max_height: None,
                format: None,
            }),
        },
        &ResourceLimits::default(),
    )
    .unwrap_err();

    assert_eq!(
        err,
        OxideError::InvalidInput {
            reason: "lossless compression does not accept image resampling or reencoding options"
                .to_owned()
        }
    );
}

#[test]
fn compress_pdf_rejects_lossy_without_explicit_image_options() {
    let err = compress_pdf(
        &empty_page_pdf(),
        &CompressionOptions {
            mode: CompressionMode::Lossy,
            images: None,
        },
        &ResourceLimits::default(),
    )
    .unwrap_err();

    assert_eq!(
        err,
        OxideError::InvalidInput {
            reason: "lossy compression requires explicit image options".to_owned()
        }
    );
}

#[test]
fn compress_pdf_reencodes_images_when_lossy_options_are_explicit() {
    let compressed = compress_pdf(
        &pdf_with_duplicate_image_resources(),
        &CompressionOptions {
            mode: CompressionMode::Lossy,
            images: Some(CompressionImageOptions {
                quality: Some(80),
                max_width: Some(1),
                max_height: None,
                format: Some(CompressionImageFormat::Jpeg),
            }),
        },
        &ResourceLimits::default(),
    )
    .unwrap();
    let document = lopdf::Document::load_mem(&compressed.bytes).unwrap();
    let (left, right) = duplicate_image_resource_ids(&document);
    assert_eq!(left, right);
    let image_stream = document.get_object(left).unwrap().as_stream().unwrap();
    assert_eq!(
        image_stream.dict.get(b"Filter").unwrap().as_name().unwrap(),
        b"DCTDecode"
    );
    assert!(image_stream.content.starts_with(&[0xFF, 0xD8]));
}

#[test]
fn compress_pdf_returns_unsupported_for_non_jpeg_lossy_target() {
    let err = compress_pdf(
        &pdf_with_duplicate_image_resources(),
        &CompressionOptions {
            mode: CompressionMode::Lossy,
            images: Some(CompressionImageOptions {
                quality: Some(80),
                max_width: None,
                max_height: None,
                format: Some(CompressionImageFormat::Png),
            }),
        },
        &ResourceLimits::default(),
    )
    .unwrap_err();

    assert_eq!(
        err,
        OxideError::UnsupportedPdfFeature {
            feature: "lossy image target formats other than jpeg".to_owned()
        }
    );
}

#[test]
fn compress_pdf_rejects_unsupported_stream_filter() {
    let err = compress_pdf(
        &pdf_with_unsupported_filtered_stream(),
        &CompressionOptions::default(),
        &ResourceLimits::default(),
    )
    .unwrap_err();

    assert_eq!(
        err,
        OxideError::UnsupportedPdfFeature {
            feature: "stream filter 'DCTDecode'".to_owned()
        }
    );
}

#[test]
fn pdf_security_encrypts_with_aes256_and_decrypts_with_correct_password() {
    let pdf = include_bytes!("../../../tests/test.pdf");
    let encrypted = encrypt_pdf(
        pdf,
        &SecurityEncryptOptions {
            owner_password: "owner-pass".to_owned(),
            user_password: "user-pass".to_owned(),
            algorithm: EncryptionAlgorithm::Aes256,
            permissions: PermissionPolicy::default(),
        },
        &ResourceLimits::default(),
    )
    .unwrap();

    let encrypted_document = lopdf::Document::load_mem(&encrypted.bytes).unwrap();
    assert!(encrypted_document.is_encrypted());
    let encrypted_dict = encrypted_document.get_encrypted().unwrap();
    assert_eq!(encrypted_dict.get(b"V").unwrap().as_i64().unwrap(), 5);
    assert_eq!(encrypted_dict.get(b"R").unwrap().as_i64().unwrap(), 6);

    let decrypted = decrypt_pdf(
        &encrypted.bytes,
        &SecurityDecryptOptions {
            password: Some("user-pass".to_owned()),
        },
        &ResourceLimits::default(),
    )
    .unwrap();
    let decrypted_document = lopdf::Document::load_mem(&decrypted.bytes).unwrap();
    assert!(!decrypted_document.is_encrypted());
    assert_eq!(decrypted_document.get_pages().len(), 3);
}

#[test]
fn pdf_security_rejects_wrong_or_missing_password_without_decrypted_output() {
    let encrypted = encrypt_pdf(
        &empty_page_pdf(),
        &SecurityEncryptOptions {
            owner_password: "owner-pass".to_owned(),
            user_password: "user-pass".to_owned(),
            algorithm: EncryptionAlgorithm::Aes256,
            permissions: PermissionPolicy::default(),
        },
        &ResourceLimits::default(),
    )
    .unwrap();

    let wrong = decrypt_pdf(
        &encrypted.bytes,
        &SecurityDecryptOptions {
            password: Some("wrong-pass".to_owned()),
        },
        &ResourceLimits::default(),
    )
    .unwrap_err();
    assert_eq!(wrong, OxideError::IncorrectPassword);

    let missing = decrypt_pdf(
        &encrypted.bytes,
        &SecurityDecryptOptions { password: None },
        &ResourceLimits::default(),
    )
    .unwrap_err();
    assert_eq!(missing, OxideError::EncryptedPdf);
}

#[test]
fn pdf_security_reports_and_sets_permission_policy() {
    let restricted = PermissionPolicy {
        print: true,
        modify: false,
        copy: false,
        annotate: false,
        fill_forms: true,
        accessibility: true,
        assemble: false,
        high_quality_print: true,
    };
    let encrypted = encrypt_pdf(
        &empty_page_pdf(),
        &SecurityEncryptOptions {
            owner_password: "owner-pass".to_owned(),
            user_password: "user-pass".to_owned(),
            algorithm: EncryptionAlgorithm::Aes256,
            permissions: restricted.clone(),
        },
        &ResourceLimits::default(),
    )
    .unwrap();

    let report_text = inspect_pdf_permissions(
        &encrypted.bytes,
        &SecurityPermissionGetOptions {
            password: Some("owner-pass".to_owned()),
        },
        &ResourceLimits::default(),
    )
    .unwrap();
    let report: PermissionReport = serde_json::from_str(&report_text.text).unwrap();
    assert!(report.encrypted);
    assert_eq!(report.revision, Some(6));
    assert_eq!(report.permissions.copy, restricted.copy);
    assert_eq!(report.permissions.modify, restricted.modify);
    assert_eq!(report.permissions.fill_forms, restricted.fill_forms);

    let updated = set_pdf_permissions(
        &encrypted.bytes,
        &SecurityPermissionSetOptions {
            owner_password: "owner-pass".to_owned(),
            user_password: "new-user-pass".to_owned(),
            algorithm: EncryptionAlgorithm::Aes256,
            permissions: PermissionPolicy {
                copy: true,
                modify: true,
                ..restricted
            },
        },
        &ResourceLimits::default(),
    )
    .unwrap();
    let updated_report: PermissionReport = serde_json::from_str(
        &inspect_pdf_permissions(
            &updated.bytes,
            &SecurityPermissionGetOptions {
                password: Some("new-user-pass".to_owned()),
            },
            &ResourceLimits::default(),
        )
        .unwrap()
        .text,
    )
    .unwrap();
    assert!(updated_report.permissions.copy);
    assert!(updated_report.permissions.modify);
}

#[test]
fn pdf_security_rejects_rc4_and_unsupported_revisions() {
    let rc4 = encrypt_pdf(
        &empty_page_pdf(),
        &SecurityEncryptOptions {
            owner_password: "owner-pass".to_owned(),
            user_password: "user-pass".to_owned(),
            algorithm: EncryptionAlgorithm::Rc4,
            permissions: PermissionPolicy::default(),
        },
        &ResourceLimits::default(),
    )
    .unwrap_err();
    assert!(matches!(rc4, OxideError::UnsupportedPdfFeature { .. }));

    let unsupported = encrypted_pdf_with_revision(2);
    let err = decrypt_pdf(
        &unsupported,
        &SecurityDecryptOptions {
            password: Some("user-pass".to_owned()),
        },
        &ResourceLimits::default(),
    )
    .unwrap_err();
    assert!(matches!(err, OxideError::UnsupportedPdfFeature { .. }));
}

#[test]
fn image_resources_reject_malformed_xobject_dictionary() {
    let pdf = pdf_with_malformed_xobject_resources();

    let err = inspect_pdf_images(&pdf, &ImageInspectOptions::default()).unwrap_err();
    assert!(matches!(err, OxideError::ParsePdf));

    let err = edit_pdf_images_artifacts(
        &[Artifact::pdf(&pdf)],
        &ImageEditOptions {
            action: ImageEditAction::Delete,
            name: Some("Logo".to_owned()),
            page: None,
        },
        &ResourceLimits::default(),
    )
    .unwrap_err();
    assert!(matches!(err, OxideError::ParsePdf));
}

#[test]
fn color_operations_rewrite_simple_content_and_reject_rasterize_pages() {
    let pdf = pdf_with_rgb_fill_content();
    let inverted = edit_pdf_colors(
        &pdf,
        &ColorEditOptions {
            action: ColorEditAction::Invert,
            pages: Some("1".to_owned()),
            from: None,
            to: None,
            factor: None,
            rasterize_pages: false,
        },
        &ResourceLimits::default(),
    )
    .unwrap();
    let document = lopdf::Document::load_mem(&inverted.bytes).unwrap();
    assert_eq!(page_rgb_operator(&document, 1, "rg"), Some([0.0, 1.0, 1.0]));

    let replaced = edit_pdf_colors(
        &pdf,
        &ColorEditOptions {
            action: ColorEditAction::Replace,
            pages: Some("1".to_owned()),
            from: Some([1.0, 0.0, 0.0]),
            to: Some([0.0, 0.0, 1.0]),
            factor: None,
            rasterize_pages: false,
        },
        &ResourceLimits::default(),
    )
    .unwrap();
    let document = lopdf::Document::load_mem(&replaced.bytes).unwrap();
    assert_eq!(page_rgb_operator(&document, 1, "rg"), Some([0.0, 0.0, 1.0]));

    let err = edit_pdf_colors(
        &pdf,
        &ColorEditOptions {
            action: ColorEditAction::Contrast,
            pages: None,
            from: None,
            to: None,
            factor: Some(1.25),
            rasterize_pages: true,
        },
        &ResourceLimits::default(),
    )
    .unwrap_err();
    assert!(matches!(err, OxideError::UnsupportedPdfFeature { .. }));
    assert!(err.to_string().contains("rasterize_pages"));
}

#[test]
fn pdf_operator_runner_handles_page_editing_tasks() {
    let pdf = include_bytes!("../../../tests/test.pdf");
    let mut runner = PdfOperatorRunner::default();

    let merged = runner
        .run(
            &TaskSpec {
                id: TaskId::new("merge"),
                op: OperatorSpec::PdfEdit(PdfEditOptions::Merge(MergeOptions {})),
                inputs: vec![artifact_ref("a"), artifact_ref("b")],
            },
            &[Artifact::pdf(pdf), Artifact::pdf(pdf)],
        )
        .unwrap();

    assert!(matches!(merged, Artifact::Pdf(_)));
}

#[test]
fn pdf_operator_runner_enforces_output_size_limit() {
    let pdf = include_bytes!("../../../tests/test.pdf");
    let mut runner = PdfOperatorRunner::with_limits(ResourceLimits {
        max_output_bytes: Some(1),
        ..ResourceLimits::default()
    });

    let err = runner
        .run(
            &TaskSpec {
                id: TaskId::new("split"),
                op: OperatorSpec::PdfEdit(PdfEditOptions::KeepPages(SplitOptions {
                    pages: "1".to_owned(),
                })),
                inputs: vec![artifact_ref("source")],
            },
            &[Artifact::pdf(pdf)],
        )
        .unwrap_err();

    assert_eq!(
        err,
        OxideError::ResourceLimitExceeded {
            limit: "max_output_bytes".to_owned()
        }
    );
}

#[test]
fn pdf_operator_runner_emits_signature_verification_report() {
    let pdf = pdf_with_signature_dictionary(vec![0, 64, 192, 64], vec![0x30, 0x82]);
    let trust_anchors = write_test_trust_anchors("signature_report");
    let mut runner = PdfOperatorRunner::default();

    let artifact = runner
        .run(
            &TaskSpec {
                id: TaskId::new("verify"),
                op: OperatorSpec::PdfSign(PdfSignOptions::Verify(SignatureOptions {
                    mode: SignatureMode::Verify,
                    trust_anchors: Some(trust_anchors),
                })),
                inputs: vec![artifact_ref("source")],
            },
            &[Artifact::pdf(&pdf)],
        )
        .unwrap();

    let Artifact::Text(report_text) = artifact else {
        panic!("signature verification should emit a text JSON report");
    };
    let report: SignatureVerificationReport = serde_json::from_str(&report_text.text).unwrap();
    assert_eq!(report.trust_anchor_count, 1);
    assert_eq!(report.verdict, SignatureVerdict::Unsupported);
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
        SignatureCheckState::Unsupported
    );
    assert_eq!(
        report.signatures[0].revocation_status.status,
        SignatureCheckState::Indeterminate
    );
}

#[test]
fn verify_pdf_signatures_requires_explicit_trust_anchors() {
    let pdf = include_bytes!("../../../tests/fixtures/signature-placeholder.pdf");

    let err = verify_pdf_signatures(
        pdf,
        &SignatureOptions::default(),
        &ResourceLimits::default(),
    )
    .unwrap_err();

    assert_eq!(
        err,
        OxideError::InvalidInput {
            reason: "signature verification requires explicit trust anchors".to_owned()
        }
    );
}

#[test]
fn verify_pdf_signatures_rejects_empty_trust_anchor_file() {
    let pdf = include_bytes!("../../../tests/fixtures/signature-placeholder.pdf");
    let trust_anchors = write_empty_trust_anchors("empty_signature_anchors");

    let err = verify_pdf_signatures(
        pdf,
        &SignatureOptions {
            mode: SignatureMode::Verify,
            trust_anchors: Some(trust_anchors),
        },
        &ResourceLimits::default(),
    )
    .unwrap_err();

    assert_eq!(
        err,
        OxideError::InvalidInput {
            reason: "trust anchors file contains no valid PEM certificates".to_owned()
        }
    );
}

#[test]
fn verify_pdf_signatures_rejects_invalid_trust_anchor_certificate() {
    let pdf = include_bytes!("../../../tests/fixtures/signature-placeholder.pdf");
    let trust_anchors = write_invalid_trust_anchors("invalid_signature_anchors");

    let err = verify_pdf_signatures(
        pdf,
        &SignatureOptions {
            mode: SignatureMode::Verify,
            trust_anchors: Some(trust_anchors),
        },
        &ResourceLimits::default(),
    )
    .unwrap_err();

    assert_eq!(
        err,
        OxideError::InvalidInput {
            reason: "trust anchors file contains no valid PEM certificates".to_owned()
        }
    );
}

#[test]
fn verify_pdf_signatures_reports_unsigned_pdf_as_indeterminate() {
    let pdf = include_bytes!("../../../tests/test.pdf");
    let trust_anchors = write_test_trust_anchors("unsigned_pdf_report");

    let report = verify_pdf_signatures(
        pdf,
        &SignatureOptions {
            mode: SignatureMode::Verify,
            trust_anchors: Some(trust_anchors),
        },
        &ResourceLimits::default(),
    )
    .unwrap();
    let report: SignatureVerificationReport = serde_json::from_str(&report.text).unwrap();

    assert_eq!(report.verdict, SignatureVerdict::Indeterminate);
    assert_eq!(report.trust_anchor_count, 1);
    assert!(report.signatures.is_empty());
    assert_eq!(report.diagnostics.len(), 1);
    assert_eq!(report.diagnostics[0].code, "no_signatures");
}

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
    assert!(report.signatures[0]
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "byte_range_not_ordered"));
}

#[test]
fn signature_research_scanner_finds_signature_markers() {
    let pdf = include_bytes!("../../../tests/fixtures/signature-placeholder.pdf");

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

#[test]
fn image_artifacts_to_pdf_converts_real_jpeg() {
    let image = include_bytes!("../../../tests/test.jpg");

    let pdf = image_artifacts_to_pdf(
        &[Artifact::image(image)],
        &ImageToPdfOptions::default(),
        &ResourceLimits::default(),
    )
    .unwrap();
    let document = lopdf::Document::load_mem(&pdf.bytes).unwrap();

    assert_eq!(document.get_pages().len(), 1);
}

#[test]
fn image_artifacts_to_pdf_writes_one_page_per_image() {
    let image = include_bytes!("../../../tests/test.jpg");

    let pdf = image_artifacts_to_pdf(
        &[Artifact::image(image), Artifact::image(image)],
        &ImageToPdfOptions::default(),
        &ResourceLimits::default(),
    )
    .unwrap();
    let document = lopdf::Document::load_mem(&pdf.bytes).unwrap();

    assert_eq!(document.get_pages().len(), 2);
}

#[test]
fn image_artifacts_to_pdf_enforces_pixel_limit() {
    let image = include_bytes!("../../../tests/test.jpg");
    let limits = ResourceLimits {
        max_pixels: Some(1),
        ..ResourceLimits::default()
    };

    let err = image_artifacts_to_pdf(
        &[Artifact::image(image)],
        &ImageToPdfOptions::default(),
        &limits,
    )
    .unwrap_err();

    assert_eq!(
        err,
        OxideError::ResourceLimitExceeded {
            limit: "max_pixels".to_owned()
        }
    );
}

#[test]
fn image_artifacts_to_pdf_rejects_unknown_image_format() {
    let err = image_artifacts_to_pdf(
        &[Artifact::image(b"not an image")],
        &ImageToPdfOptions::default(),
        &ResourceLimits::default(),
    )
    .unwrap_err();

    assert_eq!(err, OxideError::ImageDecode);
}

#[test]
fn image_artifacts_to_pdf_enforces_output_size_limit() {
    let image = include_bytes!("../../../tests/test.jpg");

    let err = image_artifacts_to_pdf(
        &[Artifact::image(image)],
        &ImageToPdfOptions::default(),
        &ResourceLimits {
            max_output_bytes: Some(1),
            ..ResourceLimits::default()
        },
    )
    .unwrap_err();

    assert_eq!(
        err,
        OxideError::ResourceLimitExceeded {
            limit: "max_output_bytes".to_owned()
        }
    );
}

#[test]
fn svg_to_pdf_converts_vector_svg_to_parseable_pdf() {
    let svg = br##"<svg xmlns="http://www.w3.org/2000/svg" width="120" height="80">
            <rect x="10" y="10" width="100" height="60" fill="#0077cc"/>
        </svg>"##;

    let pdf = svg_to_pdf(svg, &SvgToPdfOptions::default(), &ResourceLimits::default()).unwrap();
    let document = lopdf::Document::load_mem(&pdf.bytes).unwrap();

    assert_eq!(document.get_pages().len(), 1);
}

#[test]
fn svg_to_pdf_rasterizes_only_when_requested() {
    let svg = br##"<svg xmlns="http://www.w3.org/2000/svg" width="120" height="80">
            <circle cx="60" cy="40" r="30" fill="#ef4444"/>
        </svg>"##;

    let pdf = svg_to_pdf(
        svg,
        &SvgToPdfOptions { rasterize: true },
        &ResourceLimits::default(),
    )
    .unwrap();
    let document = lopdf::Document::load_mem(&pdf.bytes).unwrap();

    assert_eq!(document.get_pages().len(), 1);
}

#[test]
fn svg_to_pdf_rejects_invalid_svg() {
    let err = svg_to_pdf(
        b"<svg><broken>",
        &SvgToPdfOptions::default(),
        &ResourceLimits::default(),
    )
    .unwrap_err();

    assert_eq!(err, OxideError::SvgParse);
}

#[test]
fn svg_to_pdf_rejects_non_svg_magic_bytes() {
    let err = svg_to_pdf(
        b"%PDF-1.7\nnot svg",
        &SvgToPdfOptions::default(),
        &ResourceLimits::default(),
    )
    .unwrap_err();

    assert_eq!(err, OxideError::SvgParse);
}

#[test]
fn render_pdf_page_writes_png_for_real_pdf() {
    let pdf = include_bytes!("../../../tests/test.pdf");

    let image = render_pdf_page(
        pdf,
        &RenderOptions {
            page: 1,
            format: Some("png".to_owned()),
            scale: Some(1.0),
        },
        &ResourceLimits::default(),
    )
    .unwrap();
    let decoded = image::load_from_memory(&image.bytes).unwrap();

    assert!(decoded.width() > 0);
    assert!(decoded.height() > 0);
}

#[test]
fn render_pdf_page_rejects_out_of_range_page() {
    let pdf = include_bytes!("../../../tests/test.pdf");

    let err = render_pdf_page(
        pdf,
        &RenderOptions {
            page: 99,
            format: Some("png".to_owned()),
            scale: Some(1.0),
        },
        &ResourceLimits::default(),
    )
    .unwrap_err();

    assert!(matches!(err, OxideError::InvalidInput { .. }));
    assert!(err.to_string().contains("page 99 is out of range"));
}

#[test]
fn extract_text_from_pdf_returns_plain_text_for_real_pdf() {
    let pdf = include_bytes!("../../../tests/test.pdf");

    let text = extract_text_from_pdf(
        pdf,
        &ExtractTextOptions::default(),
        &ResourceLimits::default(),
    )
    .unwrap();

    assert!(!text.text.trim().is_empty());
    assert!(text.diagnostics.is_empty());
}

#[test]
fn extract_text_from_pdf_rejects_pdf_without_text_layer() {
    let pdf = empty_page_pdf();

    let err = extract_text_from_pdf(
        &pdf,
        &ExtractTextOptions::default(),
        &ResourceLimits::default(),
    )
    .unwrap_err();

    assert!(matches!(err, OxideError::InvalidInput { .. }));
    assert!(err.to_string().contains("no extractable text layer"));
}

#[test]
fn extract_text_from_pdf_rejects_unknown_format() {
    let pdf = include_bytes!("../../../tests/test.pdf");

    let err = extract_text_from_pdf(
        pdf,
        &ExtractTextOptions {
            format: Some("json".to_owned()),
        },
        &ResourceLimits::default(),
    )
    .unwrap_err();

    assert!(matches!(err, OxideError::InvalidInput { .. }));
    assert!(err
        .to_string()
        .contains("unsupported text extraction format"));
}

#[test]
fn extract_text_from_pdf_rejects_non_pdf_magic_bytes() {
    let err = extract_text_from_pdf(
        b"<svg></svg>",
        &ExtractTextOptions::default(),
        &ResourceLimits::default(),
    )
    .unwrap_err();

    assert!(matches!(err, OxideError::InvalidInput { .. }));
    assert!(err.to_string().contains("expected PDF"));
}

#[test]
fn pdf_operator_runner_handles_extract_text_tasks() {
    let pdf = include_bytes!("../../../tests/test.pdf");
    let mut runner = PdfOperatorRunner::default();

    let extracted = runner
        .run(
            &TaskSpec {
                id: TaskId::new("extract"),
                op: OperatorSpec::PdfInspect(PdfInspectOptions::ExtractText(
                    ExtractTextOptions::default(),
                )),
                inputs: vec![artifact_ref("source")],
            },
            &[Artifact::pdf(pdf)],
        )
        .unwrap();

    let Artifact::Text(text) = extracted else {
        panic!("expected text artifact");
    };
    assert!(!text.text.trim().is_empty());
}

#[test]
fn execute_workflow_enforces_timeout() {
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
    store.insert(artifact_ref("source"), Artifact::bytes(b"input"));
    let mut runner = SlowRunner;

    let err = execute_workflow(&workflow, store, &mut runner).unwrap_err();

    assert_eq!(
        err,
        OxideError::ResourceLimitExceeded {
            limit: "timeout_ms".to_owned()
        }
    );
}

#[test]
fn execute_workflow_enforces_total_input_size_limit() {
    let workflow = workflow_from_json(
        r#"
            {
              "version": 1,
              "inputs": [
                { "id": "first", "path": "a.bin" },
                { "id": "second", "path": "b.bin" }
              ],
              "tasks": [],
              "outputs": [{ "id": "final", "from": "first", "path": "out.bin" }],
              "limits": { "max_total_input_bytes": 9 }
            }
            "#,
    );
    let mut store = ArtifactStore::new();
    store.insert(artifact_ref("first"), Artifact::bytes(b"12345"));
    store.insert(artifact_ref("second"), Artifact::bytes(b"67890"));
    let mut runner = RecordingRunner::default();

    let err = execute_workflow(&workflow, store, &mut runner).unwrap_err();

    assert_eq!(
        err,
        OxideError::ResourceLimitExceeded {
            limit: "max_total_input_bytes".to_owned()
        }
    );
    assert!(runner.executed.is_empty());
}

#[test]
fn watermark_pdf_adds_text_watermark_to_selected_page() {
    let pdf = include_bytes!("../../../tests/test.pdf");

    let watermarked = watermark_pdf_artifacts(
        &[Artifact::pdf(pdf)],
        &WatermarkOptions {
            kind: WatermarkKind::Text,
            text: Some("DRAFT".to_owned()),
            font: Some("DejaVu Sans".to_owned()),
            font_path: None,
            font_size: Some(36.0),
            opacity: Some(0.4),
            rotation: Some(30.0),
            position: Some("center".to_owned()),
            pages: Some("1".to_owned()),
            scale: None,
            rasterize: false,
        },
        &ResourceLimits::default(),
    )
    .unwrap();
    let document = lopdf::Document::load_mem(&watermarked.bytes).unwrap();

    assert_eq!(document.get_pages().len(), 3);
    assert!(page_resources(&document, 1).has(b"Font"));
    assert!(page_resources(&document, 1).has(b"ExtGState"));
    assert!(page_content_contains_operator(&document, 1, "Tj"));
    assert!(!page_content_contains_operator(&document, 2, "Tj"));
}

#[test]
fn watermark_pdf_rejects_missing_text_font_without_substitution() {
    let pdf = include_bytes!("../../../tests/test.pdf");

    let err = watermark_pdf_artifacts(
        &[Artifact::pdf(pdf)],
        &WatermarkOptions {
            kind: WatermarkKind::Text,
            text: Some("DRAFT".to_owned()),
            font: Some("Definitely Missing Font Family".to_owned()),
            font_path: None,
            font_size: Some(36.0),
            opacity: Some(0.4),
            rotation: None,
            position: Some("center".to_owned()),
            pages: Some("1".to_owned()),
            scale: None,
            rasterize: false,
        },
        &ResourceLimits::default(),
    )
    .unwrap_err();

    assert_eq!(err, OxideError::FontResolution);
}

#[test]
fn watermark_pdf_enforces_image_pixel_limit() {
    let pdf = include_bytes!("../../../tests/test.pdf");
    let image = include_bytes!("../../../tests/test.jpg");

    let err = watermark_pdf_artifacts(
        &[Artifact::pdf(pdf), Artifact::image(image)],
        &WatermarkOptions {
            kind: WatermarkKind::Image,
            text: None,
            font: None,
            font_path: None,
            font_size: None,
            opacity: None,
            rotation: None,
            position: None,
            pages: None,
            scale: None,
            rasterize: false,
        },
        &ResourceLimits {
            max_pixels: Some(1),
            ..ResourceLimits::default()
        },
    )
    .unwrap_err();

    assert_eq!(
        err,
        OxideError::ResourceLimitExceeded {
            limit: "max_pixels".to_owned()
        }
    );
}

#[test]
fn watermark_pdf_adds_image_watermark_to_selected_page() {
    let pdf = include_bytes!("../../../tests/test.pdf");
    let image = include_bytes!("../../../tests/test.jpg");

    let watermarked = watermark_pdf_artifacts(
        &[Artifact::pdf(pdf), Artifact::image(image)],
        &WatermarkOptions {
            kind: WatermarkKind::Image,
            text: None,
            font: None,
            font_path: None,
            font_size: None,
            opacity: Some(0.3),
            rotation: Some(15.0),
            position: Some("bottom_right".to_owned()),
            pages: Some("2".to_owned()),
            scale: Some(0.25),
            rasterize: false,
        },
        &ResourceLimits::default(),
    )
    .unwrap();
    let document = lopdf::Document::load_mem(&watermarked.bytes).unwrap();

    assert!(page_resources(&document, 2).has(b"XObject"));
    assert!(page_content_contains_operator(&document, 2, "Do"));
    assert!(!page_content_contains_operator(&document, 1, "Do"));
}

#[test]
fn watermark_pdf_adds_svg_watermark_as_vector_xobject() {
    let pdf = include_bytes!("../../../tests/test.pdf");
    let svg = simple_svg();

    let watermarked = watermark_pdf_artifacts(
        &[Artifact::pdf(pdf), Artifact::svg(svg)],
        &WatermarkOptions {
            kind: WatermarkKind::Svg,
            text: None,
            font: None,
            font_path: None,
            font_size: None,
            opacity: Some(0.5),
            rotation: None,
            position: Some("top_left".to_owned()),
            pages: Some("3".to_owned()),
            scale: Some(0.2),
            rasterize: false,
        },
        &ResourceLimits::default(),
    )
    .unwrap();
    let document = lopdf::Document::load_mem(&watermarked.bytes).unwrap();

    assert!(page_resources(&document, 3).has(b"XObject"));
    assert!(page_content_contains_operator(&document, 3, "Do"));
    assert!(page_xobject_subtypes(&document, 3).contains(&b"Form".to_vec()));
    let form_operators = page_form_xobject_operators(&document, 3);
    assert!(form_operators.iter().any(|operator| operator == "f"));
    assert!(!form_operators
        .windows(2)
        .any(|operators| operators == ["re", "S"]));
}

#[test]
fn watermark_pdf_rasterizes_svg_only_when_requested() {
    let pdf = include_bytes!("../../../tests/test.pdf");
    let svg = simple_svg();

    let watermarked = watermark_pdf_artifacts(
        &[Artifact::pdf(pdf), Artifact::svg(svg)],
        &WatermarkOptions {
            kind: WatermarkKind::Svg,
            text: None,
            font: None,
            font_path: None,
            font_size: None,
            opacity: Some(0.5),
            rotation: None,
            position: Some("top_left".to_owned()),
            pages: Some("1".to_owned()),
            scale: Some(0.2),
            rasterize: true,
        },
        &ResourceLimits::default(),
    )
    .unwrap();
    let document = lopdf::Document::load_mem(&watermarked.bytes).unwrap();

    assert!(page_xobject_subtypes(&document, 1).contains(&b"Image".to_vec()));
}

#[test]
fn watermark_pdf_rejects_malformed_svg_without_panic() {
    let pdf = include_bytes!("../../../tests/test.pdf");

    let err = watermark_pdf_artifacts(
        &[Artifact::pdf(pdf), Artifact::svg(b"<svg><broken>")],
        &WatermarkOptions {
            kind: WatermarkKind::Svg,
            text: None,
            font: None,
            font_path: None,
            font_size: None,
            opacity: None,
            rotation: None,
            position: None,
            pages: None,
            scale: None,
            rasterize: false,
        },
        &ResourceLimits::default(),
    )
    .unwrap_err();

    assert_eq!(err, OxideError::SvgParse);
}

fn workflow_from_json(json: &str) -> Workflow {
    serde_json::from_str(json).unwrap()
}

fn artifact_ref(value: &str) -> ArtifactRef {
    serde_json::from_str(&format!("{value:?}")).unwrap()
}

fn write_test_trust_anchors(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "oxidepdf_core_{name}_{}_anchors.pem",
        std::process::id()
    ));
    std::fs::write(
        &path,
        include_bytes!("../../../tests/fixtures/test-trust-anchor.txt"),
    )
    .unwrap();
    path
}

fn write_empty_trust_anchors(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "oxidepdf_core_{name}_{}_anchors.pem",
        std::process::id()
    ));
    std::fs::write(&path, "not a certificate bundle\n").unwrap();
    path
}

fn write_invalid_trust_anchors(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "oxidepdf_core_{name}_{}_anchors.pem",
        std::process::id()
    ));
    std::fs::write(
        &path,
        "-----BEGIN CERTIFICATE-----\nMIIB\n-----END CERTIFICATE-----\n",
    )
    .unwrap();
    path
}

fn encrypted_pdf_with_revision(revision: i64) -> Vec<u8> {
    let mut document = lopdf::Document::load_mem(&empty_page_pdf()).unwrap();
    let encrypt_id = document.new_object_id();
    document.objects.insert(
        encrypt_id,
        Object::Dictionary(lopdf::dictionary! {
            "Filter" => "Standard",
            "V" => 1,
            "R" => revision,
            "Length" => 40,
            "P" => -4,
            "O" => Object::string_literal(vec![0u8; 32]),
            "U" => Object::string_literal(vec![0u8; 32]),
        }),
    );
    document.trailer.set("Encrypt", encrypt_id);

    let mut bytes = Vec::new();
    document.save_to(&mut bytes).unwrap();
    bytes
}

fn pdf_with_signature_dictionary(byte_range: Vec<i64>, contents: Vec<u8>) -> Vec<u8> {
    let mut document = lopdf::Document::with_version("1.7");
    let pages_id = document.new_object_id();
    let page_id = document.new_object_id();
    let sig_field_id = document.new_object_id();
    let sig_value_id = document.new_object_id();
    let acroform_id = document.new_object_id();
    let catalog_id = document.new_object_id();

    let byte_range = byte_range
        .into_iter()
        .map(lopdf::Object::Integer)
        .collect::<Vec<_>>();
    let sig_value = lopdf::dictionary! {
        "Type" => "Sig",
        "Filter" => "Adobe.PPKLite",
        "SubFilter" => "adbe.pkcs7.detached",
        "ByteRange" => lopdf::Object::Array(byte_range),
        "Contents" => lopdf::Object::String(contents, lopdf::StringFormat::Hexadecimal),
    };
    document
        .objects
        .insert(sig_value_id, lopdf::Object::Dictionary(sig_value));

    let sig_field = lopdf::dictionary! {
        "Type" => "Annot",
        "Subtype" => "Widget",
        "FT" => "Sig",
        "T" => lopdf::Object::string_literal("Approval"),
        "V" => sig_value_id,
        "Rect" => lopdf::Object::Array(vec![0.into(), 0.into(), 0.into(), 0.into()]),
        "P" => page_id,
    };
    document
        .objects
        .insert(sig_field_id, lopdf::Object::Dictionary(sig_field));

    let page = lopdf::dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => lopdf::Object::Array(vec![0.into(), 0.into(), 200.into(), 200.into()]),
        "Annots" => lopdf::Object::Array(vec![sig_field_id.into()]),
    };
    document
        .objects
        .insert(page_id, lopdf::Object::Dictionary(page));

    let pages = lopdf::dictionary! {
        "Type" => "Pages",
        "Kids" => lopdf::Object::Array(vec![page_id.into()]),
        "Count" => 1,
    };
    document
        .objects
        .insert(pages_id, lopdf::Object::Dictionary(pages));

    let acroform = lopdf::dictionary! {
        "Fields" => lopdf::Object::Array(vec![sig_field_id.into()]),
    };
    document
        .objects
        .insert(acroform_id, lopdf::Object::Dictionary(acroform));

    let catalog = lopdf::dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
        "AcroForm" => acroform_id,
    };
    document
        .objects
        .insert(catalog_id, lopdf::Object::Dictionary(catalog));
    document.trailer.set("Root", catalog_id);

    let mut bytes = Vec::new();
    document.save_to(&mut bytes).unwrap();
    bytes
}

fn assert_page_numbers(document: &lopdf::Document, expected: &[u32]) {
    let pages = document.get_pages();
    let actual = pages.keys().copied().collect::<Vec<_>>();
    assert_eq!(actual, expected);
}

fn page_rotation(document: &lopdf::Document, page_number: u32) -> i64 {
    let page_id = document.get_pages().get(&page_number).copied().unwrap();
    let page = document.get_object(page_id).unwrap().as_dict().unwrap();
    page.get(b"Rotate")
        .and_then(lopdf::Object::as_i64)
        .unwrap_or(0)
}

fn page_optional_box(document: &lopdf::Document, page_number: u32, key: &[u8]) -> Option<[f32; 4]> {
    let page_id = document.get_pages().get(&page_number).copied().unwrap();
    let page = document.get_object(page_id).unwrap().as_dict().unwrap();
    let values = page.get(key).ok()?.as_array().ok()?;
    Some([
        object_to_f32(&values[0]).unwrap(),
        object_to_f32(&values[1]).unwrap(),
        object_to_f32(&values[2]).unwrap(),
        object_to_f32(&values[3]).unwrap(),
    ])
}

fn page_box(document: &lopdf::Document, page_number: u32, key: &[u8]) -> [f32; 4] {
    page_optional_box(document, page_number, key).unwrap()
}

fn page_content_contains(document: &lopdf::Document, page_number: u32, operator: &str) -> bool {
    let page_id = document.get_pages().get(&page_number).copied().unwrap();
    document
        .get_page_content(page_id)
        .ok()
        .and_then(|content| lopdf::content::Content::decode(&content).ok())
        .is_some_and(|content| {
            content
                .operations
                .iter()
                .any(|operation| operation.operator == operator)
        })
}

fn page_rgb_operator(
    document: &lopdf::Document,
    page_number: u32,
    operator: &str,
) -> Option<[f32; 3]> {
    let page_id = document.get_pages().get(&page_number).copied().unwrap();
    let content = document.get_page_content(page_id).ok()?;
    let content = lopdf::content::Content::decode(&content).ok()?;
    content.operations.iter().find_map(|operation| {
        if operation.operator == operator && operation.operands.len() == 3 {
            Some([
                object_to_f32(&operation.operands[0]).ok()?,
                object_to_f32(&operation.operands[1]).ok()?,
                object_to_f32(&operation.operands[2]).ok()?,
            ])
        } else {
            None
        }
    })
}

fn pdf_with_blank_and_marked_page() -> Vec<u8> {
    let mut document = lopdf::Document::with_version("1.7");
    let pages_id = document.new_object_id();
    let blank_page_id = document.new_object_id();
    let marked_page_id = document.new_object_id();
    let marked_content_id = document.new_object_id();
    let catalog_id = document.new_object_id();

    let marked_content = lopdf::content::Content {
        operations: vec![lopdf::content::Operation::new("q", vec![])],
    }
    .encode()
    .unwrap();
    document.objects.insert(
        marked_content_id,
        Object::Stream(Stream::new(Dictionary::new(), marked_content)),
    );
    document.objects.insert(
        blank_page_id,
        Object::Dictionary(lopdf::dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => Object::Array(vec![0.into(), 0.into(), 100.into(), 100.into()]),
        }),
    );
    document.objects.insert(
        marked_page_id,
        Object::Dictionary(lopdf::dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => Object::Array(vec![0.into(), 0.into(), 100.into(), 100.into()]),
            "Contents" => marked_content_id,
        }),
    );
    document.objects.insert(
        pages_id,
        Object::Dictionary(lopdf::dictionary! {
            "Type" => "Pages",
            "Kids" => Object::Array(vec![blank_page_id.into(), marked_page_id.into()]),
            "Count" => 2,
        }),
    );
    document.objects.insert(
        catalog_id,
        Object::Dictionary(lopdf::dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        }),
    );
    document.trailer.set("Root", catalog_id);

    let mut bytes = Vec::new();
    document.save_to(&mut bytes).unwrap();
    bytes
}

fn pdf_with_blank_page_and_missing_resources() -> Vec<u8> {
    let mut document = lopdf::Document::with_version("1.7");
    let pages_id = document.new_object_id();
    let page_id = document.new_object_id();
    let catalog_id = document.new_object_id();

    document.objects.insert(
        page_id,
        Object::Dictionary(lopdf::dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => Object::Array(vec![0.into(), 0.into(), 100.into(), 100.into()]),
            "Resources" => Object::Reference((99, 0)),
        }),
    );
    document.objects.insert(
        pages_id,
        Object::Dictionary(lopdf::dictionary! {
            "Type" => "Pages",
            "Kids" => Object::Array(vec![page_id.into()]),
            "Count" => 1,
        }),
    );
    document.objects.insert(
        catalog_id,
        Object::Dictionary(lopdf::dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        }),
    );
    document.trailer.set("Root", catalog_id);

    let mut bytes = Vec::new();
    document.save_to(&mut bytes).unwrap();
    bytes
}

fn empty_page_pdf() -> Vec<u8> {
    let mut pdf = pdf_writer::Pdf::new();
    let catalog_id = pdf_writer::Ref::new(1);
    let pages_id = pdf_writer::Ref::new(2);
    let page_id = pdf_writer::Ref::new(3);

    pdf.catalog(catalog_id).pages(pages_id);
    pdf.pages(pages_id).kids([page_id]).count(1);
    let mut page = pdf.page(page_id);
    page.media_box(pdf_writer::Rect::new(0.0, 0.0, A4_WIDTH, A4_HEIGHT));
    page.parent(pages_id);
    page.finish();

    pdf.finish()
}

fn pdf_with_unreferenced_stream_object() -> Vec<u8> {
    let mut document = lopdf::Document::load_mem(&empty_page_pdf()).unwrap();
    let unused_id = document.new_object_id();
    document.objects.insert(
        unused_id,
        Object::Stream(Stream::new(Dictionary::new(), b"unused".to_vec())),
    );

    let mut bytes = Vec::new();
    document.save_to(&mut bytes).unwrap();
    bytes
}

fn pdf_with_large_plain_content_stream() -> Vec<u8> {
    let content = b"0 0 0 rg\n0 0 100 100 re f\n".repeat(64);
    pdf_with_content_stream(Stream::new(Dictionary::new(), content))
}

fn pdf_with_unsupported_filtered_stream() -> Vec<u8> {
    let mut stream = Stream::new(Dictionary::new(), b"not jpeg data".to_vec());
    stream.dict.set("Filter", "DCTDecode");
    pdf_with_content_stream(stream)
}

fn pdf_with_content_stream(stream: Stream) -> Vec<u8> {
    let mut document = lopdf::Document::with_version("1.7");
    let pages_id = document.new_object_id();
    let page_id = document.new_object_id();
    let content_id = document.new_object_id();
    let catalog_id = document.new_object_id();

    document.objects.insert(content_id, Object::Stream(stream));
    document.objects.insert(
        page_id,
        Object::Dictionary(lopdf::dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => Object::Array(vec![0.into(), 0.into(), 100.into(), 100.into()]),
            "Contents" => content_id,
        }),
    );
    document.objects.insert(
        pages_id,
        Object::Dictionary(lopdf::dictionary! {
            "Type" => "Pages",
            "Kids" => Object::Array(vec![page_id.into()]),
            "Count" => 1,
        }),
    );
    document.objects.insert(
        catalog_id,
        Object::Dictionary(lopdf::dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        }),
    );
    document.trailer.set("Root", catalog_id);

    let mut bytes = Vec::new();
    document.save_to(&mut bytes).unwrap();
    bytes
}

fn pdf_with_duplicate_image_resources() -> Vec<u8> {
    let mut document = lopdf::Document::load_mem(&empty_page_pdf()).unwrap();
    let page_id = *document.get_pages().get(&1).unwrap();
    let left_id = document.add_object(test_image_stream());
    let right_id = document.add_object(test_image_stream());

    document
        .get_object_mut(page_id)
        .unwrap()
        .as_dict_mut()
        .unwrap()
        .set(
            "Resources",
            Object::Dictionary(lopdf::dictionary! {
                "XObject" => Object::Dictionary(lopdf::dictionary! {
                    "Left" => left_id,
                    "Right" => right_id,
                }),
            }),
        );

    let mut bytes = Vec::new();
    document.save_to(&mut bytes).unwrap();
    bytes
}

fn test_image_stream() -> Stream {
    Stream::new(
        lopdf::dictionary! {
            "Type" => "XObject",
            "Subtype" => "Image",
            "Width" => 1,
            "Height" => 3,
            "ColorSpace" => "DeviceRGB",
            "BitsPerComponent" => 8,
        },
        b"rgbpixel!".to_vec(),
    )
}

fn first_page_content_stream(document: &lopdf::Document) -> &Stream {
    let page_id = *document.get_pages().get(&1).unwrap();
    let content_id = document
        .get_dictionary(page_id)
        .unwrap()
        .get(b"Contents")
        .unwrap()
        .as_reference()
        .unwrap();
    document
        .get_object(content_id)
        .unwrap()
        .as_stream()
        .unwrap()
}

fn duplicate_image_resource_ids(document: &lopdf::Document) -> (lopdf::ObjectId, lopdf::ObjectId) {
    let resources = page_resources(document, 1);
    let xobjects = resources.get(b"XObject").unwrap().as_dict().unwrap();
    (
        xobjects.get(b"Left").unwrap().as_reference().unwrap(),
        xobjects.get(b"Right").unwrap().as_reference().unwrap(),
    )
}

fn metadata_entries<const N: usize>(entries: [(&str, &str); N]) -> Vec<MetadataEntry> {
    entries
        .into_iter()
        .map(|(key, value)| MetadataEntry {
            key: key.to_owned(),
            value: value.to_owned(),
        })
        .collect()
}

fn pdf_with_text_form_field(readonly: bool) -> Vec<u8> {
    let mut document = lopdf::Document::with_version("1.7");
    let pages_id = document.new_object_id();
    let page_id = document.new_object_id();
    let field_id = document.new_object_id();
    let acroform_id = document.new_object_id();
    let catalog_id = document.new_object_id();
    let flags = if readonly { 1 } else { 0 };

    document.objects.insert(
        field_id,
        Object::Dictionary(lopdf::dictionary! {
            "FT" => "Tx",
            "T" => Object::string_literal("customer"),
            "V" => Object::string_literal(""),
            "Ff" => flags,
            "Rect" => Object::Array(vec![10.into(), 10.into(), 120.into(), 30.into()]),
            "P" => page_id,
        }),
    );
    document.objects.insert(
        page_id,
        Object::Dictionary(lopdf::dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => Object::Array(vec![0.into(), 0.into(), 200.into(), 200.into()]),
            "Annots" => Object::Array(vec![field_id.into()]),
        }),
    );
    document.objects.insert(
        pages_id,
        Object::Dictionary(lopdf::dictionary! {
            "Type" => "Pages",
            "Kids" => Object::Array(vec![page_id.into()]),
            "Count" => 1,
        }),
    );
    document.objects.insert(
        acroform_id,
        Object::Dictionary(lopdf::dictionary! {
            "Fields" => Object::Array(vec![field_id.into()]),
        }),
    );
    document.objects.insert(
        catalog_id,
        Object::Dictionary(lopdf::dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
            "AcroForm" => acroform_id,
        }),
    );
    document.trailer.set("Root", catalog_id);

    let mut bytes = Vec::new();
    document.save_to(&mut bytes).unwrap();
    bytes
}

fn pdf_with_named_outline_destination() -> Vec<u8> {
    let mut document = lopdf::Document::load_mem(&empty_page_pdf()).unwrap();
    let outline_id = document.new_object_id();
    let item_id = document.new_object_id();
    document.objects.insert(
        item_id,
        Object::Dictionary(lopdf::dictionary! {
            "Title" => Object::string_literal("Named destination"),
            "Parent" => outline_id,
            "Dest" => Object::Name(b"named-destination".to_vec()),
        }),
    );
    document.objects.insert(
        outline_id,
        Object::Dictionary(lopdf::dictionary! {
            "Type" => "Outlines",
            "First" => item_id,
            "Last" => item_id,
            "Count" => 1,
        }),
    );
    document
        .catalog_mut()
        .unwrap()
        .set("Outlines", Object::Reference(outline_id));

    let mut bytes = Vec::new();
    document.save_to(&mut bytes).unwrap();
    bytes
}

fn pdf_with_malformed_names_tree() -> Vec<u8> {
    let mut document = lopdf::Document::load_mem(&empty_page_pdf()).unwrap();
    document
        .catalog_mut()
        .unwrap()
        .set("Names", Object::string_literal("malformed names tree"));

    let mut bytes = Vec::new();
    document.save_to(&mut bytes).unwrap();
    bytes
}

fn pdf_with_malformed_annotation_array() -> Vec<u8> {
    let mut document = lopdf::Document::load_mem(&empty_page_pdf()).unwrap();
    let page_id = *document.get_pages().get(&1).unwrap();
    document
        .get_object_mut(page_id)
        .unwrap()
        .as_dict_mut()
        .unwrap()
        .set("Annots", Object::string_literal("malformed annotations"));

    let mut bytes = Vec::new();
    document.save_to(&mut bytes).unwrap();
    bytes
}

fn pdf_with_malformed_acroform() -> Vec<u8> {
    let mut document = lopdf::Document::load_mem(&empty_page_pdf()).unwrap();
    document
        .catalog_mut()
        .unwrap()
        .set("AcroForm", Object::string_literal("malformed acroform"));

    let mut bytes = Vec::new();
    document.save_to(&mut bytes).unwrap();
    bytes
}

fn pdf_with_malformed_xobject_resources() -> Vec<u8> {
    let mut document = lopdf::Document::load_mem(&empty_page_pdf()).unwrap();
    let page_id = *document.get_pages().get(&1).unwrap();
    let page = document
        .get_object_mut(page_id)
        .unwrap()
        .as_dict_mut()
        .unwrap();
    page.set(
        "Resources",
        Object::Dictionary(lopdf::dictionary! {
            "XObject" => Object::string_literal("malformed xobject dictionary"),
        }),
    );

    let mut bytes = Vec::new();
    document.save_to(&mut bytes).unwrap();
    bytes
}

fn pdf_with_rgb_fill_content() -> Vec<u8> {
    let mut document = lopdf::Document::with_version("1.7");
    let pages_id = document.new_object_id();
    let page_id = document.new_object_id();
    let content_id = document.new_object_id();
    let catalog_id = document.new_object_id();
    let content = lopdf::content::Content {
        operations: vec![
            lopdf::content::Operation::new(
                "rg",
                vec![Object::Real(1.0), Object::Real(0.0), Object::Real(0.0)],
            ),
            lopdf::content::Operation::new(
                "re",
                vec![
                    Object::Integer(0),
                    Object::Integer(0),
                    Object::Integer(100),
                    Object::Integer(100),
                ],
            ),
            lopdf::content::Operation::new("f", Vec::new()),
        ],
    }
    .encode()
    .unwrap();
    document.objects.insert(
        content_id,
        Object::Stream(lopdf::Stream::new(lopdf::Dictionary::new(), content)),
    );
    document.objects.insert(
        page_id,
        Object::Dictionary(lopdf::dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => Object::Array(vec![0.into(), 0.into(), 100.into(), 100.into()]),
            "Contents" => content_id,
        }),
    );
    document.objects.insert(
        pages_id,
        Object::Dictionary(lopdf::dictionary! {
            "Type" => "Pages",
            "Kids" => Object::Array(vec![page_id.into()]),
            "Count" => 1,
        }),
    );
    document.objects.insert(
        catalog_id,
        Object::Dictionary(lopdf::dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        }),
    );
    document.trailer.set("Root", catalog_id);

    let mut bytes = Vec::new();
    document.save_to(&mut bytes).unwrap();
    bytes
}

fn pdf_with_xfa_form() -> Vec<u8> {
    let mut document = lopdf::Document::load_mem(&pdf_with_text_form_field(false)).unwrap();
    let catalog = document.catalog().unwrap();
    let acroform_id = catalog.get(b"AcroForm").unwrap().as_reference().unwrap();
    document
        .get_object_mut(acroform_id)
        .unwrap()
        .as_dict_mut()
        .unwrap()
        .set("XFA", Object::string_literal("xfa packet"));
    let mut bytes = Vec::new();
    document.save_to(&mut bytes).unwrap();
    bytes
}

fn page_resources(document: &lopdf::Document, page_number: u32) -> Dictionary {
    let page_id = document.get_pages().get(&page_number).copied().unwrap();
    let resources = document
        .get_dictionary(page_id)
        .unwrap()
        .get(b"Resources")
        .unwrap();
    match resources {
        Object::Dictionary(dictionary) => dictionary.clone(),
        Object::Reference(id) => document.get_dictionary(*id).unwrap().clone(),
        other => panic!("unexpected resources object: {other:?}"),
    }
}

fn page_content_contains_operator(
    document: &lopdf::Document,
    page_number: u32,
    operator: &str,
) -> bool {
    let page_id = document.get_pages().get(&page_number).copied().unwrap();
    document
        .get_page_contents(page_id)
        .into_iter()
        .filter_map(|content_id| document.get_object(content_id).ok())
        .filter_map(|object| object.as_stream().ok())
        .filter_map(|stream| lopdf::content::Content::decode(&stream.content).ok())
        .flat_map(|content| content.operations)
        .any(|operation| operation.operator == operator)
}

fn page_xobject_subtypes(document: &lopdf::Document, page_number: u32) -> Vec<Vec<u8>> {
    let resources = page_resources(document, page_number);
    let Ok(xobjects) = resources.get(b"XObject").and_then(Object::as_dict) else {
        return Vec::new();
    };
    xobjects
        .iter()
        .filter_map(|(_, object)| object.as_reference().ok())
        .filter_map(|id| document.get_object(id).ok())
        .filter_map(|object| object.as_stream().ok())
        .filter_map(|stream| stream.dict.get(b"Subtype").and_then(Object::as_name).ok())
        .map(|name| name.to_vec())
        .collect()
}

fn page_xobject_count(document: &lopdf::Document, page_number: u32) -> usize {
    let resources = page_resources(document, page_number);
    resources
        .get(b"XObject")
        .and_then(Object::as_dict)
        .map(|dictionary| dictionary.len())
        .unwrap_or(0)
}

fn page_content_text_contains(
    document: &lopdf::Document,
    page_number: u32,
    expected: &str,
) -> bool {
    let page_id = document.get_pages().get(&page_number).copied().unwrap();
    String::from_utf8_lossy(&document.get_page_content(page_id).unwrap()).contains(expected)
}

fn page_form_xobject_operators(document: &lopdf::Document, page_number: u32) -> Vec<String> {
    let resources = page_resources(document, page_number);
    let Ok(xobjects) = resources.get(b"XObject").and_then(Object::as_dict) else {
        return Vec::new();
    };
    let mut operators = Vec::new();
    let mut seen = BTreeSet::new();
    for (_, object) in xobjects.iter() {
        if let Ok(id) = object.as_reference() {
            collect_form_xobject_operators(document, id, &mut seen, &mut operators);
        }
    }
    operators
}

fn collect_form_xobject_operators(
    document: &lopdf::Document,
    object_id: lopdf::ObjectId,
    seen: &mut BTreeSet<lopdf::ObjectId>,
    operators: &mut Vec<String>,
) {
    if !seen.insert(object_id) {
        return;
    }
    let Ok(stream) = document
        .get_object(object_id)
        .and_then(lopdf::Object::as_stream)
    else {
        return;
    };
    if stream
        .dict
        .get(b"Subtype")
        .and_then(lopdf::Object::as_name)
        .ok()
        != Some(b"Form".as_slice())
    {
        return;
    }
    if let Ok(content) = stream.get_plain_content() {
        if let Ok(content) = lopdf::content::Content::decode(&content) {
            operators.extend(
                content
                    .operations
                    .into_iter()
                    .map(|operation| operation.operator),
            );
        }
    }
    let Ok(resources) = stream.dict.get(b"Resources").and_then(Object::as_dict) else {
        return;
    };
    let Ok(xobjects) = resources.get(b"XObject").and_then(Object::as_dict) else {
        return;
    };
    for (_, object) in xobjects.iter() {
        if let Ok(id) = object.as_reference() {
            collect_form_xobject_operators(document, id, seen, operators);
        }
    }
}

fn simple_svg() -> &'static [u8] {
    br##"<svg xmlns="http://www.w3.org/2000/svg" width="120" height="80">
            <rect x="10" y="10" width="100" height="60" fill="#16a34a"/>
        </svg>"##
}

#[derive(Default)]
struct RecordingRunner {
    executed: Vec<String>,
    fail_on: Option<&'static str>,
    error: Option<OxideError>,
}

impl OperatorRunner for RecordingRunner {
    fn run(&mut self, task: &TaskSpec, _inputs: &[Artifact]) -> Result<Artifact, OxideError> {
        self.executed.push(task.id.as_str().to_owned());
        if self.fail_on == Some(task.id.as_str()) {
            return Err(self.error.take().unwrap());
        }

        Ok(Artifact::bytes(task.id.as_str().as_bytes()))
    }
}

struct SlowRunner;

impl OperatorRunner for SlowRunner {
    fn run(&mut self, _task: &TaskSpec, _inputs: &[Artifact]) -> Result<Artifact, OxideError> {
        std::thread::sleep(std::time::Duration::from_millis(5));
        Ok(Artifact::bytes(b"finished"))
    }
}
