//! Integration tests for the `adv` CLI surface.
//!
//! Split out of the former monolithic `src/lib.rs` test module. These drive the
//! CLI through its public `run_with_io`/`command` entry points only.

mod common;
#[allow(unused_imports)]
use common::*;
#[allow(unused_imports)]
use oxidepdf_cli::{command, run, run_with_io};
use std::fs;

#[test]
fn metadata_commands_set_and_get_json_report() {
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
    );
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
    );
    assert_eq!(code, 0);
    let report: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(report).unwrap()).unwrap();
    assert_eq!(report["entries"]["title"], "Quarterly Report");
    assert_eq!(report["entries"]["author"], "OxidePDF");
}

#[test]
fn attachment_commands_add_list_extract_and_delete() {
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
    );
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
    );
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
    );
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
    );
    assert_eq!(code, 0);
    assert_eq!(pdf_page_count(&deleted), 1);
}

#[test]
fn commands_with_two_inputs_reject_shared_stdin() {
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
    );
    assert_eq!(code, 2);
    assert!(!outline_output.exists());
    assert!(String::from_utf8(stderr.clone())
        .unwrap()
        .contains("cannot read both inputs from stdin"));

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
    );
    assert_eq!(code, 2);
    assert!(!attach_output.exists());
    assert!(String::from_utf8(stderr)
        .unwrap()
        .contains("cannot read both inputs from stdin"));
}

#[test]
fn annotation_and_interactive_commands_remove_selected_elements() {
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
    );
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
    );
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
    );
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
    );
    assert_eq!(code, 0);
    let report: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(empty_report).unwrap()).unwrap();
    assert!(report["annotations"].as_array().unwrap().is_empty());
}

#[test]
fn form_commands_fill_inspect_unlock_and_remove() {
    let dir = temp_dir("form_commands_fill_inspect_unlock_and_remove");
    let input = dir.join("input.pdf");
    let filled = dir.join("filled.pdf");
    let report = dir.join("forms.json");
    let unlocked = dir.join("unlocked.pdf");
    let removed = dir.join("removed.pdf");
    let empty_report = dir.join("empty-forms.json");
    fs::write(&input, form_pdf(true)).unwrap();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let code = run_with_io(
        [
            "oxidepdf",
            "pdf_adv",
            "form",
            "fill",
            input.to_str().unwrap(),
            "--field",
            "customer=Ada",
            "-o",
            filled.to_str().unwrap(),
        ],
        [],
        &mut stdout,
        &mut stderr,
    );
    assert_eq!(code, 0);

    let code = run_with_io(
        [
            "oxidepdf",
            "pdf_adv",
            "form",
            "inspect",
            filled.to_str().unwrap(),
            "-o",
            report.to_str().unwrap(),
        ],
        [],
        &mut stdout,
        &mut stderr,
    );
    assert_eq!(code, 0);
    let report: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&report).unwrap()).unwrap();
    assert_eq!(report["fields"][0]["value"], "Ada");
    assert_eq!(report["fields"][0]["readonly"], true);

    let code = run_with_io(
        [
            "oxidepdf",
            "pdf_adv",
            "form",
            "unlock-readonly",
            filled.to_str().unwrap(),
            "-o",
            unlocked.to_str().unwrap(),
        ],
        [],
        &mut stdout,
        &mut stderr,
    );
    assert_eq!(code, 0);

    let code = run_with_io(
        [
            "oxidepdf",
            "pdf_adv",
            "form",
            "remove",
            unlocked.to_str().unwrap(),
            "-o",
            removed.to_str().unwrap(),
        ],
        [],
        &mut stdout,
        &mut stderr,
    );
    assert_eq!(code, 0);
    let code = run_with_io(
        [
            "oxidepdf",
            "pdf_adv",
            "form",
            "inspect",
            removed.to_str().unwrap(),
            "-o",
            empty_report.to_str().unwrap(),
        ],
        [],
        &mut stdout,
        &mut stderr,
    );
    assert_eq!(code, 0);
    let report: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(empty_report).unwrap()).unwrap();
    assert!(report["fields"].as_array().unwrap().is_empty());
}

