use oxidepdf_core::*;
use schemars::schema_for;
use serde_json::Value;

fn without_properties(mut schema: Value, names: &[&str]) -> Value {
    let Some(properties) = schema
        .get_mut("properties")
        .and_then(serde_json::Value::as_object_mut)
    else {
        return schema;
    };
    for name in names {
        properties.remove(*name);
    }
    if let Some(required) = schema
        .get_mut("required")
        .and_then(serde_json::Value::as_array_mut)
    {
        required.retain(|value| !names.iter().any(|name| value.as_str() == Some(name)));
    }
    schema
}

fn with_standard_font_default(mut schema: Value) -> Value {
    if let Some(font) = schema
        .get_mut("properties")
        .and_then(serde_json::Value::as_object_mut)
        .and_then(|properties| properties.get_mut("font"))
        .and_then(serde_json::Value::as_object_mut)
    {
        font.insert("default".to_owned(), Value::String("Helvetica".to_owned()));
    }
    schema
}

/// JSON Schema for an op's leaf options struct. Ops with no options (unit
/// variants / empty structs) get an empty-object schema.
pub fn op_schema(family: &str, op: &str) -> Value {
    macro_rules! s {
        ($t:ty) => {
            serde_json::to_value(schema_for!($t)).unwrap_or(Value::Object(Default::default()))
        };
    }
    let empty = || Value::Object(Default::default());
    match (family, op) {
        ("PdfEdit", "Merge") => s!(MergeOptions),
        ("PdfEdit", "KeepPages") => s!(SplitOptions),
        ("PdfEdit", "ExtractPages") => s!(PageSelectionOptions),
        ("PdfEdit", "ReorderPages") => s!(ReorderOptions),
        ("PdfEdit", "RotatePages") => s!(RotateOptions),
        ("PdfEdit", "DeletePages") => s!(PageSelectionOptions),
        ("PdfEdit", "DeleteBlankPages") => s!(DeleteBlankPagesOptions),
        ("PdfEdit", "CropPages") => s!(CropPagesOptions),
        ("PdfEdit", "ScalePages") => s!(ScalePagesOptions),
        ("PdfEdit", "SinglePage") => s!(SinglePageOptions),
        ("PdfEdit", "NUp") => s!(NUpOptions),
        ("PdfEdit", "Booklet") => s!(BookletOptions),
        ("PdfEdit", "PageNumbers") => s!(PageNumbersOptions),
        ("PdfEdit", "ImageToPdf") => s!(ImageToPdfOptions),
        ("PdfEdit", "SvgToPdf") => s!(SvgToPdfOptions),
        ("PdfEdit", "Watermark") => {
            with_standard_font_default(without_properties(s!(WatermarkOptions), &["font_path"]))
        }
        ("PdfEdit", "Overlay") => {
            with_standard_font_default(without_properties(s!(OverlayOptions), &["font_path"]))
        }
        ("PdfEdit", "ImageEdit") => s!(ImageEditOptions),
        ("PdfEdit", "Color") => s!(ColorEditOptions),
        ("PdfEdit", "Metadata") => s!(MetadataEditOptions),
        ("PdfEdit", "Outline") => s!(OutlineEditOptions),
        ("PdfEdit", "Attachment") => s!(AttachmentEditOptions),
        ("PdfEdit", "Annotation") => s!(AnnotationEditOptions),
        ("PdfEdit", "FormFill") => s!(FormFillOptions),
        ("PdfEdit", "FormUnlockReadonly") => empty(),
        ("PdfEdit", "FormRemove") => empty(),
        ("PdfEdit", "InteractiveRemove") => s!(InteractiveRemovalOptions),
        ("PdfEdit", "Compression") => s!(CompressionOptions),

        ("PdfInspect", "Render") => s!(RenderOptions),
        ("PdfInspect", "ExtractText") => s!(ExtractTextOptions),
        ("PdfInspect", "Metadata") => s!(MetadataInspectOptions),
        ("PdfInspect", "Outline") => s!(OutlineInspectOptions),
        ("PdfInspect", "Attachments") => s!(AttachmentInspectOptions),
        ("PdfInspect", "AttachmentExtract") => s!(AttachmentExtractOptions),
        ("PdfInspect", "Annotations") => s!(AnnotationInspectOptions),
        ("PdfInspect", "Forms") => s!(FormInspectOptions),
        ("PdfInspect", "Images") => s!(ImageInspectOptions),
        ("PdfInspect", "ImageExtract") => s!(ImageExtractOptions),

        ("PdfSecurity", "Encrypt") => s!(SecurityEncryptOptions),
        ("PdfSecurity", "Decrypt") => s!(SecurityDecryptOptions),
        ("PdfSecurity", "PermissionsGet") => s!(SecurityPermissionGetOptions),
        ("PdfSecurity", "PermissionsSet") => s!(SecurityPermissionSetOptions),

        ("PdfCompare", "Report") => s!(CompareOptions),
        ("PdfCompare", "VisualDiff") => s!(VisualDiffOptions),

        ("PdfSign", "List") => without_properties(s!(SignatureOptions), &["trust_anchors"]),
        ("PdfSign", "Verify") => without_properties(s!(SignatureOptions), &["trust_anchors"]),
        ("PdfSign", "DeleteField") => s!(SignatureDeleteFieldOptions),

        _ => empty(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_overlay_schemas_default_to_standard_pdf_font() {
        for op in ["Watermark", "Overlay"] {
            let schema = op_schema("PdfEdit", op);
            assert_eq!(
                schema["properties"]["font"]["default"],
                Value::String("Helvetica".to_owned()),
                "{op} should not depend on system fonts in the web UI"
            );
        }
    }
}
