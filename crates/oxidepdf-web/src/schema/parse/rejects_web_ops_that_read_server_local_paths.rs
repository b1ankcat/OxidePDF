
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_web_ops_that_read_server_local_paths() {
        let cases = [
            (
                "PdfSign",
                "Add",
                r#"{"field_name":"sig","certificate":"/etc/passwd","private_key":"/etc/shadow"}"#,
            ),
            ("PdfSign", "List", r#"{"trust_anchors":"/etc/passwd"}"#),
            ("PdfSign", "Verify", r#"{"trust_anchors":"/etc/passwd"}"#),
            ("PdfSign", "Timestamp", r#"{"token":"/etc/passwd"}"#),
            (
                "PdfEdit",
                "Watermark",
                r#"{"kind":"text","text":"x","font_path":"/etc/passwd"}"#,
            ),
            (
                "PdfEdit",
                "Overlay",
                r#"{"kind":"text","text":"x","font_path":"/etc/passwd"}"#,
            ),
        ];

        for (family, op, json) in cases {
            assert!(
                matches!(parse_op(family, op, json), Err(ParseOpError::UnknownOp)),
                "{family}/{op} should not be accepted by the web API"
            );
        }
    }

    #[test]
    fn accepts_text_watermark_with_standard_font_only() {
        assert!(
            parse_op(
                "PdfEdit",
                "Watermark",
                r#"{"kind":"text","text":"DRAFT","font":"Helvetica"}"#
            )
            .is_ok()
        );
    }

    #[test]
    fn text_watermark_without_font_uses_standard_web_font() {
        let op = parse_op("PdfEdit", "Watermark", r#"{"kind":"text","text":"DRAFT"}"#).unwrap();
        let OperatorSpec::PdfEdit(PdfEditOptions::Watermark(options)) = op else {
            panic!("expected watermark op");
        };
        assert_eq!(options.font.as_deref(), Some("Helvetica"));
    }

    #[test]
    fn text_overlay_without_font_uses_standard_web_font() {
        let op = parse_op("PdfEdit", "Overlay", r#"{"kind":"text","text":"DRAFT"}"#).unwrap();
        let OperatorSpec::PdfEdit(PdfEditOptions::Overlay(options)) = op else {
            panic!("expected overlay op");
        };
        assert_eq!(options.font.as_deref(), Some("Helvetica"));
    }
}
