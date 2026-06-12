#![forbid(unsafe_code)]
#![doc = "Core contracts and shared logic for OxidePDF."]

mod annotations;
mod attachments;
mod compare;
mod compression;
mod errors;
mod forms;
mod interactive;
mod metadata;
mod operators;
mod outlines;
mod overlay;
mod page_ops;
mod pdf_io;
mod security;
mod signatures;
mod workflow;

pub use annotations::{
    edit_pdf_annotations, inspect_pdf_annotations, AnnotationEditAction, AnnotationEditOptions,
    AnnotationInspectOptions,
};
pub use attachments::{
    edit_pdf_attachment_artifacts, extract_pdf_attachment, inspect_pdf_attachments,
    AttachmentEditAction, AttachmentEditOptions, AttachmentExtractOptions,
    AttachmentInspectOptions,
};
pub use errors::OxideError;
pub use forms::{
    fill_pdf_form, inspect_pdf_forms, remove_pdf_forms, unlock_pdf_form_readonly, FormFieldValue,
    FormFillOptions, FormInspectOptions,
};
pub use interactive::{remove_pdf_interactive_elements, InteractiveRemovalOptions};
pub use metadata::{
    edit_pdf_metadata, inspect_pdf_metadata, MetadataEditAction, MetadataEditOptions,
    MetadataEntry, MetadataInspectOptions,
};
pub use operators::{PdfEditOptions, PdfInspectOptions, PdfSignOptions};
pub use outlines::{
    edit_pdf_outline, inspect_pdf_outline, OutlineEditAction, OutlineEditOptions,
    OutlineInspectOptions, OutlineItem, OutlineTree,
};
pub use workflow::{
    execute_workflow, validate_workflow, Artifact, ArtifactRef, ArtifactStore, BytesArtifact,
    ExecutionPlan, ExecutionResult, ImageArtifact, InputSpec, OperatorRunner, OperatorSpec,
    OutputSpec, PdfArtifact, PdfOperatorRunner, ResourceLimits, SvgArtifact, TaskId, TaskSpec,
    TextArtifact, TextExtractionDiagnostic, TextExtractionDiagnosticCode, Workflow,
    WorkflowMetadata, WorkflowVersion, WORKFLOW_SCHEMA_VERSION,
};

pub(crate) use pdf_io::{
    add_resource_dict_entry, enforce_input_bytes, enforce_max_pages, enforce_max_pixels,
    enforce_output_bytes, ensure_pdf_magic, load_pdf, map_pdf_extract_error,
    merge_resource_dictionary, object_to_f32, page_size, pdf_bytes, rebuild_pages_tree,
    remap_imported_references, resource_limit, save_pdf,
};

pub use compare::PdfCompareOptions;
pub use compression::{
    compress_pdf, CompressionImageFormat, CompressionImageOptions, CompressionMode,
    CompressionOptions,
};
pub use overlay::{
    edit_pdf_colors, edit_pdf_images_artifacts, extract_pdf_image, extract_text_from_pdf,
    image_artifacts_to_pdf, inspect_pdf_images, overlay_pdf_artifacts, render_pdf_page, svg_to_pdf,
    watermark_pdf_artifacts, ColorEditAction, ColorEditOptions, ExtractTextOptions,
    ImageEditAction, ImageEditOptions, ImageExtractOptions, ImageInspectOptions, ImageToPdfOptions,
    OverlayKind, OverlayOptions, RenderOptions, SvgToPdfOptions, WatermarkKind, WatermarkOptions,
};
pub use page_ops::{
    add_pdf_page_numbers, add_pdf_page_numbers_with_limits, booklet_pdf_pages,
    booklet_pdf_pages_with_limits, crop_pdf_pages, crop_pdf_pages_with_limits,
    delete_blank_pdf_pages, delete_blank_pdf_pages_with_limits, delete_pdf_pages,
    delete_pdf_pages_with_limits, extract_pdf_pages, extract_pdf_pages_with_limits,
    merge_pdf_artifacts, merge_pdf_artifacts_with_limits, nup_pdf_pages, nup_pdf_pages_with_limits,
    pdf_to_single_page, pdf_to_single_page_with_limits, reorder_pdf, reorder_pdf_with_limits,
    rotate_pdf, rotate_pdf_with_limits, scale_pdf_pages, scale_pdf_pages_with_limits, split_pdf,
    split_pdf_with_limits, BookletOptions, CropPagesOptions, DeleteBlankPagesOptions, MergeOptions,
    NUpOptions, PageNumberPosition, PageNumbersOptions, PageSelectionOptions, ReorderOptions,
    RotateOptions, ScalePagesOptions, SinglePageOptions, SplitOptions,
};
pub use security::PdfSecurityOptions;
pub use signatures::{
    inspect_pdf_signature_markers_for_research, verify_pdf_signatures, ByteRangeResearch,
    ByteRangeVerification, ContentsVerification, SignatureCheckState, SignatureCheckStatus,
    SignatureDiagnostic, SignatureEntryReport, SignatureMode, SignatureOptions,
    SignatureResearchReport, SignatureVerdict, SignatureVerificationReport,
};

#[cfg(test)]
mod tests;
