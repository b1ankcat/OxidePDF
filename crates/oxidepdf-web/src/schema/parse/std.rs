use oxidepdf_core::*;

#[derive(Debug)]
pub(crate) enum ParseOpError {
    UnknownOp,
    Json(serde_json::Error),
}

impl std::fmt::Display for ParseOpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownOp => write!(f, "unknown family/op"),
            Self::Json(e) => write!(f, "options_json: {e}"),
        }
    }
}

impl std::error::Error for ParseOpError {}

fn is_standard_pdf_font(family: &str) -> bool {
    matches!(
        family,
        "Courier"
            | "Courier-Bold"
            | "Courier-Oblique"
            | "Courier-BoldOblique"
            | "Helvetica"
            | "Helvetica-Bold"
            | "Helvetica-Oblique"
            | "Helvetica-BoldOblique"
            | "Times-Roman"
            | "Times-Bold"
            | "Times-Italic"
            | "Times-BoldItalic"
            | "Symbol"
            | "ZapfDingbats"
    )
}

fn rejected_font_family(family: Option<&str>) -> bool {
    family.is_some_and(|family| !is_standard_pdf_font(family))
}

fn text_watermark_needs_font(kind: WatermarkKind) -> bool {
    kind == WatermarkKind::Text
}

fn text_overlay_needs_font(kind: OverlayKind) -> bool {
    matches!(
        kind,
        OverlayKind::Watermark
            | OverlayKind::Text
            | OverlayKind::Stamp
            | OverlayKind::SignatureAppearance
    )
}

fn apply_standard_font(font: &mut Option<String>) {
    if font.is_none() {
        *font = Some("Helvetica".to_owned());
    }
}

fn sanitize_watermark_options(options: &mut WatermarkOptions) -> Result<(), ParseOpError> {
    if options.font_path.is_some() || rejected_font_family(options.font.as_deref()) {
        return Err(ParseOpError::UnknownOp);
    }
    if text_watermark_needs_font(options.kind) {
        apply_standard_font(&mut options.font);
    }
    Ok(())
}

fn sanitize_overlay_options(options: &mut OverlayOptions) -> Result<(), ParseOpError> {
    if options.font_path.is_some() || rejected_font_family(options.font.as_deref()) {
        return Err(ParseOpError::UnknownOp);
    }
    if text_overlay_needs_font(options.kind) {
        apply_standard_font(&mut options.font);
    }
    Ok(())
}

fn signature_options_without_trust_anchors(json: &str) -> Result<SignatureOptions, ParseOpError> {
    let options: SignatureOptions = serde_json::from_str(json).map_err(ParseOpError::Json)?;
    if options.trust_anchors.is_some() {
        return Err(ParseOpError::UnknownOp);
    }
    Ok(options)
}

