
#[tokio::test]
async fn form_commands_fill_inspect_unlock_and_remove() {
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
    )
    .await;
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
    )
    .await;
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
    )
    .await;
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
    )
    .await;
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
    )
    .await;
    assert_eq!(code, 0);
    let report: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(empty_report).unwrap()).unwrap();
    assert!(report["fields"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn stamp_overlay_image_and_color_commands_write_expected_outputs() {
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
    )
    .await;
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
    )
    .await;
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
    )
    .await;
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
    )
    .await;
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
    )
    .await;
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
    )
    .await;
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
    )
    .await;
    assert_eq!(code, 0);
    assert_eq!(pdf_rgb_operator(&colored, 1, "rg"), Some([0.0, 1.0, 1.0]));
}
