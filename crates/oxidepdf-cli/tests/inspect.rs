//! Integration tests for the `inspect` CLI surface.
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
fn extract_text_command_writes_plain_text() {
    let dir = temp_dir("extract_text_command_writes_plain_text");
    let output = dir.join("extracted.txt");
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let code = run_with_io(
        [
            "oxidepdf",
            "pdf_inspect",
            "extract-text",
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
    assert!(!fs::read_to_string(output).unwrap().trim().is_empty());
}

#[test]
fn extract_text_command_rejects_pdf_without_text_layer() {
    let dir = temp_dir("extract_text_command_rejects_pdf_without_text_layer");
    let input = dir.join("image.pdf");
    let output = dir.join("extracted.txt");
    fs::write(&input, image_only_pdf()).unwrap();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let code = run_with_io(
        [
            "oxidepdf",
            "pdf_inspect",
            "extract-text",
            input.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
        ],
        [],
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(code, 3);
    assert_eq!(stdout, b"");
    assert!(!output.exists());
    assert!(String::from_utf8(stderr)
        .unwrap()
        .contains("no extractable text layer"));
}
