mod op_schema;
mod parse;

use op_schema::op_schema;
pub use parse::parse_op;

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

pub fn schema() -> Vec<FamilySchema> {
    let mut families = vec![
        FamilySchema {
            name: "PdfEdit",
            ops: vec![
                OpMeta {
                    name: "Merge",
                    multi_input: true,
                    output_type: "pdf",
                    schema: Value::Null,
                },
                OpMeta {
                    name: "KeepPages",
                    multi_input: false,
                    output_type: "pdf",
                    schema: Value::Null,
                },
                OpMeta {
                    name: "ExtractPages",
                    multi_input: false,
                    output_type: "pdf",
                    schema: Value::Null,
                },
                OpMeta {
                    name: "ReorderPages",
                    multi_input: false,
                    output_type: "pdf",
                    schema: Value::Null,
                },
                OpMeta {
                    name: "RotatePages",
                    multi_input: false,
                    output_type: "pdf",
                    schema: Value::Null,
                },
                OpMeta {
                    name: "DeletePages",
                    multi_input: false,
                    output_type: "pdf",
                    schema: Value::Null,
                },
                OpMeta {
                    name: "DeleteBlankPages",
                    multi_input: false,
                    output_type: "pdf",
                    schema: Value::Null,
                },
                OpMeta {
                    name: "CropPages",
                    multi_input: false,
                    output_type: "pdf",
                    schema: Value::Null,
                },
                OpMeta {
                    name: "ScalePages",
                    multi_input: false,
                    output_type: "pdf",
                    schema: Value::Null,
                },
                OpMeta {
                    name: "SinglePage",
                    multi_input: false,
                    output_type: "pdf",
                    schema: Value::Null,
                },
                OpMeta {
                    name: "NUp",
                    multi_input: false,
                    output_type: "pdf",
                    schema: Value::Null,
                },
                OpMeta {
                    name: "Booklet",
                    multi_input: false,
                    output_type: "pdf",
                    schema: Value::Null,
                },
                OpMeta {
                    name: "PageNumbers",
                    multi_input: false,
                    output_type: "pdf",
                    schema: Value::Null,
                },
                OpMeta {
                    name: "ImageToPdf",
                    multi_input: true,
                    output_type: "pdf",
                    schema: Value::Null,
                },
                OpMeta {
                    name: "SvgToPdf",
                    multi_input: true,
                    output_type: "pdf",
                    schema: Value::Null,
                },
                OpMeta {
                    name: "Watermark",
                    multi_input: false,
                    output_type: "pdf",
                    schema: Value::Null,
                },
                OpMeta {
                    name: "Overlay",
                    multi_input: true,
                    output_type: "pdf",
                    schema: Value::Null,
                },
                OpMeta {
                    name: "ImageEdit",
                    multi_input: false,
                    output_type: "pdf",
                    schema: Value::Null,
                },
                OpMeta {
                    name: "Color",
                    multi_input: false,
                    output_type: "pdf",
                    schema: Value::Null,
                },
                OpMeta {
                    name: "Metadata",
                    multi_input: false,
                    output_type: "pdf",
                    schema: Value::Null,
                },
                OpMeta {
                    name: "Outline",
                    multi_input: false,
                    output_type: "pdf",
                    schema: Value::Null,
                },
                OpMeta {
                    name: "Attachment",
                    multi_input: true,
                    output_type: "pdf",
                    schema: Value::Null,
                },
                OpMeta {
                    name: "Annotation",
                    multi_input: false,
                    output_type: "pdf",
                    schema: Value::Null,
                },
                OpMeta {
                    name: "FormFill",
                    multi_input: false,
                    output_type: "pdf",
                    schema: Value::Null,
                },
                OpMeta {
                    name: "FormUnlockReadonly",
                    multi_input: false,
                    output_type: "pdf",
                    schema: Value::Null,
                },
                OpMeta {
                    name: "FormRemove",
                    multi_input: false,
                    output_type: "pdf",
                    schema: Value::Null,
                },
                OpMeta {
                    name: "InteractiveRemove",
                    multi_input: false,
                    output_type: "pdf",
                    schema: Value::Null,
                },
                OpMeta {
                    name: "Compression",
                    multi_input: false,
                    output_type: "pdf",
                    schema: Value::Null,
                },
            ],
        },
        FamilySchema {
            name: "PdfInspect",
            ops: vec![
                OpMeta {
                    name: "Render",
                    multi_input: false,
                    output_type: "image",
                    schema: Value::Null,
                },
                OpMeta {
                    name: "ExtractText",
                    multi_input: false,
                    output_type: "text",
                    schema: Value::Null,
                },
                OpMeta {
                    name: "Metadata",
                    multi_input: false,
                    output_type: "text",
                    schema: Value::Null,
                },
                OpMeta {
                    name: "Outline",
                    multi_input: false,
                    output_type: "text",
                    schema: Value::Null,
                },
                OpMeta {
                    name: "Attachments",
                    multi_input: false,
                    output_type: "text",
                    schema: Value::Null,
                },
                OpMeta {
                    name: "AttachmentExtract",
                    multi_input: false,
                    output_type: "bytes",
                    schema: Value::Null,
                },
                OpMeta {
                    name: "Annotations",
                    multi_input: false,
                    output_type: "text",
                    schema: Value::Null,
                },
                OpMeta {
                    name: "Forms",
                    multi_input: false,
                    output_type: "text",
                    schema: Value::Null,
                },
                OpMeta {
                    name: "Images",
                    multi_input: false,
                    output_type: "text",
                    schema: Value::Null,
                },
                OpMeta {
                    name: "ImageExtract",
                    multi_input: false,
                    output_type: "image",
                    schema: Value::Null,
                },
            ],
        },
        FamilySchema {
            name: "PdfSecurity",
            ops: vec![
                OpMeta {
                    name: "Encrypt",
                    multi_input: false,
                    output_type: "pdf",
                    schema: Value::Null,
                },
                OpMeta {
                    name: "Decrypt",
                    multi_input: false,
                    output_type: "pdf",
                    schema: Value::Null,
                },
                OpMeta {
                    name: "PermissionsGet",
                    multi_input: false,
                    output_type: "text",
                    schema: Value::Null,
                },
                OpMeta {
                    name: "PermissionsSet",
                    multi_input: false,
                    output_type: "pdf",
                    schema: Value::Null,
                },
            ],
        },
        FamilySchema {
            name: "PdfCompare",
            ops: vec![
                OpMeta {
                    name: "Report",
                    multi_input: true,
                    output_type: "text",
                    schema: Value::Null,
                },
                OpMeta {
                    name: "VisualDiff",
                    multi_input: true,
                    output_type: "image",
                    schema: Value::Null,
                },
            ],
        },
        FamilySchema {
            name: "PdfSign",
            ops: vec![
                OpMeta {
                    name: "Add",
                    multi_input: false,
                    output_type: "pdf",
                    schema: Value::Null,
                },
                OpMeta {
                    name: "List",
                    multi_input: false,
                    output_type: "text",
                    schema: Value::Null,
                },
                OpMeta {
                    name: "Verify",
                    multi_input: false,
                    output_type: "text",
                    schema: Value::Null,
                },
                OpMeta {
                    name: "DeleteField",
                    multi_input: false,
                    output_type: "pdf",
                    schema: Value::Null,
                },
                OpMeta {
                    name: "Timestamp",
                    multi_input: false,
                    output_type: "pdf",
                    schema: Value::Null,
                },
            ],
        },
    ];
    for family in &mut families {
        for op in &mut family.ops {
            op.schema = op_schema(family.name, op.name);
        }
    }
    families
}
