
#[test]
fn watermark_pdf_adds_svg_watermark_as_vector_xobject() {
    let pdf = fixture_pdf();
    let svg = simple_svg();

    let watermarked = watermark_pdf_artifacts(
        &[Artifact::pdf(pdf).unwrap(), Artifact::svg(svg).unwrap()],
        &WatermarkOptions {
            kind: WatermarkKind::Svg,
            text: None,
            font: None,
            font_path: None,
            font_size: None,
            opacity: Some(0.5),
            rotation: None,
            position: Some("top_left".to_owned()),
            pages: Some("3".to_owned()),
            scale: Some(0.2),
            rasterize: false,
        },
        &ResourceLimits::default(),
    )
    .unwrap();
    let document = lopdf::Document::load_mem(&watermarked.bytes).unwrap();

    assert!(page_resources(&document, 3).has(b"XObject"));
    assert!(page_content_contains_operator(&document, 3, "Do"));
    assert!(page_xobject_subtypes(&document, 3).contains(&b"Form".to_vec()));
    let form_operators = page_form_xobject_operators(&document, 3);
    assert!(form_operators.iter().any(|operator| operator == "f"));
    assert!(
        !form_operators
            .windows(2)
            .any(|operators| operators == ["re", "S"])
    );
}

#[test]
fn watermark_pdf_rasterizes_svg_only_when_requested() {
    let pdf = fixture_pdf();
    let svg = simple_svg();

    let watermarked = watermark_pdf_artifacts(
        &[Artifact::pdf(pdf).unwrap(), Artifact::svg(svg).unwrap()],
        &WatermarkOptions {
            kind: WatermarkKind::Svg,
            text: None,
            font: None,
            font_path: None,
            font_size: None,
            opacity: Some(0.5),
            rotation: None,
            position: Some("top_left".to_owned()),
            pages: Some("1".to_owned()),
            scale: Some(0.2),
            rasterize: true,
        },
        &ResourceLimits::default(),
    )
    .unwrap();
    let document = lopdf::Document::load_mem(&watermarked.bytes).unwrap();

    assert!(page_xobject_subtypes(&document, 1).contains(&b"Image".to_vec()));
}

#[test]
fn watermark_pdf_rejects_malformed_svg_without_panic() {
    let pdf = fixture_pdf();

    let err = watermark_pdf_artifacts(
        &[
            Artifact::pdf(pdf).unwrap(),
            Artifact::svg(b"<svg><broken>").unwrap(),
        ],
        &WatermarkOptions {
            kind: WatermarkKind::Svg,
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
        &ResourceLimits::default(),
    )
    .unwrap_err();

    assert_eq!(err, OxideError::SvgParse);
}
