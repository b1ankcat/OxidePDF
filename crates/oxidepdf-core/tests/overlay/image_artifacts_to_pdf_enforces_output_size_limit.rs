
#[test]
fn image_artifacts_to_pdf_enforces_output_size_limit() {
    let image = fixture_jpg();

    let err = image_artifacts_to_pdf(
        &[Artifact::image(image).unwrap()],
        &ImageToPdfOptions::default(),
        &ResourceLimits {
            max_output_bytes: Some(1),
            ..ResourceLimits::default()
        },
    )
    .unwrap_err();

    assert_eq!(
        err,
        OxideError::ResourceLimitExceeded {
            limit: "max_output_bytes".to_owned()
        }
    );
}

#[test]
fn svg_to_pdf_converts_vector_svg_to_parseable_pdf() {
    let svg = br##"<svg xmlns="http://www.w3.org/2000/svg" width="120" height="80">
            <rect x="10" y="10" width="100" height="60" fill="#0077cc"/>
        </svg>"##;

    let pdf = svg_to_pdf(svg, &SvgToPdfOptions::default(), &ResourceLimits::default()).unwrap();
    let document = lopdf::Document::load_mem(&pdf.bytes).unwrap();

    assert_eq!(document.get_pages().len(), 1);
}

#[test]
fn svg_to_pdf_rasterizes_only_when_requested() {
    let svg = br##"<svg xmlns="http://www.w3.org/2000/svg" width="120" height="80">
            <circle cx="60" cy="40" r="30" fill="#ef4444"/>
        </svg>"##;

    let pdf = svg_to_pdf(
        svg,
        &SvgToPdfOptions { rasterize: true },
        &ResourceLimits::default(),
    )
    .unwrap();
    let document = lopdf::Document::load_mem(&pdf.bytes).unwrap();

    assert_eq!(document.get_pages().len(), 1);
}

#[test]
fn svg_to_pdf_rejects_invalid_svg() {
    let err = svg_to_pdf(
        b"<svg><broken>",
        &SvgToPdfOptions::default(),
        &ResourceLimits::default(),
    )
    .unwrap_err();

    assert_eq!(err, OxideError::SvgParse);
}

#[test]
fn svg_to_pdf_rejects_non_svg_magic_bytes() {
    let err = svg_to_pdf(
        b"%PDF-1.7\nnot svg",
        &SvgToPdfOptions::default(),
        &ResourceLimits::default(),
    )
    .unwrap_err();

    assert_eq!(err, OxideError::SvgParse);
}

#[test]
fn render_pdf_page_writes_png_for_real_pdf() {
    let pdf = fixture_pdf();

    let image = render_pdf_page(
        pdf,
        &RenderOptions {
            page: 1,
            format: Some("png".to_owned()),
            scale: Some(1.0),
        },
        &ResourceLimits::default(),
    )
    .unwrap();
    let decoded = image::load_from_memory(&image.bytes).unwrap();

    assert!(decoded.width() > 0);
    assert!(decoded.height() > 0);
}

#[test]
fn render_pdf_page_rejects_out_of_range_page() {
    let pdf = fixture_pdf();

    let err = render_pdf_page(
        pdf,
        &RenderOptions {
            page: 99,
            format: Some("png".to_owned()),
            scale: Some(1.0),
        },
        &ResourceLimits::default(),
    )
    .unwrap_err();

    assert!(matches!(err, OxideError::InvalidInput { .. }));
    assert!(err.to_string().contains("page 99 is out of range"));
}

#[test]
fn extract_text_from_pdf_returns_plain_text_for_real_pdf() {
    let pdf = fixture_pdf();

    let text = extract_text_from_pdf(
        pdf,
        &ExtractTextOptions::default(),
        &ResourceLimits::default(),
    )
    .unwrap();

    assert!(!text.text.trim().is_empty());
    assert!(text.diagnostics.is_empty());
}

#[test]
fn extract_text_from_pdf_rejects_pdf_without_text_layer() {
    let pdf = empty_page_pdf();

    let err = extract_text_from_pdf(
        &pdf,
        &ExtractTextOptions::default(),
        &ResourceLimits::default(),
    )
    .unwrap_err();

    assert!(matches!(err, OxideError::InvalidInput { .. }));
    assert!(err.to_string().contains("no extractable text layer"));
}

