#[allow(unused_imports)]
use common::*;
#[allow(unused_imports)]
use oxidepdf_cli::{command, run, run_with_io};
use std::fs;

#[tokio::test]
async fn metadata_commands_set_and_get_json_report() {
    let dir = temp_dir("metadata_commands_set_and_get_json_report");
    let input = dir.join("input.pdf");
    let edited = dir.join("metadata.pdf");
    let report = dir.join("metadata.json");
    fs::write(&input, empty_page_pdf()).unwrap();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let code = run_with_io(
        [
            "oxidepdf",
            "pdf_adv",
            "metadata",
            "set",
            input.to_str().unwrap(),
            "--entry",
            "title=Quarterly Report",
            "--entry",
            "author=OxidePDF",
            "-o",
            edited.to_str().unwrap(),
        ],
        [],
        &mut stdout,
        &mut stderr,
    )
    .await;
    assert_eq!(code, 0);
    assert_eq!(stderr, b"");

    let code = run_with_io(
        [
            "oxidepdf",
            "pdf_adv",
            "metadata",
            "get",
            edited.to_str().unwrap(),
            "-o",
            report.to_str().unwrap(),
        ],
        [],
        &mut stdout,
        &mut stderr,
    )
    .await;
    assert_eq!(code, 0);
    let report: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(report).unwrap()).unwrap();
    assert_eq!(report["entries"]["title"], "Quarterly Report");
    assert_eq!(report["entries"]["author"], "OxidePDF");
}

#[tokio::test]
async fn attachment_commands_add_list_extract_and_delete() {
    let dir = temp_dir("attachment_commands_add_list_extract_and_delete");
    let input = dir.join("input.pdf");
    let note = dir.join("note.txt");
    let attached = dir.join("attached.pdf");
    let report = dir.join("attachments.json");
    let extracted = dir.join("extracted.txt");
    let deleted = dir.join("deleted.pdf");
    fs::write(&input, empty_page_pdf()).unwrap();
    fs::write(&note, b"attachment bytes").unwrap();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let code = run_with_io(
        [
            "oxidepdf",
            "pdf_adv",
            "attach",
            "add",
            input.to_str().unwrap(),
            note.to_str().unwrap(),
            "--description",
            "Review note",
            "-o",
            attached.to_str().unwrap(),
        ],
        [],
        &mut stdout,
        &mut stderr,
    )
    .await;
    assert_eq!(code, 0);
    assert_eq!(stderr, b"");

    let code = run_with_io(
        [
            "oxidepdf",
            "pdf_adv",
            "attach",
            "list",
            attached.to_str().unwrap(),
            "-o",
            report.to_str().unwrap(),
        ],
        [],
        &mut stdout,
        &mut stderr,
    )
    .await;
    assert_eq!(code, 0);
    let report: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&report).unwrap()).unwrap();
    assert_eq!(report["attachments"][0]["name"], "note.txt");

    let code = run_with_io(
        [
            "oxidepdf",
            "pdf_adv",
            "attach",
            "extract",
            attached.to_str().unwrap(),
            "--name",
            "note.txt",
            "-o",
            extracted.to_str().unwrap(),
        ],
        [],
        &mut stdout,
        &mut stderr,
    )
    .await;
    assert_eq!(code, 0);
    assert_eq!(fs::read(&extracted).unwrap(), b"attachment bytes");

    let code = run_with_io(
        [
            "oxidepdf",
            "pdf_adv",
            "attach",
            "delete",
            attached.to_str().unwrap(),
            "--name",
            "note.txt",
            "-o",
            deleted.to_str().unwrap(),
        ],
        [],
        &mut stdout,
        &mut stderr,
    )
    .await;
    assert_eq!(code, 0);
    assert_eq!(pdf_page_count(&deleted), 1);
}

#[tokio::test]
async fn commands_with_two_inputs_reject_shared_stdin() {
    let dir = temp_dir("commands_with_two_inputs_reject_shared_stdin");
    let outline_output = dir.join("outline.pdf");
    let attach_output = dir.join("attached.pdf");
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let code = run_with_io(
        [
            "oxidepdf",
            "pdf_adv",
            "outline",
            "set",
            "-",
            "--tree",
            "-",
            "-o",
            outline_output.to_str().unwrap(),
        ],
        empty_page_pdf(),
        &mut stdout,
        &mut stderr,
    )
    .await;
    assert_eq!(code, 2);
    assert!(!outline_output.exists());
    assert!(
        String::from_utf8(stderr.clone())
            .unwrap()
            .contains("cannot read both inputs from stdin")
    );

    stdout.clear();
    stderr.clear();
    let code = run_with_io(
        [
            "oxidepdf",
            "pdf_adv",
            "attach",
            "add",
            "-",
            "-",
            "--name",
            "note.txt",
            "-o",
            attach_output.to_str().unwrap(),
        ],
        empty_page_pdf(),
        &mut stdout,
        &mut stderr,
    )
    .await;
    assert_eq!(code, 2);
    assert!(!attach_output.exists());
    assert!(
        String::from_utf8(stderr)
            .unwrap()
            .contains("cannot read both inputs from stdin")
    );
}

#[tokio::test]
async fn annotation_and_interactive_commands_remove_selected_elements() {
    let dir = temp_dir("annotation_and_interactive_commands_remove_selected_elements");
    let input = dir.join("input.pdf");
    let annotated = dir.join("annotated.pdf");
    let report = dir.join("annotations.json");
    let removed = dir.join("removed.pdf");
    let empty_report = dir.join("empty-annotations.json");
    fs::write(&input, empty_page_pdf()).unwrap();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let code = run_with_io(
        [
            "oxidepdf",
            "pdf_adv",
            "annot",
            "add",
            input.to_str().unwrap(),
            "--page",
            "1",
            "--id",
            "review-note",
            "--text",
            "Review this page",
            "-o",
            annotated.to_str().unwrap(),
        ],
        [],
        &mut stdout,
        &mut stderr,
    )
    .await;
    assert_eq!(code, 0);

    let code = run_with_io(
        [
            "oxidepdf",
            "pdf_adv",
            "annot",
            "list",
            annotated.to_str().unwrap(),
            "-o",
            report.to_str().unwrap(),
        ],
        [],
        &mut stdout,
        &mut stderr,
    )
    .await;
    assert_eq!(code, 0);
    let report: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&report).unwrap()).unwrap();
    assert_eq!(report["annotations"][0]["id"], "review-note");

    let code = run_with_io(
        [
            "oxidepdf",
            "pdf_edit",
            "interactive-remove",
            annotated.to_str().unwrap(),
            "--annotations",
            "-o",
            removed.to_str().unwrap(),
        ],
        [],
        &mut stdout,
        &mut stderr,
    )
    .await;
    assert_eq!(code, 0);
    let code = run_with_io(
        [
            "oxidepdf",
            "pdf_adv",
            "annot",
            "list",
            removed.to_str().unwrap(),
            "-o",
            empty_report.to_str().unwrap(),
        ],
        [],
        &mut stdout,
        &mut stderr,
    )
    .await;
    assert_eq!(code, 0);
    let report: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(empty_report).unwrap()).unwrap();
    assert!(report["annotations"].as_array().unwrap().is_empty());
}
