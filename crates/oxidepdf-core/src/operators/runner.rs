use super::options::{PdfEditOptions, PdfInspectOptions, PdfSignOptions};
use crate::workflow::ResourceLimits;
use crate::{
    Artifact, OxideError, PdfCompareOptions, PdfSecurityOptions, add_pdf_signature,
    add_pdf_timestamp, booklet_pdf_pages_with_limits, compare_pdf_report, compare_pdf_visual_diff,
    decrypt_pdf, delete_pdf_signature_field, edit_pdf_attachment_artifacts,
    edit_pdf_images_artifacts, encrypt_pdf, enforce_input_bytes, extract_pdf_attachment,
    extract_pdf_image, extract_text_from_pdf, image_artifacts_to_pdf, inspect_pdf_permissions,
    load_pdf, nup_pdf_pages_with_limits, overlay_pdf_artifacts, pdf_bytes, render_pdf_page,
    set_pdf_permissions, svg_to_pdf, verify_pdf_signatures, watermark_pdf_artifacts,
};

include!("runner/edit.rs");
include!("runner/inspect.rs");
include!("runner/security_compare_sign.rs");
include!("runner/inputs.rs");
