#[allow(unused_imports)]
use common::*;
#[allow(unused_imports)]
use oxidepdf_cli::{command, run, run_with_io};
use std::fs;

#[tokio::test]
async fn merge_command_writes_combined_pdf() {
    let dir = temp_dir("merge_command_writes_combined_pdf");
    let input = fixture_pdf();
    let output = dir.join("merged.pdf");
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let code = run_with_io(
        [
            "oxidepdf",
            "pdf_edit",
            "merge",
            input.to_str().unwrap(),
            input.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
        ],
        [],
        &mut stdout,
        &mut stderr,
    )
    .await;

    assert_eq!(code, 0);
    assert_eq!(stdout, b"");
    assert_eq!(stderr, b"");
    assert_eq!(pdf_page_count(&output), 6);
}

#[tokio::test]
async fn split_command_writes_selected_pages() {
    let dir = temp_dir("split_command_writes_selected_pages");
    let output = dir.join("split.pdf");
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let code = run_with_io(
        [
            "oxidepdf",
            "pdf_edit",
            "keep-pages",
            fixture_pdf().to_str().unwrap(),
            "--pages",
            "1,3",
            "-o",
            output.to_str().unwrap(),
        ],
        [],
        &mut stdout,
        &mut stderr,
    )
    .await;

    assert_eq!(code, 0);
    assert_eq!(stdout, b"");
    assert_eq!(stderr, b"");
    assert_eq!(pdf_page_count(&output), 2);
}

#[tokio::test]
async fn rotate_command_updates_rotation() {
    let dir = temp_dir("rotate_command_updates_rotation");
    let output = dir.join("rotated.pdf");
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let code = run_with_io(
        [
            "oxidepdf",
            "pdf_edit",
            "rotate-pages",
            fixture_pdf().to_str().unwrap(),
            "--pages",
            "1",
            "--degrees",
            "90",
            "-o",
            output.to_str().unwrap(),
        ],
        [],
        &mut stdout,
        &mut stderr,
    )
    .await;

    assert_eq!(code, 0);
    assert_eq!(stdout, b"");
    assert_eq!(stderr, b"");
    assert_eq!(pdf_page_rotation(&output, 1), 90);
}

#[tokio::test]
async fn delete_pages_command_removes_selected_pages() {
    let dir = temp_dir("delete_pages_command_removes_selected_pages");
    let output = dir.join("deleted.pdf");
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let code = run_with_io(
        [
            "oxidepdf",
            "pdf_edit",
            "delete-pages",
            fixture_pdf().to_str().unwrap(),
            "--pages",
            "2",
            "-o",
            output.to_str().unwrap(),
        ],
        [],
        &mut stdout,
        &mut stderr,
    )
    .await;

    assert_eq!(code, 0);
    assert_eq!(stdout, b"");
    assert_eq!(stderr, b"");
    assert_eq!(pdf_page_count(&output), 2);
}

#[tokio::test]
async fn extract_pages_command_writes_selected_pages() {
    let dir = temp_dir("extract_pages_command_writes_selected_pages");
    let output = dir.join("extracted.pdf");
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let code = run_with_io(
        [
            "oxidepdf",
            "pdf_edit",
            "extract-pages",
            fixture_pdf().to_str().unwrap(),
            "--pages",
            "3,1",
            "-o",
            output.to_str().unwrap(),
        ],
        [],
        &mut stdout,
        &mut stderr,
    )
    .await;

    assert_eq!(code, 0);
    assert_eq!(stdout, b"");
    assert_eq!(stderr, b"");
    assert_eq!(pdf_page_count(&output), 2);
}

#[tokio::test]
async fn crop_pages_command_sets_crop_box() {
    let dir = temp_dir("crop_pages_command_sets_crop_box");
    let output = dir.join("cropped.pdf");
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let code = run_with_io(
        [
            "oxidepdf",
            "pdf_edit",
            "crop-pages",
            fixture_pdf().to_str().unwrap(),
            "--pages",
            "1",
            "--left",
            "10",
            "--bottom",
            "20",
            "--right",
            "300",
            "--top",
            "400",
            "-o",
            output.to_str().unwrap(),
        ],
        [],
        &mut stdout,
        &mut stderr,
    )
    .await;

    assert_eq!(code, 0);
    assert_eq!(stdout, b"");
    assert_eq!(stderr, b"");
    assert_eq!(
        pdf_page_box(&output, 1, b"CropBox"),
        [10.0, 20.0, 300.0, 400.0]
    );
}