#[test]
fn stamp_overlay_image_and_color_commands_write_expected_outputs() {
    let dir = temp_dir("stamp_overlay_image_and_color_commands_write_expected_outputs");
    let input = dir.join("input.pdf");
    let overlay = dir.join("overlay.pdf");
    let stamped = dir.join("stamped.pdf");
    let overlaid = dir.join("overlaid.pdf");
    let image_report = dir.join("images.json");
    let extracted = dir.join("image.rgb");
    let image_added = dir.join("image-added.pdf");
    let image_deleted = dir.join("image-deleted.pdf");
    let colored = dir.join("colored.pdf");
    fs::write(&input, empty_page_pdf()).unwrap();
    fs::write(&overlay, fixture_pdf_bytes()).unwrap();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let code = run_with_io(
        [
            "oxidepdf",
            "pdf_edit",
            "stamp",
            input.to_str().unwrap(),
            "--text",
            "APPROVED",
            "--font",
            "Helvetica",
            "-o",
            stamped.to_str().unwrap(),
        ],
        [],
        &mut stdout,
        &mut stderr,
    );
    assert_eq!(code, 0);
    assert!(page_has_content_operator(&stamped, 1, "Tj"));

    let code = run_with_io(
        [
            "oxidepdf",
            "pdf_edit",
            "overlay-pdf",
            stamped.to_str().unwrap(),
            overlay.to_str().unwrap(),
            "--source-page",
            "1",
            "-o",
            overlaid.to_str().unwrap(),
        ],
        [],
        &mut stdout,
        &mut stderr,
    );
    assert_eq!(code, 0);
    assert!(page_has_content_operator(&overlaid, 1, "Do"));

    let code = run_with_io(
        [
            "oxidepdf",
            "pdf_adv",
            "image",
            "list",
            overlaid.to_str().unwrap(),
            "-o",
            image_report.to_str().unwrap(),
        ],
        [],
        &mut stdout,
        &mut stderr,
    );
    assert_eq!(code, 0);
    let report: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&image_report).unwrap()).unwrap();
    assert!(report["images"].as_array().unwrap().is_empty());

    let code = run_with_io(
        [
            "oxidepdf",
            "pdf_adv",
            "image",
            "add",
            overlaid.to_str().unwrap(),
            fixture_jpg().to_str().unwrap(),
            "--name",
            "Logo",
            "--page",
            "1",
            "-o",
            image_added.to_str().unwrap(),
        ],
        [],
        &mut stdout,
        &mut stderr,
    );
    assert_eq!(code, 0);

    let code = run_with_io(
        [
            "oxidepdf",
            "pdf_adv",
            "image",
            "extract",
            image_added.to_str().unwrap(),
            "--name",
            "Logo",
            "-o",
            extracted.to_str().unwrap(),
        ],
        [],
        &mut stdout,
        &mut stderr,
    );
    assert_eq!(code, 0);
    assert!(!fs::read(&extracted).unwrap().is_empty());

    let code = run_with_io(
        [
            "oxidepdf",
            "pdf_adv",
            "image",
            "delete",
            image_added.to_str().unwrap(),
            "--name",
            "Logo",
            "-o",
            image_deleted.to_str().unwrap(),
        ],
        [],
        &mut stdout,
        &mut stderr,
    );
    assert_eq!(code, 0);

    let color_input = dir.join("color.pdf");
    fs::write(&color_input, pdf_with_rgb_fill_content()).unwrap();
    let code = run_with_io(
        [
            "oxidepdf",
            "pdf_edit",
            "color",
            "invert",
            color_input.to_str().unwrap(),
            "-o",
            colored.to_str().unwrap(),
        ],
        [],
        &mut stdout,
        &mut stderr,
    );
    assert_eq!(code, 0);
    assert_eq!(pdf_rgb_operator(&colored, 1, "rg"), Some([0.0, 1.0, 1.0]));
}
