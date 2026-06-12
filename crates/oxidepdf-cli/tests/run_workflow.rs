use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;

#[test]
fn run_workflow_file_succeeds() {
    let dir = temp_dir("run_workflow_file_succeeds");
    let input = dir.join("input.bin");
    let output = dir.join("output.bin");
    let workflow = dir.join("workflow.yaml");
    fs::write(&input, b"input").unwrap();
    fs::write(
        &workflow,
        format!(
            r#"
            version: 1
            inputs:
              - id: source
                path: {}
            tasks: []
            outputs:
              - id: final
                from: source
                path: {}
            "#,
            input.display(),
            output.display()
        ),
    )
    .unwrap();

    Command::cargo_bin("oxidepdf")
        .unwrap()
        .args(["run", "--workflow", workflow.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::eq(""))
        .stderr(predicate::eq(""));

    assert_eq!(fs::read(output).unwrap(), b"input");
}

#[test]
fn invalid_workflow_exits_with_code_2() {
    let dir = temp_dir("invalid_workflow_exits_with_code_2");
    let workflow = dir.join("workflow.yaml");
    fs::write(&workflow, "version: 1\ninputs: []\ntasks: []\n").unwrap();

    Command::cargo_bin("oxidepdf")
        .unwrap()
        .args(["run", "--workflow", workflow.to_str().unwrap()])
        .assert()
        .code(2)
        .stdout(predicate::eq(""))
        .stderr(predicate::str::contains("invalid workflow"));
}

#[test]
fn unsupported_operator_exits_with_code_3() {
    let dir = temp_dir("invalid_pdf_operator_exits_with_code_3");
    let input = dir.join("input.bin");
    let output = dir.join("output.bin");
    let workflow = dir.join("workflow.yaml");
    fs::write(&input, b"input").unwrap();
    fs::write(
        &workflow,
        format!(
            r#"
            version: 1
            inputs:
              - id: source
                path: {}
            tasks:
              - id: rotate
                op:
                  rotate:
                    pages: "1"
                    degrees: 90
                inputs: [source]
            outputs:
              - id: final
                from: rotate
                path: {}
            "#,
            input.display(),
            output.display()
        ),
    )
    .unwrap();

    Command::cargo_bin("oxidepdf")
        .unwrap()
        .args(["run", "--workflow", workflow.to_str().unwrap()])
        .assert()
        .code(3)
        .stdout(predicate::eq(""))
        .stderr(predicate::str::contains("expected PDF"));

    assert!(!output.exists());
}

#[test]
fn workflow_rotate_updates_pdf_page_rotation() {
    let dir = temp_dir("workflow_rotate_updates_pdf_page_rotation");
    let output = dir.join("output.pdf");
    let workflow = dir.join("workflow.yaml");
    fs::write(
        &workflow,
        format!(
            r#"
            version: 1
            inputs:
              - id: source
                path: {}
            tasks:
              - id: rotate
                op:
                  rotate:
                    pages: "1"
                    degrees: 90
                inputs: [source]
            outputs:
              - id: final
                from: rotate
                path: {}
            "#,
            fixture_pdf().display(),
            output.display()
        ),
    )
    .unwrap();

    Command::cargo_bin("oxidepdf")
        .unwrap()
        .args(["run", "--workflow", workflow.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::eq(""))
        .stderr(predicate::eq(""));

    assert_eq!(pdf_page_rotation(&output, 1), 90);
}

#[test]
fn reorder_command_writes_parseable_pdf() {
    let dir = temp_dir("reorder_command_writes_parseable_pdf");
    let output = dir.join("reordered.pdf");

    Command::cargo_bin("oxidepdf")
        .unwrap()
        .args([
            "reorder",
            fixture_pdf().to_str().unwrap(),
            "--pages",
            "3,1,2",
            "-o",
            output.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::eq(""))
        .stderr(predicate::eq(""));

    assert_eq!(pdf_page_count(&output), 3);
}

#[test]
fn img2pdf_command_writes_parseable_pdf() {
    let dir = temp_dir("img2pdf_command_writes_parseable_pdf");
    let output = dir.join("image.pdf");

    Command::cargo_bin("oxidepdf")
        .unwrap()
        .args([
            "img2pdf",
            fixture_jpg().to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::eq(""))
        .stderr(predicate::eq(""));

    assert_eq!(pdf_page_count(&output), 1);
}

#[test]
fn svg2pdf_command_writes_parseable_pdf() {
    let dir = temp_dir("svg2pdf_command_writes_parseable_pdf");
    let input = dir.join("input.svg");
    let output = dir.join("svg.pdf");
    fs::write(
        &input,
        br##"<svg xmlns="http://www.w3.org/2000/svg" width="120" height="80">
            <rect x="10" y="10" width="100" height="60" fill="#16a34a"/>
        </svg>"##,
    )
    .unwrap();

    Command::cargo_bin("oxidepdf")
        .unwrap()
        .args([
            "svg2pdf",
            input.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::eq(""))
        .stderr(predicate::eq(""));

    assert_eq!(pdf_page_count(&output), 1);
}

#[test]
fn render_command_writes_png() {
    let dir = temp_dir("render_command_writes_png");
    let output = dir.join("page.png");

    Command::cargo_bin("oxidepdf")
        .unwrap()
        .args([
            "render",
            fixture_pdf().to_str().unwrap(),
            "--page",
            "1",
            "-o",
            output.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::eq(""))
        .stderr(predicate::eq(""));

    let image = image::load_from_memory(&fs::read(output).unwrap()).unwrap();
    assert!(image.width() > 0);
    assert!(image.height() > 0);
}

#[test]
fn extract_text_command_writes_plain_text() {
    let dir = temp_dir("extract_text_command_writes_plain_text");
    let output = dir.join("extracted.txt");

    Command::cargo_bin("oxidepdf")
        .unwrap()
        .args([
            "extract-text",
            fixture_pdf().to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::eq(""))
        .stderr(predicate::eq(""));

    assert!(!fs::read_to_string(output).unwrap().trim().is_empty());
}

#[test]
fn workflow_extract_text_writes_plain_text() {
    let dir = temp_dir("workflow_extract_text_writes_plain_text");
    let output = dir.join("extracted.txt");
    let workflow = dir.join("workflow.yaml");
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
                  extract_text:
                    format: plain
                inputs: [source]
            outputs:
              - id: final
                from: extract
                path: {}
            "#,
            fixture_pdf().display(),
            output.display()
        ),
    )
    .unwrap();

    Command::cargo_bin("oxidepdf")
        .unwrap()
        .args(["run", "--workflow", workflow.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::eq(""))
        .stderr(predicate::eq(""));

    assert!(!fs::read_to_string(output).unwrap().trim().is_empty());
}

#[test]
fn watermark_text_command_writes_parseable_pdf() {
    let dir = temp_dir("watermark_text_command_writes_parseable_pdf");
    let output = dir.join("watermarked.pdf");

    Command::cargo_bin("oxidepdf")
        .unwrap()
        .args([
            "watermark",
            fixture_pdf().to_str().unwrap(),
            "--kind",
            "text",
            "--text",
            "DRAFT",
            "--font",
            "DejaVu Sans",
            "--pages",
            "1",
            "-o",
            output.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::eq(""))
        .stderr(predicate::eq(""));

    assert_eq!(pdf_page_count(&output), 3);
    assert!(page_has_content_operator(&output, 1, "Tj"));
}

#[test]
fn workflow_watermark_image_writes_parseable_pdf() {
    let dir = temp_dir("workflow_watermark_image_writes_parseable_pdf");
    let output = dir.join("watermarked.pdf");
    let workflow = dir.join("workflow.yaml");
    fs::write(
        &workflow,
        format!(
            r#"
            version: 1
            inputs:
              - id: source
                path: {}
              - id: mark
                path: {}
            tasks:
              - id: watermark
                op:
                  watermark:
                    kind: image
                    opacity: 0.3
                    pages: "2"
                    position: center
                inputs: [source, mark]
            outputs:
              - id: final
                from: watermark
                path: {}
            "#,
            fixture_pdf().display(),
            fixture_jpg().display(),
            output.display()
        ),
    )
    .unwrap();

    Command::cargo_bin("oxidepdf")
        .unwrap()
        .args(["run", "--workflow", workflow.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::eq(""))
        .stderr(predicate::eq(""));

    assert_eq!(pdf_page_count(&output), 3);
    assert!(page_has_content_operator(&output, 2, "Do"));
}

#[test]
fn render_command_rejects_out_of_range_page() {
    let dir = temp_dir("render_command_rejects_out_of_range_page");
    let output = dir.join("page.png");

    Command::cargo_bin("oxidepdf")
        .unwrap()
        .args([
            "render",
            fixture_pdf().to_str().unwrap(),
            "--page",
            "99",
            "-o",
            output.to_str().unwrap(),
        ])
        .assert()
        .code(3)
        .stdout(predicate::eq(""))
        .stderr(predicate::str::contains("page 99 is out of range"));

    assert!(!output.exists());
}

fn temp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "oxidepdf_cli_integration_{}_{}",
        name,
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn fixture_pdf() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/test.pdf")
        .canonicalize()
        .unwrap()
}

fn fixture_jpg() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/test.jpg")
        .canonicalize()
        .unwrap()
}

fn pdf_page_count(path: &std::path::Path) -> usize {
    lopdf::Document::load(path).unwrap().get_pages().len()
}

fn pdf_page_rotation(path: &std::path::Path, page_number: u32) -> i64 {
    let document = lopdf::Document::load(path).unwrap();
    let page_id = document.get_pages().get(&page_number).copied().unwrap();
    let page = document.get_object(page_id).unwrap().as_dict().unwrap();
    page.get(b"Rotate")
        .and_then(lopdf::Object::as_i64)
        .unwrap_or(0)
}

fn page_has_content_operator(path: &std::path::Path, page_number: u32, operator: &str) -> bool {
    let document = lopdf::Document::load(path).unwrap();
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