#[test]
fn extract_text_from_pdf_rejects_unknown_format() {
    let pdf = fixture_pdf();

    let err = extract_text_from_pdf(
        pdf,
        &ExtractTextOptions {
            format: Some("json".to_owned()),
        },
        &ResourceLimits::default(),
    )
    .unwrap_err();

    assert!(matches!(err, OxideError::InvalidInput { .. }));
    assert!(
        err.to_string()
            .contains("unsupported text extraction format")
    );
}

#[test]
fn extract_text_from_pdf_rejects_non_pdf_magic_bytes() {
    let err = extract_text_from_pdf(
        b"<svg></svg>",
        &ExtractTextOptions::default(),
        &ResourceLimits::default(),
    )
    .unwrap_err();

    assert!(matches!(err, OxideError::InvalidInput { .. }));
    assert!(err.to_string().contains("expected PDF"));
}

#[test]
fn watermark_pdf_adds_text_watermark_to_selected_page() {
    let pdf = blank_three_page_pdf();

    let watermarked = watermark_pdf_artifacts(
        &[Artifact::pdf(&pdf).unwrap()],
        &WatermarkOptions {
            kind: WatermarkKind::Text,
            text: Some("DRAFT".to_owned()),
            font: Some("DejaVu Sans".to_owned()),
            font_path: None,
            font_size: Some(36.0),
            opacity: Some(0.4),
            rotation: Some(30.0),
            position: Some("center".to_owned()),
            pages: Some("1".to_owned()),
            scale: None,
            rasterize: false,
        },
        &ResourceLimits::default(),
    )
    .unwrap();
    let document = lopdf::Document::load_mem(&watermarked.bytes).unwrap();

    assert_eq!(document.get_pages().len(), 3);
    assert!(page_resources(&document, 1).has(b"Font"));
    assert!(page_resources(&document, 1).has(b"ExtGState"));
    assert!(page_content_contains_operator(&document, 1, "Tj"));
    assert!(!page_content_contains_operator(&document, 2, "Tj"));
}

#[test]
fn watermark_pdf_rejects_missing_text_font_without_substitution() {
    let pdf = fixture_pdf();

    let err = watermark_pdf_artifacts(
        &[Artifact::pdf(pdf).unwrap()],
        &WatermarkOptions {
            kind: WatermarkKind::Text,
            text: Some("DRAFT".to_owned()),
            font: Some("Definitely Missing Font Family".to_owned()),
            font_path: None,
            font_size: Some(36.0),
            opacity: Some(0.4),
            rotation: None,
            position: Some("center".to_owned()),
            pages: Some("1".to_owned()),
            scale: None,
            rasterize: false,
        },
        &ResourceLimits::default(),
    )
    .unwrap_err();

    assert_eq!(err, OxideError::FontResolution);
}

#[test]
fn watermark_pdf_enforces_image_pixel_limit() {
    let pdf = fixture_pdf();
    let image = fixture_jpg();

    let err = watermark_pdf_artifacts(
        &[Artifact::pdf(pdf).unwrap(), Artifact::image(image).unwrap()],
        &WatermarkOptions {
            kind: WatermarkKind::Image,
            text: None,
            font: None,
            font_path: None,
            font_size: None,
            opacity: None,
            rotation: None,
            position: None,
            pages: None,
            scale: None,
            rasterize: false,
        },
        &ResourceLimits {
            max_pixels: Some(1),
            ..ResourceLimits::default()
        },
    )
    .unwrap_err();

    assert_eq!(
        err,
        OxideError::ResourceLimitExceeded {
            limit: "max_pixels".to_owned()
        }
    );
}

#[test]
fn watermark_pdf_adds_image_watermark_to_selected_page() {
    let pdf = fixture_pdf();
    let image = fixture_jpg();

    let watermarked = watermark_pdf_artifacts(
        &[Artifact::pdf(pdf).unwrap(), Artifact::image(image).unwrap()],
        &WatermarkOptions {
            kind: WatermarkKind::Image,
            text: None,
            font: None,
            font_path: None,
            font_size: None,
            opacity: Some(0.3),
            rotation: Some(15.0),
            position: Some("bottom_right".to_owned()),
            pages: Some("2".to_owned()),
            scale: Some(0.25),
            rasterize: false,
        },
        &ResourceLimits::default(),
    )
    .unwrap();
    let document = lopdf::Document::load_mem(&watermarked.bytes).unwrap();

    assert!(page_resources(&document, 2).has(b"XObject"));
    assert!(page_content_contains_operator(&document, 2, "Do"));
    assert!(!page_content_contains_operator(&document, 1, "Do"));
}
