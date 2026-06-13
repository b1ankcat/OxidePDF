//! Integration tests for the `workflow_ops` CLI surface.
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
fn workflow_img2pdf_writes_parseable_pdf() {
    let dir = temp_dir("workflow_img2pdf_writes_parseable_pdf");
    let workflow = dir.join("workflow.yaml");
    let output = dir.join("image.pdf");
    fs::write(
        &workflow,
        format!(
            r#"
            version: 1
            inputs:
              - id: source
                path: {}
            tasks:
              - id: convert
                op:
                  pdf_edit:
                    image_to_pdf:
                      layout: original_size
                inputs: [source]
            outputs:
              - id: final
                from: convert
                path: {}
            "#,
            yaml_path(fixture_jpg()),
            yaml_path(&output)
        ),
    )
    .unwrap();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let code = run_with_io(
        ["oxidepdf", "run", "--workflow", workflow.to_str().unwrap()],
        [],
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(code, 0);
    assert_eq!(stdout, b"");
    assert_eq!(stderr, b"");
    assert_eq!(pdf_page_count(&output), 1);
}

#[test]
fn workflow_extract_text_writes_plain_text() {
    let dir = temp_dir("workflow_extract_text_writes_plain_text");
    let workflow = dir.join("workflow.yaml");
    let output = dir.join("extracted.txt");
    fs::write(
        &workflow,
        format!(
            r#"
            version: 1
            inputs:
              - id: source
                path: {}
            tasks:
              - id: extract
                op:
                  pdf_inspect:
                    extract_text:
                      format: plain
                inputs: [source]
            outputs:
              - id: final
                from: extract
                path: {}
            "#,
            yaml_path(fixture_pdf()),
            yaml_path(&output)
        ),
    )
    .unwrap();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let code = run_with_io(
        ["oxidepdf", "run", "--workflow", workflow.to_str().unwrap()],
        [],
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(code, 0);
    assert_eq!(stdout, b"");
    assert_eq!(stderr, b"");
    assert!(!fs::read_to_string(output).unwrap().trim().is_empty());
}
