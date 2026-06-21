#[allow(unused_imports)]
use common::*;
#[allow(unused_imports)]
use oxidepdf_core::*;

#[test]
fn overlay_pdf_page_and_signature_appearance_are_visual_only() {
    let pdf = empty_page_pdf();
    let overlay = fixture_pdf();
    let overlaid = overlay_pdf_artifacts(
        &[
            Artifact::pdf(&pdf).unwrap(),
            Artifact::pdf(overlay).unwrap(),
        ],
        &OverlayOptions {
            kind: OverlayKind::PdfPage,
            text: None,
            font: None,
            font_path: None,
            font_size: None,
            opacity: Some(1.0),
            rotation: None,
            position: Some("center".to_owned()),
            pages: Some("1".to_owned()),
            scale: Some(0.5),
            rasterize: false,
            source_page: Some(1),
        },
        &ResourceLimits::default(),
    )
    .unwrap();
    let document = lopdf::Document::load_mem(&overlaid.bytes).unwrap();
    assert!(page_resources(&document, 1).has(b"XObject"));
    assert!(page_content_contains_operator(&document, 1, "Do"));

    let appearance = overlay_pdf_artifacts(
        &[Artifact::pdf(&pdf).unwrap()],
        &OverlayOptions {
            kind: OverlayKind::SignatureAppearance,
            text: Some("Ada Lovelace".to_owned()),
            font: Some("Helvetica".to_owned()),
            font_path: None,
            font_size: Some(24.0),
            opacity: Some(1.0),
            rotation: None,
            position: Some("bottom_right".to_owned()),
            pages: Some("1".to_owned()),
            scale: None,
            rasterize: false,
            source_page: None,
        },
        &ResourceLimits::default(),
    )
    .unwrap();
    let trust_anchors = write_test_trust_anchors("signature_appearance_report");
    let report = verify_pdf_signatures(
        &appearance.bytes,
        &SignatureOptions {
            mode: SignatureMode::Verify,
            trust_anchors: Some(trust_anchors),
        },
        &ResourceLimits::default(),
    )
    .unwrap();
    let report: serde_json::Value = serde_json::from_str(&report.text).unwrap();
    assert!(report["signatures"].as_array().unwrap().is_empty());
}

#[test]
fn image_resources_list_add_replace_delete_and_extract() {
    let image = fixture_jpg();
    let pdf = image_artifacts_to_pdf(
        &[Artifact::image(image).unwrap()],
        &ImageToPdfOptions {
            layout: Some("original_size".to_owned()),
        },
        &ResourceLimits::default(),
    )
    .unwrap();

    let report = inspect_pdf_images(&pdf.bytes, &ImageInspectOptions::default())
        .unwrap()
        .text;
    let report: serde_json::Value = serde_json::from_str(&report).unwrap();
    assert_eq!(report["images"][0]["name"], "Im1");

    let extracted = extract_pdf_image(
        &pdf.bytes,
        &ImageExtractOptions {
            name: "Im1".to_owned(),
        },
        &ResourceLimits::default(),
    )
    .unwrap();
    assert!(!extracted.bytes.is_empty());

    let added = edit_pdf_images_artifacts(
        &[
            Artifact::pdf(empty_page_pdf()).unwrap(),
            Artifact::image(image).unwrap(),
        ],
        &ImageEditOptions {
            action: ImageEditAction::Add,
            name: Some("Logo".to_owned()),
            page: Some(1),
        },
        &ResourceLimits::default(),
    )
    .unwrap();
    let added_doc = lopdf::Document::load_mem(&added.bytes).unwrap();
    assert!(page_resources(&added_doc, 1).has(b"XObject"));

    let replaced = edit_pdf_images_artifacts(
        &[
            Artifact::pdf(&added.bytes).unwrap(),
            Artifact::image(image).unwrap(),
        ],
        &ImageEditOptions {
            action: ImageEditAction::Replace,
            name: Some("Logo".to_owned()),
            page: None,
        },
        &ResourceLimits::default(),
    )
    .unwrap();
    let report = inspect_pdf_images(&replaced.bytes, &ImageInspectOptions::default())
        .unwrap()
        .text;
    let report: serde_json::Value = serde_json::from_str(&report).unwrap();
    assert_eq!(report["images"][0]["name"], "Logo");

    let deleted = edit_pdf_images_artifacts(
        &[Artifact::pdf(&replaced.bytes).unwrap()],
        &ImageEditOptions {
            action: ImageEditAction::Delete,
            name: Some("Logo".to_owned()),
            page: None,
        },
        &ResourceLimits::default(),
    )
    .unwrap();
    let report = inspect_pdf_images(&deleted.bytes, &ImageInspectOptions::default())
        .unwrap()
        .text;
    let report: serde_json::Value = serde_json::from_str(&report).unwrap();
    assert!(report["images"].as_array().unwrap().is_empty());
}

