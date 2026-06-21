
#[test]
fn compare_command_writes_json_report() {
    let dir = temp_dir("compare_command_writes_json_report");
    let output = dir.join("compare.json");

    Command::cargo_bin("oxidepdf")
        .unwrap()
        .args([
            "pdf_compare",
            "report",
            fixture_pdf().to_str().unwrap(),
            fixture_pdf().to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::eq(""))
        .stderr(predicate::eq(""));

    let report: serde_json::Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
    assert_eq!(report["equal"], true);
    assert_eq!(report["differences"], serde_json::json!([]));
}

#[test]
fn compare_command_writes_visual_diff_png() {
    let dir = temp_dir("compare_command_writes_visual_diff_png");
    let output = dir.join("diff.png");

    Command::cargo_bin("oxidepdf")
        .unwrap()
        .args([
            "pdf_compare",
            "visual-diff",
            fixture_pdf().to_str().unwrap(),
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
                  pdf_inspect:
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
            "pdf_edit",
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
fn sign_appearance_command_writes_parseable_pdf() {
    let dir = temp_dir("sign_appearance_command_writes_parseable_pdf");
    let output = dir.join("signature-appearance.pdf");

    Command::cargo_bin("oxidepdf")
        .unwrap()
        .args([
            "pdf_sign",
            "appearance",
            fixture_pdf().to_str().unwrap(),
            "--text",
            "SIGNED",
            "--font",
            "Helvetica",
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
                  pdf_edit:
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
            "pdf_inspect",
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

#[test]
fn workflow_crop_pages_sets_crop_box() {
    let dir = temp_dir("workflow_crop_pages_sets_crop_box");
    let output = dir.join("cropped.pdf");
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
              - id: crop
                op:
                  pdf_edit:
                    crop_pages:
                      pages: "1"
                      left: 10.0
                      bottom: 20.0
                      right: 300.0
                      top: 400.0
                inputs: [source]
            outputs:
              - id: final
                from: crop
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

    assert_eq!(
        pdf_page_box(&output, 1, b"CropBox"),
        [10.0, 20.0, 300.0, 400.0]
    );
}

#[test]
fn workflow_page_numbers_writes_selected_labels() {
    let dir = temp_dir("workflow_page_numbers_writes_selected_labels");
    let output = dir.join("numbered.pdf");
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
              - id: number
                op:
                  pdf_edit:
                    page_numbers:
                      pages: "2-3"
                      start: 4
                      prefix: "p"
                      suffix: ""
                      font_size: 10.0
                      position: bottom_right
                inputs: [source]
            outputs:
              - id: final
                from: number
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

    assert!(!page_content_contains(&output, 1, "p4"));
    assert!(page_content_contains(&output, 2, "p4"));
    assert!(page_content_contains(&output, 3, "p5"));
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