#[tokio::test]
async fn scale_pages_command_scales_selected_page() {
    let dir = temp_dir("scale_pages_command_scales_selected_page");
    let output = dir.join("scaled.pdf");
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let code = run_with_io(
        [
            "oxidepdf",
            "pdf_edit",
            "scale-pages",
            fixture_pdf().to_str().unwrap(),
            "--pages",
            "1",
            "--factor",
            "0.5",
            "-o",
            output.to_str().unwrap(),
        ],
        [],
        &mut stdout,
        &mut stderr,
    )
    .await;

    assert_eq!(code, 0);
    assert_eq!(stdout, b"");
    assert_eq!(stderr, b"");
    assert_eq!(pdf_page_box(&output, 1, b"MediaBox")[2], 306.0);
    assert_eq!(pdf_page_box(&output, 2, b"MediaBox")[2], 612.0);
}

#[tokio::test]
async fn delete_blank_pages_command_removes_structurally_blank_pages() {
    let dir = temp_dir("delete_blank_pages_command_removes_structurally_blank_pages");
    let input = dir.join("blank-and-marked.pdf");
    let output = dir.join("without-blank.pdf");
    fs::write(&input, pdf_with_blank_and_marked_page()).unwrap();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let code = run_with_io(
        [
            "oxidepdf",
            "pdf_edit",
            "delete-blank-pages",
            input.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
        ],
        [],
        &mut stdout,
        &mut stderr,
    )
    .await;

    assert_eq!(code, 0);
    assert_eq!(stdout, b"");
    assert_eq!(stderr, b"");
    assert_eq!(pdf_page_count(&output), 1);
}

#[tokio::test]
async fn single_page_command_combines_pages() {
    let dir = temp_dir("single_page_command_combines_pages");
    let output = dir.join("single.pdf");
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let code = run_with_io(
        [
            "oxidepdf",
            "pdf_edit",
            "single-page",
            fixture_pdf().to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
        ],
        [],
        &mut stdout,
        &mut stderr,
    )
    .await;

    assert_eq!(code, 0);
    assert_eq!(stdout, b"");
    assert_eq!(stderr, b"");
    assert_eq!(pdf_page_count(&output), 1);
    assert_eq!(pdf_page_box(&output, 1, b"MediaBox")[3], 2376.0);
}

#[tokio::test]
async fn nup_command_places_pages_on_fewer_output_pages() {
    let dir = temp_dir("nup_command_places_pages_on_fewer_output_pages");
    let output = dir.join("nup.pdf");
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let code = run_with_io(
        [
            "oxidepdf",
            "pdf_edit",
            "nup",
            fixture_pdf().to_str().unwrap(),
            "--columns",
            "2",
            "--rows",
            "2",
            "-o",
            output.to_str().unwrap(),
        ],
        [],
        &mut stdout,
        &mut stderr,
    )
    .await;

    assert_eq!(code, 0);
    assert_eq!(stdout, b"");
    assert_eq!(stderr, b"");
    assert_eq!(pdf_page_count(&output), 1);
    assert_eq!(pdf_page_xobject_count(&output, 1), 3);
}

#[tokio::test]
async fn booklet_command_writes_imposed_pages() {
    let dir = temp_dir("booklet_command_writes_imposed_pages");
    let output = dir.join("booklet.pdf");
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let code = run_with_io(
        [
            "oxidepdf",
            "pdf_edit",
            "booklet",
            fixture_pdf().to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
        ],
        [],
        &mut stdout,
        &mut stderr,
    )
    .await;

    assert_eq!(code, 0);
    assert_eq!(stdout, b"");
    assert_eq!(stderr, b"");
    assert_eq!(pdf_page_count(&output), 2);
    assert_eq!(pdf_page_xobject_count(&output, 2), 2);
}
