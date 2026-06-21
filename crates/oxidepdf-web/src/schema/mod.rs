mod op_schema;
mod parse;

use op_schema::op_schema;
pub(crate) use parse::parse_op;

use serde::Serialize;
use serde_json::Value;

#[derive(Serialize, Clone)]
pub struct OpMeta {
    pub name: &'static str,
    pub multi_input: bool,
    pub output_type: &'static str,
    /// JSON Schema for this op's options struct (drives the parameter form).
    pub schema: Value,
}

#[derive(Serialize, Clone)]
pub struct FamilySchema {
    pub name: &'static str,
    pub ops: Vec<OpMeta>,
}

fn family(
    name: &'static str,
    ops: Vec<(&'static str, bool, &'static str)>,
) -> Result<FamilySchema, serde_json::Error> {
    let ops = ops
        .into_iter()
        .map(|(op_name, multi_input, output_type)| {
            Ok(OpMeta {
                name: op_name,
                multi_input,
                output_type,
                schema: op_schema(name, op_name)?,
            })
        })
        .collect::<Result<Vec<_>, serde_json::Error>>()?;
    Ok(FamilySchema { name, ops })
}

pub(crate) fn schema() -> Result<Vec<FamilySchema>, serde_json::Error> {
    Ok(vec![
        family(
            "PdfEdit",
            vec![
                ("Merge", true, "pdf"),
                ("KeepPages", false, "pdf"),
                ("ExtractPages", false, "pdf"),
                ("ReorderPages", false, "pdf"),
                ("RotatePages", false, "pdf"),
                ("DeletePages", false, "pdf"),
                ("DeleteBlankPages", false, "pdf"),
                ("CropPages", false, "pdf"),
                ("ScalePages", false, "pdf"),
                ("SinglePage", false, "pdf"),
                ("NUp", false, "pdf"),
                ("Booklet", false, "pdf"),
                ("PageNumbers", false, "pdf"),
                ("ImageToPdf", true, "pdf"),
                ("SvgToPdf", true, "pdf"),
                ("Watermark", false, "pdf"),
                ("Overlay", true, "pdf"),
                ("ImageEdit", false, "pdf"),
                ("Color", false, "pdf"),
                ("Metadata", false, "pdf"),
                ("Outline", false, "pdf"),
                ("Attachment", true, "pdf"),
                ("Annotation", false, "pdf"),
                ("FormFill", false, "pdf"),
                ("FormUnlockReadonly", false, "pdf"),
                ("FormRemove", false, "pdf"),
                ("InteractiveRemove", false, "pdf"),
                ("Compression", false, "pdf"),
            ],
        )?,
        family(
            "PdfInspect",
            vec![
                ("Render", false, "image"),
                ("ExtractText", false, "text"),
                ("Metadata", false, "text"),
                ("Outline", false, "text"),
                ("Attachments", false, "text"),
                ("AttachmentExtract", false, "bytes"),
                ("Annotations", false, "text"),
                ("Forms", false, "text"),
                ("Images", false, "text"),
                ("ImageExtract", false, "image"),
            ],
        )?,
        family(
            "PdfSecurity",
            vec![
                ("Encrypt", false, "pdf"),
                ("Decrypt", false, "pdf"),
                ("PermissionsGet", false, "text"),
                ("PermissionsSet", false, "pdf"),
            ],
        )?,
        family(
            "PdfCompare",
            vec![("Report", true, "text"), ("VisualDiff", true, "image")],
        )?,
        family(
            "PdfSign",
            vec![
                ("List", false, "text"),
                ("Verify", false, "text"),
                ("DeleteField", false, "pdf"),
            ],
        )?,
    ])
}