pub(crate) fn parse_op(family: &str, op: &str, json: &str) -> Result<OperatorSpec, ParseOpError> {
    fn de<T: serde::de::DeserializeOwned>(j: &str) -> Result<T, ParseOpError> {
        serde_json::from_str(j).map_err(ParseOpError::Json)
    }
    Ok(match (family, op) {
        ("PdfEdit", "Merge") => OperatorSpec::PdfEdit(PdfEditOptions::Merge(de(json)?)),
        ("PdfEdit", "KeepPages") => OperatorSpec::PdfEdit(PdfEditOptions::KeepPages(de(json)?)),
        ("PdfEdit", "ExtractPages") => {
            OperatorSpec::PdfEdit(PdfEditOptions::ExtractPages(de(json)?))
        }
        ("PdfEdit", "ReorderPages") => {
            OperatorSpec::PdfEdit(PdfEditOptions::ReorderPages(de(json)?))
        }
        ("PdfEdit", "RotatePages") => OperatorSpec::PdfEdit(PdfEditOptions::RotatePages(de(json)?)),
        ("PdfEdit", "DeletePages") => OperatorSpec::PdfEdit(PdfEditOptions::DeletePages(de(json)?)),
        ("PdfEdit", "DeleteBlankPages") => {
            OperatorSpec::PdfEdit(PdfEditOptions::DeleteBlankPages(de(json)?))
        }
        ("PdfEdit", "CropPages") => OperatorSpec::PdfEdit(PdfEditOptions::CropPages(de(json)?)),
        ("PdfEdit", "ScalePages") => OperatorSpec::PdfEdit(PdfEditOptions::ScalePages(de(json)?)),
        ("PdfEdit", "SinglePage") => OperatorSpec::PdfEdit(PdfEditOptions::SinglePage(de(json)?)),
        ("PdfEdit", "NUp") => OperatorSpec::PdfEdit(PdfEditOptions::NUp(de(json)?)),
        ("PdfEdit", "Booklet") => OperatorSpec::PdfEdit(PdfEditOptions::Booklet(de(json)?)),
        ("PdfEdit", "PageNumbers") => OperatorSpec::PdfEdit(PdfEditOptions::PageNumbers(de(json)?)),
        ("PdfEdit", "ImageToPdf") => OperatorSpec::PdfEdit(PdfEditOptions::ImageToPdf(de(json)?)),
        ("PdfEdit", "SvgToPdf") => OperatorSpec::PdfEdit(PdfEditOptions::SvgToPdf(de(json)?)),
        ("PdfEdit", "Watermark") => {
            let mut options = de(json)?;
            sanitize_watermark_options(&mut options)?;
            OperatorSpec::PdfEdit(PdfEditOptions::Watermark(options))
        }
        ("PdfEdit", "Overlay") => {
            let mut options = de(json)?;
            sanitize_overlay_options(&mut options)?;
            OperatorSpec::PdfEdit(PdfEditOptions::Overlay(options))
        }
        ("PdfEdit", "ImageEdit") => OperatorSpec::PdfEdit(PdfEditOptions::ImageEdit(de(json)?)),
        ("PdfEdit", "Color") => OperatorSpec::PdfEdit(PdfEditOptions::Color(de(json)?)),
        ("PdfEdit", "Metadata") => OperatorSpec::PdfEdit(PdfEditOptions::Metadata(de(json)?)),
        ("PdfEdit", "Outline") => OperatorSpec::PdfEdit(PdfEditOptions::Outline(de(json)?)),
        ("PdfEdit", "Attachment") => OperatorSpec::PdfEdit(PdfEditOptions::Attachment(de(json)?)),
        ("PdfEdit", "Annotation") => OperatorSpec::PdfEdit(PdfEditOptions::Annotation(de(json)?)),
        ("PdfEdit", "FormFill") => OperatorSpec::PdfEdit(PdfEditOptions::FormFill(de(json)?)),
        ("PdfEdit", "FormUnlockReadonly") => {
            OperatorSpec::PdfEdit(PdfEditOptions::FormUnlockReadonly)
        }
        ("PdfEdit", "FormRemove") => OperatorSpec::PdfEdit(PdfEditOptions::FormRemove),
        ("PdfEdit", "InteractiveRemove") => {
            OperatorSpec::PdfEdit(PdfEditOptions::InteractiveRemove(de(json)?))
        }
        ("PdfEdit", "Compression") => OperatorSpec::PdfEdit(PdfEditOptions::Compression(de(json)?)),

        ("PdfInspect", "Render") => OperatorSpec::PdfInspect(PdfInspectOptions::Render(de(json)?)),
        ("PdfInspect", "ExtractText") => {
            OperatorSpec::PdfInspect(PdfInspectOptions::ExtractText(de(json)?))
        }
        ("PdfInspect", "Metadata") => {
            OperatorSpec::PdfInspect(PdfInspectOptions::Metadata(de(json)?))
        }
        ("PdfInspect", "Outline") => {
            OperatorSpec::PdfInspect(PdfInspectOptions::Outline(de(json)?))
        }
        ("PdfInspect", "Attachments") => {
            OperatorSpec::PdfInspect(PdfInspectOptions::Attachments(de(json)?))
        }
        ("PdfInspect", "AttachmentExtract") => {
            OperatorSpec::PdfInspect(PdfInspectOptions::AttachmentExtract(de(json)?))
        }
        ("PdfInspect", "Annotations") => {
            OperatorSpec::PdfInspect(PdfInspectOptions::Annotations(de(json)?))
        }
        ("PdfInspect", "Forms") => OperatorSpec::PdfInspect(PdfInspectOptions::Forms(de(json)?)),
        ("PdfInspect", "Images") => OperatorSpec::PdfInspect(PdfInspectOptions::Images(de(json)?)),
        ("PdfInspect", "ImageExtract") => {
            OperatorSpec::PdfInspect(PdfInspectOptions::ImageExtract(de(json)?))
        }

        ("PdfSecurity", "Encrypt") => {
            OperatorSpec::PdfSecurity(PdfSecurityOptions::Encrypt(de(json)?))
        }
        ("PdfSecurity", "Decrypt") => {
            OperatorSpec::PdfSecurity(PdfSecurityOptions::Decrypt(de(json)?))
        }
        ("PdfSecurity", "PermissionsGet") => {
            OperatorSpec::PdfSecurity(PdfSecurityOptions::PermissionsGet(de(json)?))
        }
        ("PdfSecurity", "PermissionsSet") => {
            OperatorSpec::PdfSecurity(PdfSecurityOptions::PermissionsSet(de(json)?))
        }

        ("PdfCompare", "Report") => OperatorSpec::PdfCompare(PdfCompareOptions::Report(de(json)?)),
        ("PdfCompare", "VisualDiff") => {
            OperatorSpec::PdfCompare(PdfCompareOptions::VisualDiff(de(json)?))
        }

        ("PdfSign", "List") => OperatorSpec::PdfSign(PdfSignOptions::List(
            signature_options_without_trust_anchors(json)?,
        )),
        ("PdfSign", "Verify") => OperatorSpec::PdfSign(PdfSignOptions::Verify(
            signature_options_without_trust_anchors(json)?,
        )),
        ("PdfSign", "DeleteField") => OperatorSpec::PdfSign(PdfSignOptions::DeleteField(de(json)?)),

        _ => return Err(ParseOpError::UnknownOp),
    })
}
