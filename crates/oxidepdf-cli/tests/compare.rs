//! Integration tests for the `compare` CLI surface.
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
fn compare_command_writes_json_report() {
    let dir = temp_dir("compare_command_writes_json_report");
    let output = dir.join("compare.json");
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let code = run_with_io(
        [
            "oxidepdf",
            "pdf_compare",
            "report",
            fixture_pdf().to_str().unwrap(),
            fixture_pdf().to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
        ],
        [],
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(code, 0);
    assert_eq!(stdout, b"");
    assert_eq!(stderr, b"");
    let report: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(output).unwrap()).unwrap();
    assert_eq!(report["equal"], true);
    assert_eq!(report["differences"], serde_json::json!([]));
}

#[test]
fn compare_command_writes_visual_diff_png() {
    let dir = temp_dir("compare_command_writes_visual_diff_png");
    let left = dir.join("left.pdf");
    let right = dir.join("right.pdf");
    let output = dir.join("diff.png");
    fs::write(&left, empty_page_pdf()).unwrap();
    fs::write(&right, pdf_with_rgb_fill_content()).unwrap();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let code = run_with_io(
        [
            "oxidepdf",
            "pdf_compare",
            "visual-diff",
            left.to_str().unwrap(),
            right.to_str().unwrap(),
            "--page",
            "1",
            "-o",
            output.to_str().unwrap(),
        ],
        [],
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(code, 0);
    assert_eq!(stdout, b"");
    assert_eq!(stderr, b"");
    let image = image::load_from_memory(&fs::read(output).unwrap()).unwrap();
    assert!(image.width() > 0);
    assert!(image.height() > 0);
}
