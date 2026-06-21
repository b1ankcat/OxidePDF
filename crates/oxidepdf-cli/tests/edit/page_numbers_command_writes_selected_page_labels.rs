
#[tokio::test]
async fn page_numbers_command_writes_selected_page_labels() {
    let dir = temp_dir("page_numbers_command_writes_selected_page_labels");
    let output = dir.join("numbered.pdf");
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let code = run_with_io(
        [
            "oxidepdf",
            "pdf_edit",
            "page-numbers",
            fixture_pdf().to_str().unwrap(),
            "--pages",
            "2-3",
            "--start",
            "7",
            "--prefix",
            "p",
            "--position",
            "bottom-right",
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
    assert!(!pdf_page_content_contains(&output, 1, "p7"));
    assert!(pdf_page_content_contains(&output, 2, "p7"));
    assert!(pdf_page_content_contains(&output, 3, "p8"));
}

#[tokio::test]
async fn img2pdf_command_writes_parseable_pdf() {
    let dir = temp_dir("img2pdf_command_writes_parseable_pdf");
    let output = dir.join("image.pdf");
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let code = run_with_io(
        [
            "oxidepdf",
            "pdf_edit",
            "img2pdf",
            fixture_jpg().to_str().unwrap(),
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
async fn svg2pdf_command_writes_parseable_pdf() {
    let dir = temp_dir("svg2pdf_command_writes_parseable_pdf");
    let input = dir.join("input.svg");
    let output = dir.join("svg.pdf");
    fs::write(&input, simple_svg()).unwrap();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let code = run_with_io(
        [
            "oxidepdf",
            "pdf_edit",
            "svg2pdf",
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
async fn watermark_text_command_writes_parseable_pdf() {
    let dir = temp_dir("watermark_text_command_writes_parseable_pdf");
    let output = dir.join("watermarked.pdf");
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let code = run_with_io(
        [
            "oxidepdf",
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
        ],
        [],
        &mut stdout,
        &mut stderr,
    )
    .await;

    assert_eq!(code, 0);
    assert_eq!(stdout, b"");
    assert_eq!(stderr, b"");
    assert_eq!(pdf_page_count(&output), 3);
    assert!(page_has_content_operator(&output, 1, "Tj"));
}

#[tokio::test]
async fn watermark_text_command_returns_font_resolution_for_missing_font() {
    let dir = temp_dir("watermark_text_command_returns_font_resolution_for_missing_font");
    let output = dir.join("watermarked.pdf");
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let code = run_with_io(
        [
            "oxidepdf",
            "pdf_edit",
            "watermark",
            fixture_pdf().to_str().unwrap(),
            "--kind",
            "text",
            "--text",
            "DRAFT",
            "--font",
            "Definitely Missing Font Family",
            "-o",
            output.to_str().unwrap(),
        ],
        [],
        &mut stdout,
        &mut stderr,
    )
    .await;

    assert_eq!(code, 70);
    assert_eq!(stdout, b"");
    assert!(!output.exists());
    assert!(
        String::from_utf8(stderr)
            .unwrap()
            .contains("font_resolution")
    );
}

#[tokio::test]
async fn watermark_image_command_writes_parseable_pdf() {
    let dir = temp_dir("watermark_image_command_writes_parseable_pdf");
    let output = dir.join("watermarked.pdf");
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let code = run_with_io(
        [
            "oxidepdf",
            "pdf_edit",
            "watermark",
            fixture_pdf().to_str().unwrap(),
            "--kind",
            "image",
            "--watermark",
            fixture_jpg().to_str().unwrap(),
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
    assert!(page_has_content_operator(&output, 2, "Do"));
}

#[tokio::test]
async fn watermark_svg_command_writes_parseable_pdf() {
    let dir = temp_dir("watermark_svg_command_writes_parseable_pdf");
    let input = dir.join("watermark.svg");
    let output = dir.join("watermarked.pdf");
    fs::write(&input, simple_svg()).unwrap();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let code = run_with_io(
        [
            "oxidepdf",
            "pdf_edit",
            "watermark",
            fixture_pdf().to_str().unwrap(),
            "--kind",
            "svg",
            "--watermark",
            input.to_str().unwrap(),
            "--pages",
            "3",
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
    assert!(page_has_content_operator(&output, 3, "Do"));
}