#[test]
fn image_resources_reject_malformed_xobject_dictionary() {
    let pdf = pdf_with_malformed_xobject_resources();

    let err = inspect_pdf_images(&pdf, &ImageInspectOptions::default()).unwrap_err();
    assert!(matches!(err, OxideError::ParsePdf));

    let err = edit_pdf_images_artifacts(
        &[Artifact::pdf(&pdf).unwrap()],
        &ImageEditOptions {
            action: ImageEditAction::Delete,
            name: Some("Logo".to_owned()),
            page: None,
        },
        &ResourceLimits::default(),
    )
    .unwrap_err();
    assert!(matches!(err, OxideError::ParsePdf));
}

#[test]
fn color_operations_rewrite_simple_content_and_reject_rasterize_pages() {
    let pdf = pdf_with_rgb_fill_content();
    let inverted = edit_pdf_colors(
        &pdf,
        &ColorEditOptions {
            action: ColorEditAction::Invert,
            pages: Some("1".to_owned()),
            from: None,
            to: None,
            factor: None,
            rasterize_pages: false,
        },
        &ResourceLimits::default(),
    )
    .unwrap();
    let document = lopdf::Document::load_mem(&inverted.bytes).unwrap();
    assert_eq!(page_rgb_operator(&document, 1, "rg"), Some([0.0, 1.0, 1.0]));

    let replaced = edit_pdf_colors(
        &pdf,
        &ColorEditOptions {
            action: ColorEditAction::Replace,
            pages: Some("1".to_owned()),
            from: Some([1.0, 0.0, 0.0]),
            to: Some([0.0, 0.0, 1.0]),
            factor: None,
            rasterize_pages: false,
        },
        &ResourceLimits::default(),
    )
    .unwrap();
    let document = lopdf::Document::load_mem(&replaced.bytes).unwrap();
    assert_eq!(page_rgb_operator(&document, 1, "rg"), Some([0.0, 0.0, 1.0]));

    let err = edit_pdf_colors(
        &pdf,
        &ColorEditOptions {
            action: ColorEditAction::Contrast,
            pages: None,
            from: None,
            to: None,
            factor: Some(1.25),
            rasterize_pages: true,
        },
        &ResourceLimits::default(),
    )
    .unwrap_err();
    assert!(matches!(err, OxideError::UnsupportedPdfFeature { .. }));
    assert!(err.to_string().contains("rasterize_pages"));
}

#[test]
fn image_artifacts_to_pdf_converts_real_jpeg() {
    let image = fixture_jpg();

    let pdf = image_artifacts_to_pdf(
        &[Artifact::image(image).unwrap()],
        &ImageToPdfOptions::default(),
        &ResourceLimits::default(),
    )
    .unwrap();
    let document = lopdf::Document::load_mem(&pdf.bytes).unwrap();

    assert_eq!(document.get_pages().len(), 1);
}

#[test]
fn image_artifacts_to_pdf_writes_one_page_per_image() {
    let image = fixture_jpg();

    let pdf = image_artifacts_to_pdf(
        &[
            Artifact::image(image).unwrap(),
            Artifact::image(image).unwrap(),
        ],
        &ImageToPdfOptions::default(),
        &ResourceLimits::default(),
    )
    .unwrap();
    let document = lopdf::Document::load_mem(&pdf.bytes).unwrap();

    assert_eq!(document.get_pages().len(), 2);
}

#[test]
fn image_artifacts_to_pdf_enforces_pixel_limit() {
    let image = fixture_jpg();
    let limits = ResourceLimits {
        max_pixels: Some(1),
        ..ResourceLimits::default()
    };

    let err = image_artifacts_to_pdf(
        &[Artifact::image(image).unwrap()],
        &ImageToPdfOptions::default(),
        &limits,
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
fn image_artifacts_to_pdf_rejects_unknown_image_format() {
    let err = image_artifacts_to_pdf(
        &[Artifact::image(b"not an image").unwrap()],
        &ImageToPdfOptions::default(),
        &ResourceLimits::default(),
    )
    .unwrap_err();

    assert_eq!(err, OxideError::ImageDecode);
}
