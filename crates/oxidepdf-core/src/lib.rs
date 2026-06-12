#![forbid(unsafe_code)]
#![doc = "Core contracts and shared logic for OxidePDF."]

mod annotations;
mod compare;
mod errors;
mod forms;
mod metadata;
mod overlay;
mod page_ops;
mod pdf_io;
mod security;
mod signatures;
mod workflow;

pub use errors::OxideError;
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
    merge_resource_dictionary, object_to_f32, page_size, rebuild_pages_tree,
    remap_imported_references, resource_limit, save_pdf,
};

pub use compare::PdfCompareOptions;
pub use overlay::{
    extract_text_from_pdf, image_artifacts_to_pdf, render_pdf_page, svg_to_pdf,
    watermark_pdf_artifacts, ExtractTextOptions, ImageToPdfOptions, RenderOptions, SvgToPdfOptions,
    WatermarkKind, WatermarkOptions,
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

use serde::{Deserialize, Serialize};

/// PDF edit and creation operations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "PdfEditOptionsDef", into = "PdfEditOptionsDef")]
pub enum PdfEditOptions {
    /// Merge multiple PDFs.
    Merge(MergeOptions),
    /// Keep selected pages from a PDF.
    KeepPages(SplitOptions),
    /// Extract selected pages from a PDF.
    ExtractPages(PageSelectionOptions),
    /// Reorder pages in a PDF.
    ReorderPages(ReorderOptions),
    /// Rotate selected pages.
    RotatePages(RotateOptions),
    /// Delete selected pages.
    DeletePages(PageSelectionOptions),
    /// Delete pages with no content streams and no page resources.
    DeleteBlankPages(DeleteBlankPagesOptions),
    /// Crop selected pages.
    CropPages(CropPagesOptions),
    /// Scale selected pages.
    ScalePages(ScalePagesOptions),
    /// Combine all pages into one tall page.
    SinglePage(SinglePageOptions),
    /// Lay multiple source pages on each output page.
    NUp(NUpOptions),
    /// Arrange pages for booklet printing.
    Booklet(BookletOptions),
    /// Add page numbers to pages.
    PageNumbers(PageNumbersOptions),
    /// Convert images to PDF pages.
    ImageToPdf(ImageToPdfOptions),
    /// Convert SVG to PDF.
    SvgToPdf(SvgToPdfOptions),
    /// Add a watermark to a PDF.
    Watermark(WatermarkOptions),
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct PdfEditOptionsDef {
    merge: Option<MergeOptions>,
    keep_pages: Option<SplitOptions>,
    extract_pages: Option<PageSelectionOptions>,
    reorder_pages: Option<ReorderOptions>,
    rotate_pages: Option<RotateOptions>,
    delete_pages: Option<PageSelectionOptions>,
    delete_blank_pages: Option<DeleteBlankPagesOptions>,
    crop_pages: Option<CropPagesOptions>,
    scale_pages: Option<ScalePagesOptions>,
    single_page: Option<SinglePageOptions>,
    nup: Option<NUpOptions>,
    booklet: Option<BookletOptions>,
    page_numbers: Option<PageNumbersOptions>,
    image_to_pdf: Option<ImageToPdfOptions>,
    svg_to_pdf: Option<SvgToPdfOptions>,
    watermark: Option<WatermarkOptions>,
}

impl TryFrom<PdfEditOptionsDef> for PdfEditOptions {
    type Error = OxideError;

    fn try_from(value: PdfEditOptionsDef) -> Result<Self, Self::Error> {
        let operation_count = [
            value.merge.is_some(),
            value.keep_pages.is_some(),
            value.extract_pages.is_some(),
            value.reorder_pages.is_some(),
            value.rotate_pages.is_some(),
            value.delete_pages.is_some(),
            value.delete_blank_pages.is_some(),
            value.crop_pages.is_some(),
            value.scale_pages.is_some(),
            value.single_page.is_some(),
            value.nup.is_some(),
            value.booklet.is_some(),
            value.page_numbers.is_some(),
            value.image_to_pdf.is_some(),
            value.svg_to_pdf.is_some(),
            value.watermark.is_some(),
        ]
        .into_iter()
        .filter(|present| *present)
        .count();

        if operation_count != 1 {
            return Err(OxideError::InvalidWorkflow {
                reason: "pdf_edit must contain exactly one operation".to_owned(),
            });
        }

        if let Some(options) = value.merge {
            return Ok(Self::Merge(options));
        }
        if let Some(options) = value.keep_pages {
            return Ok(Self::KeepPages(options));
        }
        if let Some(options) = value.extract_pages {
            return Ok(Self::ExtractPages(options));
        }
        if let Some(options) = value.reorder_pages {
            return Ok(Self::ReorderPages(options));
        }
        if let Some(options) = value.rotate_pages {
            return Ok(Self::RotatePages(options));
        }
        if let Some(options) = value.delete_pages {
            return Ok(Self::DeletePages(options));
        }
        if let Some(options) = value.delete_blank_pages {
            return Ok(Self::DeleteBlankPages(options));
        }
        if let Some(options) = value.crop_pages {
            return Ok(Self::CropPages(options));
        }
        if let Some(options) = value.scale_pages {
            return Ok(Self::ScalePages(options));
        }
        if let Some(options) = value.single_page {
            return Ok(Self::SinglePage(options));
        }
        if let Some(options) = value.nup {
            return Ok(Self::NUp(options));
        }
        if let Some(options) = value.booklet {
            return Ok(Self::Booklet(options));
        }
        if let Some(options) = value.page_numbers {
            return Ok(Self::PageNumbers(options));
        }
        if let Some(options) = value.image_to_pdf {
            return Ok(Self::ImageToPdf(options));
        }
        if let Some(options) = value.svg_to_pdf {
            return Ok(Self::SvgToPdf(options));
        }
        if let Some(options) = value.watermark {
            return Ok(Self::Watermark(options));
        }

        unreachable!("operation count was already checked");
    }
}

impl From<PdfEditOptions> for PdfEditOptionsDef {
    fn from(value: PdfEditOptions) -> Self {
        match value {
            PdfEditOptions::Merge(options) => Self {
                merge: Some(options),
                ..Self::default()
            },
            PdfEditOptions::KeepPages(options) => Self {
                keep_pages: Some(options),
                ..Self::default()
            },
            PdfEditOptions::ExtractPages(options) => Self {
                extract_pages: Some(options),
                ..Self::default()
            },
            PdfEditOptions::ReorderPages(options) => Self {
                reorder_pages: Some(options),
                ..Self::default()
            },
            PdfEditOptions::RotatePages(options) => Self {
                rotate_pages: Some(options),
                ..Self::default()
            },
            PdfEditOptions::DeletePages(options) => Self {
                delete_pages: Some(options),
                ..Self::default()
            },
            PdfEditOptions::DeleteBlankPages(options) => Self {
                delete_blank_pages: Some(options),
                ..Self::default()
            },
            PdfEditOptions::CropPages(options) => Self {
                crop_pages: Some(options),
                ..Self::default()
            },
            PdfEditOptions::ScalePages(options) => Self {
                scale_pages: Some(options),
                ..Self::default()
            },
            PdfEditOptions::SinglePage(options) => Self {
                single_page: Some(options),
                ..Self::default()
            },
            PdfEditOptions::NUp(options) => Self {
                nup: Some(options),
                ..Self::default()
            },
            PdfEditOptions::Booklet(options) => Self {
                booklet: Some(options),
                ..Self::default()
            },
            PdfEditOptions::PageNumbers(options) => Self {
                page_numbers: Some(options),
                ..Self::default()
            },
            PdfEditOptions::ImageToPdf(options) => Self {
                image_to_pdf: Some(options),
                ..Self::default()
            },
            PdfEditOptions::SvgToPdf(options) => Self {
                svg_to_pdf: Some(options),
                ..Self::default()
            },
            PdfEditOptions::Watermark(options) => Self {
                watermark: Some(options),
                ..Self::default()
            },
        }
    }
}

/// PDF inspection operations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "PdfInspectOptionsDef", into = "PdfInspectOptionsDef")]
pub enum PdfInspectOptions {
    /// Render PDF pages to images.
    Render(RenderOptions),
    /// Extract text from a PDF.
    ExtractText(ExtractTextOptions),
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct PdfInspectOptionsDef {
    render: Option<RenderOptions>,
    extract_text: Option<ExtractTextOptions>,
}

impl TryFrom<PdfInspectOptionsDef> for PdfInspectOptions {
    type Error = OxideError;

    fn try_from(value: PdfInspectOptionsDef) -> Result<Self, Self::Error> {
        let operation_count = [value.render.is_some(), value.extract_text.is_some()]
            .into_iter()
            .filter(|present| *present)
            .count();

        if operation_count != 1 {
            return Err(OxideError::InvalidWorkflow {
                reason: "pdf_inspect must contain exactly one operation".to_owned(),
            });
        }

        if let Some(options) = value.render {
            return Ok(Self::Render(options));
        }
        if let Some(options) = value.extract_text {
            return Ok(Self::ExtractText(options));
        }

        unreachable!("operation count was already checked");
    }
}

impl From<PdfInspectOptions> for PdfInspectOptionsDef {
    fn from(value: PdfInspectOptions) -> Self {
        match value {
            PdfInspectOptions::Render(options) => Self {
                render: Some(options),
                ..Self::default()
            },
            PdfInspectOptions::ExtractText(options) => Self {
                extract_text: Some(options),
                ..Self::default()
            },
        }
    }
}

/// PDF signing and signature verification operations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "PdfSignOptionsDef", into = "PdfSignOptionsDef")]
pub enum PdfSignOptions {
    /// Verify PDF signatures and certificate material.
    Verify(SignatureOptions),
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct PdfSignOptionsDef {
    verify: Option<SignatureOptions>,
}

impl TryFrom<PdfSignOptionsDef> for PdfSignOptions {
    type Error = OxideError;

    fn try_from(value: PdfSignOptionsDef) -> Result<Self, Self::Error> {
        if let Some(options) = value.verify {
            return Ok(Self::Verify(options));
        }

        Err(OxideError::InvalidWorkflow {
            reason: "pdf_sign must contain exactly one operation".to_owned(),
        })
    }
}

impl From<PdfSignOptions> for PdfSignOptionsDef {
    fn from(value: PdfSignOptions) -> Self {
        match value {
            PdfSignOptions::Verify(options) => Self {
                verify: Some(options),
            },
        }
    }
}

pub(crate) fn run_pdf_edit(
    options: &PdfEditOptions,
    inputs: &[Artifact],
    limits: &ResourceLimits,
) -> Result<Artifact, OxideError> {
    match options {
        PdfEditOptions::Merge(_) => {
            merge_pdf_artifacts_with_limits(inputs, limits).map(Artifact::Pdf)
        }
        PdfEditOptions::KeepPages(options) => {
            let input = single_pdf_input(inputs)?;
            split_pdf_with_limits(input, &options.pages, limits).map(Artifact::Pdf)
        }
        PdfEditOptions::ExtractPages(options) => {
            let input = single_pdf_input(inputs)?;
            extract_pdf_pages_with_limits(input, &options.pages, limits).map(Artifact::Pdf)
        }
        PdfEditOptions::ReorderPages(options) => {
            let input = single_pdf_input(inputs)?;
            reorder_pdf_with_limits(input, &options.pages, limits).map(Artifact::Pdf)
        }
        PdfEditOptions::RotatePages(options) => {
            let input = single_pdf_input(inputs)?;
            rotate_pdf_with_limits(input, &options.pages, options.degrees, limits)
                .map(Artifact::Pdf)
        }
        PdfEditOptions::DeletePages(options) => {
            let input = single_pdf_input(inputs)?;
            delete_pdf_pages_with_limits(input, &options.pages, limits).map(Artifact::Pdf)
        }
        PdfEditOptions::DeleteBlankPages(options) => {
            let input = single_pdf_input(inputs)?;
            delete_blank_pdf_pages_with_limits(input, options, limits).map(Artifact::Pdf)
        }
        PdfEditOptions::CropPages(options) => {
            let input = single_pdf_input(inputs)?;
            crop_pdf_pages_with_limits(input, options, limits).map(Artifact::Pdf)
        }
        PdfEditOptions::ScalePages(options) => {
            let input = single_pdf_input(inputs)?;
            scale_pdf_pages_with_limits(input, options, limits).map(Artifact::Pdf)
        }
        PdfEditOptions::SinglePage(options) => {
            let input = single_pdf_input(inputs)?;
            pdf_to_single_page_with_limits(input, options, limits).map(Artifact::Pdf)
        }
        PdfEditOptions::NUp(options) => {
            let input = single_pdf_input(inputs)?;
            nup_pdf_pages_with_limits(input, options, limits).map(Artifact::Pdf)
        }
        PdfEditOptions::Booklet(options) => {
            let input = single_pdf_input(inputs)?;
            booklet_pdf_pages_with_limits(input, options, limits).map(Artifact::Pdf)
        }
        PdfEditOptions::PageNumbers(options) => {
            let input = single_pdf_input(inputs)?;
            add_pdf_page_numbers_with_limits(input, options, limits).map(Artifact::Pdf)
        }
        PdfEditOptions::ImageToPdf(options) => {
            image_artifacts_to_pdf(inputs, options, limits).map(Artifact::Pdf)
        }
        PdfEditOptions::SvgToPdf(options) => {
            let input = single_svg_input(inputs)?;
            svg_to_pdf(input, options, limits).map(Artifact::Pdf)
        }
        PdfEditOptions::Watermark(options) => {
            watermark_pdf_artifacts(inputs, options, limits).map(Artifact::Pdf)
        }
    }
}

pub(crate) fn run_pdf_inspect(
    options: &PdfInspectOptions,
    inputs: &[Artifact],
    limits: &ResourceLimits,
) -> Result<Artifact, OxideError> {
    match options {
        PdfInspectOptions::Render(options) => {
            let input = single_pdf_input(inputs)?;
            render_pdf_page(input, options, limits).map(Artifact::Image)
        }
        PdfInspectOptions::ExtractText(options) => {
            let input = single_pdf_input(inputs)?;
            extract_text_from_pdf(input, options, limits).map(Artifact::Text)
        }
    }
}

pub(crate) fn run_pdf_security(options: &PdfSecurityOptions) -> Result<Artifact, OxideError> {
    Err(OxideError::UnsupportedPdfFeature {
        feature: format!("pdf_security operation '{}'", options.operation),
    })
}

pub(crate) fn run_pdf_compare(options: &PdfCompareOptions) -> Result<Artifact, OxideError> {
    Err(OxideError::UnsupportedPdfFeature {
        feature: format!("pdf_compare mode '{}'", options.mode),
    })
}

pub(crate) fn run_pdf_sign(
    options: &PdfSignOptions,
    inputs: &[Artifact],
    limits: &ResourceLimits,
) -> Result<Artifact, OxideError> {
    match options {
        PdfSignOptions::Verify(options) => {
            let input = single_pdf_input(inputs)?;
            verify_pdf_signatures(input, options, limits).map(Artifact::Text)
        }
    }
}

fn single_pdf_input(inputs: &[Artifact]) -> Result<&[u8], OxideError> {
    if inputs.len() != 1 {
        return Err(OxideError::InvalidInput {
            reason: "operator requires exactly one PDF input".to_owned(),
        });
    }

    pdf_bytes(&inputs[0])
}

fn single_svg_input(inputs: &[Artifact]) -> Result<&[u8], OxideError> {
    if inputs.len() != 1 {
        return Err(OxideError::InvalidInput {
            reason: "svg2pdf requires exactly one SVG input".to_owned(),
        });
    }

    match &inputs[0] {
        Artifact::Svg(svg) => Ok(&svg.bytes),
        Artifact::Bytes(bytes) => Ok(&bytes.bytes),
        _ => Err(OxideError::InvalidInput {
            reason: "expected SVG input artifact".to_owned(),
        }),
    }
}

pub(crate) fn pdf_bytes(artifact: &Artifact) -> Result<&[u8], OxideError> {
    match artifact {
        Artifact::Pdf(pdf) => Ok(&pdf.bytes),
        Artifact::Bytes(bytes) => Ok(&bytes.bytes),
        _ => Err(OxideError::InvalidInput {
            reason: "expected PDF input artifact".to_owned(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::{dictionary, Dictionary, Object, Stream};
    use pdf_writer::Finish;
    use std::collections::BTreeSet;
    use std::path::PathBuf;

    const A4_WIDTH: f32 = 595.0;
    const A4_HEIGHT: f32 = 842.0;

    #[test]
    fn workflow_schema_version_starts_at_one() {
        assert_eq!(WORKFLOW_SCHEMA_VERSION, 1);
    }

    #[test]
    fn parses_example_json_workflow() {
        let workflow: Workflow = serde_json::from_str(
            r#"
            {
              "version": 1,
              "inputs": [
                { "id": "source", "path": "./input.pdf" }
              ],
              "tasks": [
                {
                  "id": "rotate_pages",
                  "op": {
                    "pdf_edit": {
                      "rotate_pages": {
                        "pages": "1,3-5",
                        "degrees": 90
                      }
                    }
                  },
                  "inputs": ["source"]
                }
              ],
              "outputs": [
                { "id": "final", "from": "rotate_pages", "path": "./output.pdf" }
              ],
              "limits": {
                "max_input_bytes": 524288000,
                "max_pages": 5000,
                "max_pixels": 160000000
              },
              "metadata": {
                "title": "Example workflow"
              }
            }
            "#,
        )
        .unwrap();

        assert_eq!(workflow.version, WorkflowVersion::V1);
        assert_eq!(workflow.inputs[0].id.as_str(), "source");
        assert_eq!(workflow.tasks[0].id.as_str(), "rotate_pages");
        assert!(matches!(
            workflow.tasks[0].op,
            OperatorSpec::PdfEdit(PdfEditOptions::RotatePages(RotateOptions {
                degrees: 90,
                ..
            }))
        ));
        assert_eq!(workflow.outputs[0].from.as_str(), "rotate_pages");
    }

    #[test]
    fn parses_example_yaml_workflow() {
        let workflow: Workflow = serde_yaml::from_str(
            r#"
            version: 1
            inputs:
              - id: source
                path: ./input.pdf
            tasks:
              - id: rotate_pages
                op:
                  pdf_edit:
                    rotate_pages:
                      pages: "1,3-5"
                      degrees: 90
                inputs: [source]
              - id: stamp
                op:
                  pdf_edit:
                    watermark:
                      kind: text
                      text: Confidential
                      opacity: 0.18
                      position: center
                inputs: [rotate_pages]
            outputs:
              - id: final
                from: stamp
                path: ./output.pdf
            limits:
              max_input_bytes: 524288000
              max_pages: 5000
              max_pixels: 160000000
            metadata:
              title: Example workflow
            "#,
        )
        .unwrap();

        assert_eq!(workflow.version, WorkflowVersion::V1);
        assert_eq!(workflow.tasks.len(), 2);
        assert!(matches!(
            workflow.tasks[1].op,
            OperatorSpec::PdfEdit(PdfEditOptions::Watermark(WatermarkOptions {
                kind: WatermarkKind::Text,
                ..
            }))
        ));
    }

    #[test]
    fn parses_signature_workflow_operator_schema() {
        let workflow: Workflow = serde_yaml::from_str(
            r#"
            version: 1
            inputs:
              - id: source
                path: ./signed.pdf
            tasks:
              - id: verify
                op:
                  pdf_sign:
                    verify:
                      mode: verify
                      trust_anchors: ./anchors.pem
                inputs: [source]
            outputs:
              - id: final
                from: verify
                path: ./report.json
            "#,
        )
        .unwrap();

        assert!(matches!(
            workflow.tasks[0].op,
            OperatorSpec::PdfSign(PdfSignOptions::Verify(SignatureOptions {
                mode: SignatureMode::Verify,
                trust_anchors: Some(_),
            }))
        ));
        match &workflow.tasks[0].op {
            OperatorSpec::PdfSign(PdfSignOptions::Verify(options)) => {
                assert_eq!(
                    options.trust_anchors.as_deref(),
                    Some(std::path::Path::new("./anchors.pem"))
                );
            }
            _ => unreachable!("asserted signature operator above"),
        }
    }

    #[test]
    fn missing_required_workflow_field_fails() {
        let err = serde_json::from_str::<Workflow>(
            r#"
            {
              "version": 1,
              "inputs": [],
              "tasks": [],
              "limits": {},
              "metadata": {}
            }
            "#,
        )
        .unwrap_err();

        assert!(err.to_string().contains("outputs"));
    }

    #[test]
    fn operator_spec_rejects_multiple_operator_keys() {
        let err = serde_json::from_str::<OperatorSpec>(
            r#"
            {
              "pdf_edit": { "rotate_pages": { "pages": "1", "degrees": 90 } },
              "pdf_inspect": { "render": { "page": 1 } }
            }
            "#,
        )
        .unwrap_err();

        assert!(err
            .to_string()
            .contains("operator spec must contain exactly one operator"));
    }

    #[test]
    fn operator_spec_rejects_removed_legacy_operator_keys() {
        let err = serde_json::from_str::<OperatorSpec>(
            r#"
            {
              "rotate": { "pages": "1", "degrees": 90 }
            }
            "#,
        )
        .unwrap_err();

        assert!(err.to_string().contains("unknown field"));
    }

    #[test]
    fn error_codes_are_stable_machine_readable_values() {
        assert_eq!(
            OxideError::UnsupportedPdfFeature {
                feature: "object stream".to_owned()
            }
            .code(),
            "unsupported_pdf_feature"
        );
        assert_eq!(OxideError::EncryptedPdf.code(), "encrypted_pdf");
        assert_eq!(OxideError::IncorrectPassword.code(), "incorrect_password");
        assert_eq!(OxideError::FontResolution.code(), "font_resolution");
        assert_eq!(
            OxideError::ResourceLimitExceeded {
                limit: "max_pages".to_owned()
            }
            .code(),
            "resource_limit_exceeded"
        );
    }

    #[test]
    fn linear_workflow_executes_tasks_in_dependency_order() {
        let workflow = workflow_from_json(
            r#"
            {
              "version": 1,
              "inputs": [{ "id": "source", "path": "input.pdf" }],
              "tasks": [
                {
                  "id": "rotate",
                  "op": { "pdf_edit": { "rotate_pages": { "pages": "1", "degrees": 90 } } },
                  "inputs": ["source"]
                },
                {
                  "id": "render",
                  "op": { "pdf_inspect": { "render": { "page": 1, "format": "png", "scale": 1.0 } } },
                  "inputs": ["rotate"]
                }
              ],
              "outputs": [{ "id": "final", "from": "render", "path": "out.png" }]
            }
            "#,
        );
        let mut store = ArtifactStore::new();
        store.insert(artifact_ref("source"), Artifact::bytes(b"input"));
        let mut runner = RecordingRunner::default();

        let result = execute_workflow(&workflow, store, &mut runner).unwrap();

        assert_eq!(runner.executed, ["rotate", "render"]);
        assert_eq!(
            result.store.get(&artifact_ref("render")),
            Some(&Artifact::bytes(b"render"))
        );
        assert_eq!(result.plan.task_order[0].as_str(), "rotate");
        assert_eq!(result.plan.task_order[1].as_str(), "render");
    }

    #[test]
    fn dag_workflow_topologically_sorts_before_execution() {
        let workflow = workflow_from_json(
            r#"
            {
              "version": 1,
              "inputs": [{ "id": "source", "path": "input.pdf" }],
              "tasks": [
                {
                  "id": "left",
                  "op": { "pdf_edit": { "rotate_pages": { "pages": "1", "degrees": 90 } } },
                  "inputs": ["source"]
                },
                {
                  "id": "right",
                  "op": { "pdf_edit": { "rotate_pages": { "pages": "1", "degrees": 180 } } },
                  "inputs": ["source"]
                },
                {
                  "id": "join",
                  "op": { "pdf_edit": { "merge": {} } },
                  "inputs": ["left", "right"]
                }
              ],
              "outputs": [{ "id": "final", "from": "join", "path": "out.pdf" }]
            }
            "#,
        );

        let plan = validate_workflow(&workflow).unwrap();
        let left = plan
            .task_order
            .iter()
            .position(|id| id.as_str() == "left")
            .unwrap();
        let right = plan
            .task_order
            .iter()
            .position(|id| id.as_str() == "right")
            .unwrap();
        let join = plan
            .task_order
            .iter()
            .position(|id| id.as_str() == "join")
            .unwrap();

        assert!(left < join);
        assert!(right < join);
    }

    #[test]
    fn cyclic_workflow_fails_validation() {
        let workflow = workflow_from_json(
            r#"
            {
              "version": 1,
              "inputs": [],
              "tasks": [
                {
                  "id": "a",
                  "op": { "pdf_edit": { "merge": {} } },
                  "inputs": ["b"]
                },
                {
                  "id": "b",
                  "op": { "pdf_edit": { "merge": {} } },
                  "inputs": ["a"]
                }
              ],
              "outputs": [{ "id": "final", "from": "b", "path": "out.pdf" }]
            }
            "#,
        );

        let err = validate_workflow(&workflow).unwrap_err();

        assert!(matches!(err, OxideError::InvalidWorkflow { .. }));
        assert!(err.to_string().contains("cycle"));
    }

    #[test]
    fn missing_artifact_reference_fails_validation() {
        let workflow = workflow_from_json(
            r#"
            {
              "version": 1,
              "inputs": [{ "id": "source", "path": "input.pdf" }],
              "tasks": [
                {
                  "id": "rotate",
                  "op": { "pdf_edit": { "rotate_pages": { "pages": "1", "degrees": 90 } } },
                  "inputs": ["missing"]
                }
              ],
              "outputs": [{ "id": "final", "from": "rotate", "path": "out.pdf" }]
            }
            "#,
        );

        let err = validate_workflow(&workflow).unwrap_err();

        assert!(matches!(err, OxideError::InvalidWorkflow { .. }));
        assert!(err.to_string().contains("missing"));
    }

    #[test]
    fn duplicate_artifact_identifiers_fail_validation() {
        let workflow = workflow_from_json(
            r#"
            {
              "version": 1,
              "inputs": [{ "id": "source", "path": "input.pdf" }],
              "tasks": [
                {
                  "id": "source",
                  "op": { "pdf_edit": { "rotate_pages": { "pages": "1", "degrees": 90 } } },
                  "inputs": ["source"]
                }
              ],
              "outputs": [{ "id": "final", "from": "source", "path": "out.pdf" }]
            }
            "#,
        );

        let err = validate_workflow(&workflow).unwrap_err();

        assert!(matches!(err, OxideError::InvalidWorkflow { .. }));
        assert!(err.to_string().contains("duplicate"));
    }

    #[test]
    fn task_failure_stops_downstream_execution() {
        let workflow = workflow_from_json(
            r#"
            {
              "version": 1,
              "inputs": [{ "id": "source", "path": "input.pdf" }],
              "tasks": [
                {
                  "id": "fail",
                  "op": { "pdf_edit": { "rotate_pages": { "pages": "1", "degrees": 90 } } },
                  "inputs": ["source"]
                },
                {
                  "id": "after",
                  "op": { "pdf_inspect": { "render": { "page": 1, "format": "png", "scale": 1.0 } } },
                  "inputs": ["fail"]
                }
              ],
              "outputs": [{ "id": "final", "from": "after", "path": "out.png" }]
            }
            "#,
        );
        let mut store = ArtifactStore::new();
        store.insert(artifact_ref("source"), Artifact::bytes(b"input"));
        let expected = OxideError::InvalidInput {
            reason: "runner failed".to_owned(),
        };
        let mut runner = RecordingRunner {
            fail_on: Some("fail"),
            error: Some(expected.clone()),
            ..RecordingRunner::default()
        };

        let err = execute_workflow(&workflow, store, &mut runner).unwrap_err();

        assert_eq!(err, expected);
        assert_eq!(runner.executed, ["fail"]);
    }

    #[test]
    fn merge_pdf_artifacts_combines_pages() {
        let pdf = include_bytes!("../../../tests/test.pdf");

        let merged = merge_pdf_artifacts(&[Artifact::pdf(pdf), Artifact::pdf(pdf)]).unwrap();
        let document = lopdf::Document::load_mem(&merged.bytes).unwrap();

        assert_eq!(document.get_pages().len(), 6);
    }

    #[test]
    fn merge_pdf_artifacts_enforces_input_and_page_limits() {
        let pdf = include_bytes!("../../../tests/test.pdf");

        let input_err = merge_pdf_artifacts_with_limits(
            &[Artifact::pdf(pdf), Artifact::pdf(pdf)],
            &ResourceLimits {
                max_input_bytes: Some(1),
                ..ResourceLimits::default()
            },
        )
        .unwrap_err();
        assert_eq!(
            input_err,
            OxideError::ResourceLimitExceeded {
                limit: "max_input_bytes".to_owned()
            }
        );

        let page_err = merge_pdf_artifacts_with_limits(
            &[Artifact::pdf(pdf), Artifact::pdf(pdf)],
            &ResourceLimits {
                max_pages: Some(5),
                ..ResourceLimits::default()
            },
        )
        .unwrap_err();
        assert_eq!(
            page_err,
            OxideError::ResourceLimitExceeded {
                limit: "max_pages".to_owned()
            }
        );
    }

    #[test]
    fn split_pdf_keeps_only_selected_pages() {
        let pdf = include_bytes!("../../../tests/test.pdf");

        let split = split_pdf(pdf, "2-3").unwrap();
        let document = lopdf::Document::load_mem(&split.bytes).unwrap();

        assert_eq!(document.get_pages().len(), 2);
        assert_page_numbers(&document, &[1, 2]);
    }

    #[test]
    fn split_pdf_enforces_resource_limits() {
        let pdf = include_bytes!("../../../tests/test.pdf");

        let err = split_pdf_with_limits(
            pdf,
            "1",
            &ResourceLimits {
                max_pages: Some(2),
                ..ResourceLimits::default()
            },
        )
        .unwrap_err();

        assert_eq!(
            err,
            OxideError::ResourceLimitExceeded {
                limit: "max_pages".to_owned()
            }
        );
    }

    #[test]
    fn reorder_pdf_rearranges_pages() {
        let pdf = include_bytes!("../../../tests/test.pdf");

        let reordered = reorder_pdf(pdf, "3,1,2").unwrap();
        let document = lopdf::Document::load_mem(&reordered.bytes).unwrap();

        assert_eq!(document.get_pages().len(), 3);
        assert_page_numbers(&document, &[1, 2, 3]);
    }

    #[test]
    fn rotate_pdf_updates_page_rotation() {
        let pdf = include_bytes!("../../../tests/test.pdf");

        let rotated = rotate_pdf(pdf, "1-2", 90).unwrap();
        let document = lopdf::Document::load_mem(&rotated.bytes).unwrap();

        assert_eq!(page_rotation(&document, 1), 90);
        assert_eq!(page_rotation(&document, 2), 90);
        assert_eq!(page_rotation(&document, 3), 0);
    }

    #[test]
    fn rotate_pdf_rejects_non_pdf_magic_bytes() {
        let err =
            rotate_pdf_with_limits(b"not a pdf", "1", 90, &ResourceLimits::default()).unwrap_err();

        assert!(matches!(err, OxideError::InvalidInput { .. }));
        assert!(err.to_string().contains("expected PDF"));
    }

    #[test]
    fn delete_pdf_pages_removes_selected_pages() {
        let pdf = include_bytes!("../../../tests/test.pdf");

        let deleted = delete_pdf_pages(pdf, "2").unwrap();
        let document = lopdf::Document::load_mem(&deleted.bytes).unwrap();

        assert_eq!(document.get_pages().len(), 2);
    }

    #[test]
    fn delete_pdf_pages_rejects_deleting_every_page() {
        let pdf = include_bytes!("../../../tests/test.pdf");

        let err = delete_pdf_pages(pdf, "1-3").unwrap_err();

        assert!(matches!(err, OxideError::InvalidInput { .. }));
        assert!(err.to_string().contains("leave at least one page"));
    }

    #[test]
    fn extract_pdf_pages_keeps_selected_order() {
        let pdf = include_bytes!("../../../tests/test.pdf");

        let extracted = extract_pdf_pages(pdf, "3,1").unwrap();
        let document = lopdf::Document::load_mem(&extracted.bytes).unwrap();

        assert_eq!(document.get_pages().len(), 2);
        assert_page_numbers(&document, &[1, 2]);
    }

    #[test]
    fn delete_blank_pdf_pages_uses_object_level_blank_detection() {
        let pdf = pdf_with_blank_and_marked_page();

        let deleted = delete_blank_pdf_pages(&pdf, &DeleteBlankPagesOptions::default()).unwrap();
        let document = lopdf::Document::load_mem(&deleted.bytes).unwrap();

        assert_eq!(document.get_pages().len(), 1);
    }

    #[test]
    fn delete_blank_pdf_pages_rejects_unresolved_resource_reference() {
        let pdf = pdf_with_blank_page_and_missing_resources();

        let err = delete_blank_pdf_pages(&pdf, &DeleteBlankPagesOptions::default()).unwrap_err();

        assert!(matches!(err, OxideError::ParsePdf));
    }

    #[test]
    fn crop_pdf_pages_sets_crop_box_on_selected_pages() {
        let pdf = include_bytes!("../../../tests/test.pdf");

        let cropped = crop_pdf_pages(
            pdf,
            &CropPagesOptions {
                pages: Some("1".to_owned()),
                left: 10.0,
                bottom: 20.0,
                right: 300.0,
                top: 400.0,
            },
        )
        .unwrap();
        let document = lopdf::Document::load_mem(&cropped.bytes).unwrap();

        assert_eq!(
            page_box(&document, 1, b"CropBox"),
            [10.0, 20.0, 300.0, 400.0]
        );
        assert!(page_optional_box(&document, 2, b"CropBox").is_none());
    }

    #[test]
    fn scale_pdf_pages_scales_page_box_and_content_stream() {
        let pdf = include_bytes!("../../../tests/test.pdf");

        let scaled = scale_pdf_pages(
            pdf,
            &ScalePagesOptions {
                pages: Some("1".to_owned()),
                factor: 0.5,
            },
        )
        .unwrap();
        let document = lopdf::Document::load_mem(&scaled.bytes).unwrap();
        let media_box = page_box(&document, 1, b"MediaBox");

        assert_eq!(media_box[2], 306.0);
        assert!(page_content_contains(&document, 1, "cm"));
        assert_eq!(page_box(&document, 2, b"MediaBox")[2], 612.0);
    }

    #[test]
    fn pdf_to_single_page_combines_pages_into_one_tall_page() {
        let pdf = include_bytes!("../../../tests/test.pdf");

        let single = pdf_to_single_page(pdf, &SinglePageOptions::default()).unwrap();
        let document = lopdf::Document::load_mem(&single.bytes).unwrap();
        let media_box = page_box(&document, 1, b"MediaBox");

        assert_eq!(document.get_pages().len(), 1);
        assert_eq!(media_box[2], 612.0);
        assert_eq!(media_box[3], 2376.0);
    }

    #[test]
    fn nup_pdf_pages_places_source_pages_as_xobjects() {
        let pdf = include_bytes!("../../../tests/test.pdf");

        let nup = nup_pdf_pages(
            pdf,
            &NUpOptions {
                columns: 2,
                rows: 2,
            },
        )
        .unwrap();
        let document = lopdf::Document::load_mem(&nup.bytes).unwrap();

        assert_eq!(document.get_pages().len(), 1);
        assert_eq!(page_xobject_count(&document, 1), 3);
        assert_eq!(
            page_box(&document, 1, b"MediaBox"),
            [0.0, 0.0, 612.0, 792.0]
        );
        assert!(page_content_contains_operator(&document, 1, "Do"));
    }

    #[test]
    fn nup_pdf_pages_rejects_zero_columns() {
        let pdf = include_bytes!("../../../tests/test.pdf");

        let err = nup_pdf_pages(
            pdf,
            &NUpOptions {
                columns: 0,
                rows: 2,
            },
        )
        .unwrap_err();

        assert!(matches!(err, OxideError::InvalidInput { .. }));
        assert!(err.to_string().contains("greater than zero"));
    }

    #[test]
    fn booklet_pdf_pages_outputs_two_up_imposed_pages() {
        let pdf = include_bytes!("../../../tests/test.pdf");

        let booklet = booklet_pdf_pages(pdf, &BookletOptions::default()).unwrap();
        let document = lopdf::Document::load_mem(&booklet.bytes).unwrap();

        assert_eq!(document.get_pages().len(), 2);
        assert_eq!(page_xobject_count(&document, 1), 1);
        assert_eq!(page_xobject_count(&document, 2), 2);
        assert!(page_content_contains_operator(&document, 1, "Do"));
    }

    #[test]
    fn add_pdf_page_numbers_writes_selected_page_content() {
        let pdf = include_bytes!("../../../tests/test.pdf");

        let numbered = add_pdf_page_numbers(
            pdf,
            &PageNumbersOptions {
                pages: Some("2-3".to_owned()),
                start: 7,
                prefix: "p".to_owned(),
                suffix: String::new(),
                font_size: 10.0,
                position: PageNumberPosition::BottomRight,
            },
        )
        .unwrap();
        let document = lopdf::Document::load_mem(&numbered.bytes).unwrap();

        assert!(!page_content_text_contains(&document, 1, "p7"));
        assert!(page_content_text_contains(&document, 2, "p7"));
        assert!(page_content_text_contains(&document, 3, "p8"));
    }

    #[test]
    fn add_pdf_page_numbers_rejects_non_ascii_text() {
        let pdf = include_bytes!("../../../tests/test.pdf");

        let err = add_pdf_page_numbers(
            pdf,
            &PageNumbersOptions {
                prefix: "第".to_owned(),
                ..PageNumbersOptions::default()
            },
        )
        .unwrap_err();

        assert!(matches!(err, OxideError::InvalidInput { .. }));
        assert!(err.to_string().contains("ASCII"));
    }

    #[test]
    fn pdf_operator_runner_handles_page_editing_tasks() {
        let pdf = include_bytes!("../../../tests/test.pdf");
        let mut runner = PdfOperatorRunner::default();

        let merged = runner
            .run(
                &TaskSpec {
                    id: TaskId::new("merge"),
                    op: OperatorSpec::PdfEdit(PdfEditOptions::Merge(MergeOptions {})),
                    inputs: vec![artifact_ref("a"), artifact_ref("b")],
                },
                &[Artifact::pdf(pdf), Artifact::pdf(pdf)],
            )
            .unwrap();

        assert!(matches!(merged, Artifact::Pdf(_)));
    }

    #[test]
    fn pdf_operator_runner_enforces_output_size_limit() {
        let pdf = include_bytes!("../../../tests/test.pdf");
        let mut runner = PdfOperatorRunner::with_limits(ResourceLimits {
            max_output_bytes: Some(1),
            ..ResourceLimits::default()
        });

        let err = runner
            .run(
                &TaskSpec {
                    id: TaskId::new("split"),
                    op: OperatorSpec::PdfEdit(PdfEditOptions::KeepPages(SplitOptions {
                        pages: "1".to_owned(),
                    })),
                    inputs: vec![artifact_ref("source")],
                },
                &[Artifact::pdf(pdf)],
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
    fn pdf_operator_runner_emits_signature_verification_report() {
        let pdf = pdf_with_signature_dictionary(vec![0, 64, 192, 64], vec![0x30, 0x82]);
        let trust_anchors = write_test_trust_anchors("signature_report");
        let mut runner = PdfOperatorRunner::default();

        let artifact = runner
            .run(
                &TaskSpec {
                    id: TaskId::new("verify"),
                    op: OperatorSpec::PdfSign(PdfSignOptions::Verify(SignatureOptions {
                        mode: SignatureMode::Verify,
                        trust_anchors: Some(trust_anchors),
                    })),
                    inputs: vec![artifact_ref("source")],
                },
                &[Artifact::pdf(&pdf)],
            )
            .unwrap();

        let Artifact::Text(report_text) = artifact else {
            panic!("signature verification should emit a text JSON report");
        };
        let report: SignatureVerificationReport = serde_json::from_str(&report_text.text).unwrap();
        assert_eq!(report.trust_anchor_count, 1);
        assert_eq!(report.verdict, SignatureVerdict::Unsupported);
        assert_eq!(report.signatures.len(), 1);
        assert_eq!(report.signatures[0].field_name.as_deref(), Some("Approval"));
        assert_eq!(
            report.signatures[0].subfilter.as_deref(),
            Some("adbe.pkcs7.detached")
        );
        assert_eq!(
            report.signatures[0].byte_range.values,
            Some([0, 64, 192, 64])
        );
        assert!(report.signatures[0].byte_range.in_bounds);
        assert!(report.signatures[0].byte_range.ordered_non_overlapping);
        assert_eq!(report.signatures[0].byte_range.gap_len, Some(128));
        assert_eq!(
            report.signatures[0].cms_status.status,
            SignatureCheckState::Unsupported
        );
        assert_eq!(
            report.signatures[0].revocation_status.status,
            SignatureCheckState::Indeterminate
        );
    }

    #[test]
    fn verify_pdf_signatures_requires_explicit_trust_anchors() {
        let pdf = include_bytes!("../../../tests/fixtures/signature-placeholder.pdf");

        let err = verify_pdf_signatures(
            pdf,
            &SignatureOptions::default(),
            &ResourceLimits::default(),
        )
        .unwrap_err();

        assert_eq!(
            err,
            OxideError::InvalidInput {
                reason: "signature verification requires explicit trust anchors".to_owned()
            }
        );
    }

    #[test]
    fn verify_pdf_signatures_rejects_empty_trust_anchor_file() {
        let pdf = include_bytes!("../../../tests/fixtures/signature-placeholder.pdf");
        let trust_anchors = write_empty_trust_anchors("empty_signature_anchors");

        let err = verify_pdf_signatures(
            pdf,
            &SignatureOptions {
                mode: SignatureMode::Verify,
                trust_anchors: Some(trust_anchors),
            },
            &ResourceLimits::default(),
        )
        .unwrap_err();

        assert_eq!(
            err,
            OxideError::InvalidInput {
                reason: "trust anchors file contains no valid PEM certificates".to_owned()
            }
        );
    }

    #[test]
    fn verify_pdf_signatures_rejects_invalid_trust_anchor_certificate() {
        let pdf = include_bytes!("../../../tests/fixtures/signature-placeholder.pdf");
        let trust_anchors = write_invalid_trust_anchors("invalid_signature_anchors");

        let err = verify_pdf_signatures(
            pdf,
            &SignatureOptions {
                mode: SignatureMode::Verify,
                trust_anchors: Some(trust_anchors),
            },
            &ResourceLimits::default(),
        )
        .unwrap_err();

        assert_eq!(
            err,
            OxideError::InvalidInput {
                reason: "trust anchors file contains no valid PEM certificates".to_owned()
            }
        );
    }

    #[test]
    fn verify_pdf_signatures_reports_unsigned_pdf_as_indeterminate() {
        let pdf = include_bytes!("../../../tests/test.pdf");
        let trust_anchors = write_test_trust_anchors("unsigned_pdf_report");

        let report = verify_pdf_signatures(
            pdf,
            &SignatureOptions {
                mode: SignatureMode::Verify,
                trust_anchors: Some(trust_anchors),
            },
            &ResourceLimits::default(),
        )
        .unwrap();
        let report: SignatureVerificationReport = serde_json::from_str(&report.text).unwrap();

        assert_eq!(report.verdict, SignatureVerdict::Indeterminate);
        assert_eq!(report.trust_anchor_count, 1);
        assert!(report.signatures.is_empty());
        assert_eq!(report.diagnostics.len(), 1);
        assert_eq!(report.diagnostics[0].code, "no_signatures");
    }

    #[test]
    fn verify_pdf_signatures_reports_malformed_byte_range_as_invalid() {
        let pdf = pdf_with_signature_dictionary(vec![0, 64, 32, 64], vec![0x30, 0x82]);
        let trust_anchors = write_test_trust_anchors("malformed_byte_range_report");

        let report = verify_pdf_signatures(
            &pdf,
            &SignatureOptions {
                mode: SignatureMode::Verify,
                trust_anchors: Some(trust_anchors),
            },
            &ResourceLimits::default(),
        )
        .unwrap();
        let report: SignatureVerificationReport = serde_json::from_str(&report.text).unwrap();

        assert_eq!(report.verdict, SignatureVerdict::Invalid);
        assert_eq!(report.signatures.len(), 1);
        assert!(report.signatures[0]
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "byte_range_not_ordered"));
    }

    #[test]
    fn signature_research_scanner_finds_signature_markers() {
        let pdf = include_bytes!("../../../tests/fixtures/signature-placeholder.pdf");

        let report = inspect_pdf_signature_markers_for_research(pdf).unwrap();

        assert_eq!(report.signature_dictionary_count, 1);
        assert_eq!(report.subfilters, vec!["adbe.pkcs7.detached"]);
        assert_eq!(report.byte_ranges.len(), 1);
        assert_eq!(report.byte_ranges[0].first_start, 0);
        assert_eq!(report.byte_ranges[0].first_len, 64);
        assert_eq!(report.byte_ranges[0].second_start, 192);
        assert_eq!(report.byte_ranges[0].second_len, 64);
        assert!(report.byte_ranges[0].in_bounds);
        assert!(report.byte_ranges[0].ordered_non_overlapping);
        assert_eq!(report.byte_ranges[0].gap_len, Some(128));
        assert_eq!(report.byte_ranges[0].covered_len, Some(128));
    }

    #[test]
    fn signature_research_scanner_reports_out_of_bounds_byte_range() {
        let pdf = b"%PDF-1.7\n1 0 obj\n<< /Type /Sig /SubFilter /ETSI.CAdES.detached /ByteRange [0 5 999 10] >>\nendobj\n%%EOF";

        let report = inspect_pdf_signature_markers_for_research(pdf).unwrap();

        assert_eq!(report.signature_dictionary_count, 1);
        assert_eq!(report.subfilters, vec!["ETSI.CAdES.detached"]);
        assert_eq!(report.byte_ranges.len(), 1);
        assert!(!report.byte_ranges[0].in_bounds);
        assert!(report.byte_ranges[0].ordered_non_overlapping);
        assert_eq!(report.byte_ranges[0].covered_len, Some(15));
    }

    #[test]
    fn signature_research_scanner_ignores_malformed_byte_range() {
        let pdf = b"%PDF-1.7\n1 0 obj\n<< /Type /Sig /ByteRange [0 nope 10 5] >>\nendobj\n%%EOF";

        let report = inspect_pdf_signature_markers_for_research(pdf).unwrap();

        assert_eq!(report.signature_dictionary_count, 1);
        assert!(report.byte_ranges.is_empty());
    }

    #[test]
    fn signature_research_scanner_rejects_non_pdf_magic_bytes() {
        let err = inspect_pdf_signature_markers_for_research(b"not a pdf").unwrap_err();

        assert!(matches!(err, OxideError::InvalidInput { .. }));
        assert!(err.to_string().contains("expected PDF"));
    }

    #[test]
    fn image_artifacts_to_pdf_converts_real_jpeg() {
        let image = include_bytes!("../../../tests/test.jpg");

        let pdf = image_artifacts_to_pdf(
            &[Artifact::image(image)],
            &ImageToPdfOptions::default(),
            &ResourceLimits::default(),
        )
        .unwrap();
        let document = lopdf::Document::load_mem(&pdf.bytes).unwrap();

        assert_eq!(document.get_pages().len(), 1);
    }

    #[test]
    fn image_artifacts_to_pdf_writes_one_page_per_image() {
        let image = include_bytes!("../../../tests/test.jpg");

        let pdf = image_artifacts_to_pdf(
            &[Artifact::image(image), Artifact::image(image)],
            &ImageToPdfOptions::default(),
            &ResourceLimits::default(),
        )
        .unwrap();
        let document = lopdf::Document::load_mem(&pdf.bytes).unwrap();

        assert_eq!(document.get_pages().len(), 2);
    }

    #[test]
    fn image_artifacts_to_pdf_enforces_pixel_limit() {
        let image = include_bytes!("../../../tests/test.jpg");
        let limits = ResourceLimits {
            max_pixels: Some(1),
            ..ResourceLimits::default()
        };

        let err = image_artifacts_to_pdf(
            &[Artifact::image(image)],
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
            &[Artifact::image(b"not an image")],
            &ImageToPdfOptions::default(),
            &ResourceLimits::default(),
        )
        .unwrap_err();

        assert_eq!(err, OxideError::ImageDecode);
    }

    #[test]
    fn image_artifacts_to_pdf_enforces_output_size_limit() {
        let image = include_bytes!("../../../tests/test.jpg");

        let err = image_artifacts_to_pdf(
            &[Artifact::image(image)],
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
        let pdf = include_bytes!("../../../tests/test.pdf");

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
        let pdf = include_bytes!("../../../tests/test.pdf");

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
        let pdf = include_bytes!("../../../tests/test.pdf");

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
        let pdf = include_bytes!("../../../tests/test.pdf");

        let err = extract_text_from_pdf(
            pdf,
            &ExtractTextOptions {
                format: Some("json".to_owned()),
            },
            &ResourceLimits::default(),
        )
        .unwrap_err();

        assert!(matches!(err, OxideError::InvalidInput { .. }));
        assert!(err
            .to_string()
            .contains("unsupported text extraction format"));
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
    fn pdf_operator_runner_handles_extract_text_tasks() {
        let pdf = include_bytes!("../../../tests/test.pdf");
        let mut runner = PdfOperatorRunner::default();

        let extracted = runner
            .run(
                &TaskSpec {
                    id: TaskId::new("extract"),
                    op: OperatorSpec::PdfInspect(PdfInspectOptions::ExtractText(
                        ExtractTextOptions::default(),
                    )),
                    inputs: vec![artifact_ref("source")],
                },
                &[Artifact::pdf(pdf)],
            )
            .unwrap();

        let Artifact::Text(text) = extracted else {
            panic!("expected text artifact");
        };
        assert!(!text.text.trim().is_empty());
    }

    #[test]
    fn execute_workflow_enforces_timeout() {
        let workflow = workflow_from_json(
            r#"
            {
              "version": 1,
              "inputs": [{ "id": "source", "path": "input.bin" }],
              "tasks": [
                {
                  "id": "slow",
                  "op": { "pdf_edit": { "merge": {} } },
                  "inputs": ["source"]
                }
              ],
              "outputs": [{ "id": "final", "from": "slow", "path": "out.bin" }],
              "limits": { "timeout_ms": 1 }
            }
            "#,
        );
        let mut store = ArtifactStore::new();
        store.insert(artifact_ref("source"), Artifact::bytes(b"input"));
        let mut runner = SlowRunner;

        let err = execute_workflow(&workflow, store, &mut runner).unwrap_err();

        assert_eq!(
            err,
            OxideError::ResourceLimitExceeded {
                limit: "timeout_ms".to_owned()
            }
        );
    }

    #[test]
    fn execute_workflow_enforces_total_input_size_limit() {
        let workflow = workflow_from_json(
            r#"
            {
              "version": 1,
              "inputs": [
                { "id": "first", "path": "a.bin" },
                { "id": "second", "path": "b.bin" }
              ],
              "tasks": [],
              "outputs": [{ "id": "final", "from": "first", "path": "out.bin" }],
              "limits": { "max_total_input_bytes": 9 }
            }
            "#,
        );
        let mut store = ArtifactStore::new();
        store.insert(artifact_ref("first"), Artifact::bytes(b"12345"));
        store.insert(artifact_ref("second"), Artifact::bytes(b"67890"));
        let mut runner = RecordingRunner::default();

        let err = execute_workflow(&workflow, store, &mut runner).unwrap_err();

        assert_eq!(
            err,
            OxideError::ResourceLimitExceeded {
                limit: "max_total_input_bytes".to_owned()
            }
        );
        assert!(runner.executed.is_empty());
    }

    #[test]
    fn watermark_pdf_adds_text_watermark_to_selected_page() {
        let pdf = include_bytes!("../../../tests/test.pdf");

        let watermarked = watermark_pdf_artifacts(
            &[Artifact::pdf(pdf)],
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
        let pdf = include_bytes!("../../../tests/test.pdf");

        let err = watermark_pdf_artifacts(
            &[Artifact::pdf(pdf)],
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
        let pdf = include_bytes!("../../../tests/test.pdf");
        let image = include_bytes!("../../../tests/test.jpg");

        let err = watermark_pdf_artifacts(
            &[Artifact::pdf(pdf), Artifact::image(image)],
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
        let pdf = include_bytes!("../../../tests/test.pdf");
        let image = include_bytes!("../../../tests/test.jpg");

        let watermarked = watermark_pdf_artifacts(
            &[Artifact::pdf(pdf), Artifact::image(image)],
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

    #[test]
    fn watermark_pdf_adds_svg_watermark_as_vector_xobject() {
        let pdf = include_bytes!("../../../tests/test.pdf");
        let svg = simple_svg();

        let watermarked = watermark_pdf_artifacts(
            &[Artifact::pdf(pdf), Artifact::svg(svg)],
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
        assert!(!form_operators
            .windows(2)
            .any(|operators| operators == ["re", "S"]));
    }

    #[test]
    fn watermark_pdf_rasterizes_svg_only_when_requested() {
        let pdf = include_bytes!("../../../tests/test.pdf");
        let svg = simple_svg();

        let watermarked = watermark_pdf_artifacts(
            &[Artifact::pdf(pdf), Artifact::svg(svg)],
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
        let pdf = include_bytes!("../../../tests/test.pdf");

        let err = watermark_pdf_artifacts(
            &[Artifact::pdf(pdf), Artifact::svg(b"<svg><broken>")],
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

    fn workflow_from_json(json: &str) -> Workflow {
        serde_json::from_str(json).unwrap()
    }

    fn artifact_ref(value: &str) -> ArtifactRef {
        serde_json::from_str(&format!("{value:?}")).unwrap()
    }

    fn write_test_trust_anchors(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "oxidepdf_core_{name}_{}_anchors.pem",
            std::process::id()
        ));
        std::fs::write(
            &path,
            include_bytes!("../../../tests/fixtures/test-trust-anchor.txt"),
        )
        .unwrap();
        path
    }

    fn write_empty_trust_anchors(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "oxidepdf_core_{name}_{}_anchors.pem",
            std::process::id()
        ));
        std::fs::write(&path, "not a certificate bundle\n").unwrap();
        path
    }

    fn write_invalid_trust_anchors(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "oxidepdf_core_{name}_{}_anchors.pem",
            std::process::id()
        ));
        std::fs::write(
            &path,
            "-----BEGIN CERTIFICATE-----\nMIIB\n-----END CERTIFICATE-----\n",
        )
        .unwrap();
        path
    }

    fn pdf_with_signature_dictionary(byte_range: Vec<i64>, contents: Vec<u8>) -> Vec<u8> {
        let mut document = lopdf::Document::with_version("1.7");
        let pages_id = document.new_object_id();
        let page_id = document.new_object_id();
        let sig_field_id = document.new_object_id();
        let sig_value_id = document.new_object_id();
        let acroform_id = document.new_object_id();
        let catalog_id = document.new_object_id();

        let byte_range = byte_range
            .into_iter()
            .map(lopdf::Object::Integer)
            .collect::<Vec<_>>();
        let sig_value = lopdf::dictionary! {
            "Type" => "Sig",
            "Filter" => "Adobe.PPKLite",
            "SubFilter" => "adbe.pkcs7.detached",
            "ByteRange" => lopdf::Object::Array(byte_range),
            "Contents" => lopdf::Object::String(contents, lopdf::StringFormat::Hexadecimal),
        };
        document
            .objects
            .insert(sig_value_id, lopdf::Object::Dictionary(sig_value));

        let sig_field = lopdf::dictionary! {
            "Type" => "Annot",
            "Subtype" => "Widget",
            "FT" => "Sig",
            "T" => lopdf::Object::string_literal("Approval"),
            "V" => sig_value_id,
            "Rect" => lopdf::Object::Array(vec![0.into(), 0.into(), 0.into(), 0.into()]),
            "P" => page_id,
        };
        document
            .objects
            .insert(sig_field_id, lopdf::Object::Dictionary(sig_field));

        let page = lopdf::dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => lopdf::Object::Array(vec![0.into(), 0.into(), 200.into(), 200.into()]),
            "Annots" => lopdf::Object::Array(vec![sig_field_id.into()]),
        };
        document
            .objects
            .insert(page_id, lopdf::Object::Dictionary(page));

        let pages = lopdf::dictionary! {
            "Type" => "Pages",
            "Kids" => lopdf::Object::Array(vec![page_id.into()]),
            "Count" => 1,
        };
        document
            .objects
            .insert(pages_id, lopdf::Object::Dictionary(pages));

        let acroform = lopdf::dictionary! {
            "Fields" => lopdf::Object::Array(vec![sig_field_id.into()]),
        };
        document
            .objects
            .insert(acroform_id, lopdf::Object::Dictionary(acroform));

        let catalog = lopdf::dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
            "AcroForm" => acroform_id,
        };
        document
            .objects
            .insert(catalog_id, lopdf::Object::Dictionary(catalog));
        document.trailer.set("Root", catalog_id);

        let mut bytes = Vec::new();
        document.save_to(&mut bytes).unwrap();
        bytes
    }

    fn assert_page_numbers(document: &lopdf::Document, expected: &[u32]) {
        let pages = document.get_pages();
        let actual = pages.keys().copied().collect::<Vec<_>>();
        assert_eq!(actual, expected);
    }

    fn page_rotation(document: &lopdf::Document, page_number: u32) -> i64 {
        let page_id = document.get_pages().get(&page_number).copied().unwrap();
        let page = document.get_object(page_id).unwrap().as_dict().unwrap();
        page.get(b"Rotate")
            .and_then(lopdf::Object::as_i64)
            .unwrap_or(0)
    }

    fn page_optional_box(
        document: &lopdf::Document,
        page_number: u32,
        key: &[u8],
    ) -> Option<[f32; 4]> {
        let page_id = document.get_pages().get(&page_number).copied().unwrap();
        let page = document.get_object(page_id).unwrap().as_dict().unwrap();
        let values = page.get(key).ok()?.as_array().ok()?;
        Some([
            object_to_f32(&values[0]).unwrap(),
            object_to_f32(&values[1]).unwrap(),
            object_to_f32(&values[2]).unwrap(),
            object_to_f32(&values[3]).unwrap(),
        ])
    }

    fn page_box(document: &lopdf::Document, page_number: u32, key: &[u8]) -> [f32; 4] {
        page_optional_box(document, page_number, key).unwrap()
    }

    fn page_content_contains(document: &lopdf::Document, page_number: u32, operator: &str) -> bool {
        let page_id = document.get_pages().get(&page_number).copied().unwrap();
        document
            .get_page_content(page_id)
            .ok()
            .and_then(|content| lopdf::content::Content::decode(&content).ok())
            .is_some_and(|content| {
                content
                    .operations
                    .iter()
                    .any(|operation| operation.operator == operator)
            })
    }

    fn pdf_with_blank_and_marked_page() -> Vec<u8> {
        let mut document = lopdf::Document::with_version("1.7");
        let pages_id = document.new_object_id();
        let blank_page_id = document.new_object_id();
        let marked_page_id = document.new_object_id();
        let marked_content_id = document.new_object_id();
        let catalog_id = document.new_object_id();

        let marked_content = lopdf::content::Content {
            operations: vec![lopdf::content::Operation::new("q", vec![])],
        }
        .encode()
        .unwrap();
        document.objects.insert(
            marked_content_id,
            Object::Stream(Stream::new(Dictionary::new(), marked_content)),
        );
        document.objects.insert(
            blank_page_id,
            Object::Dictionary(lopdf::dictionary! {
                "Type" => "Page",
                "Parent" => pages_id,
                "MediaBox" => Object::Array(vec![0.into(), 0.into(), 100.into(), 100.into()]),
            }),
        );
        document.objects.insert(
            marked_page_id,
            Object::Dictionary(lopdf::dictionary! {
                "Type" => "Page",
                "Parent" => pages_id,
                "MediaBox" => Object::Array(vec![0.into(), 0.into(), 100.into(), 100.into()]),
                "Contents" => marked_content_id,
            }),
        );
        document.objects.insert(
            pages_id,
            Object::Dictionary(lopdf::dictionary! {
                "Type" => "Pages",
                "Kids" => Object::Array(vec![blank_page_id.into(), marked_page_id.into()]),
                "Count" => 2,
            }),
        );
        document.objects.insert(
            catalog_id,
            Object::Dictionary(lopdf::dictionary! {
                "Type" => "Catalog",
                "Pages" => pages_id,
            }),
        );
        document.trailer.set("Root", catalog_id);

        let mut bytes = Vec::new();
        document.save_to(&mut bytes).unwrap();
        bytes
    }

    fn pdf_with_blank_page_and_missing_resources() -> Vec<u8> {
        let mut document = lopdf::Document::with_version("1.7");
        let pages_id = document.new_object_id();
        let page_id = document.new_object_id();
        let catalog_id = document.new_object_id();

        document.objects.insert(
            page_id,
            Object::Dictionary(lopdf::dictionary! {
                "Type" => "Page",
                "Parent" => pages_id,
                "MediaBox" => Object::Array(vec![0.into(), 0.into(), 100.into(), 100.into()]),
                "Resources" => Object::Reference((99, 0)),
            }),
        );
        document.objects.insert(
            pages_id,
            Object::Dictionary(lopdf::dictionary! {
                "Type" => "Pages",
                "Kids" => Object::Array(vec![page_id.into()]),
                "Count" => 1,
            }),
        );
        document.objects.insert(
            catalog_id,
            Object::Dictionary(lopdf::dictionary! {
                "Type" => "Catalog",
                "Pages" => pages_id,
            }),
        );
        document.trailer.set("Root", catalog_id);

        let mut bytes = Vec::new();
        document.save_to(&mut bytes).unwrap();
        bytes
    }

    fn empty_page_pdf() -> Vec<u8> {
        let mut pdf = pdf_writer::Pdf::new();
        let catalog_id = pdf_writer::Ref::new(1);
        let pages_id = pdf_writer::Ref::new(2);
        let page_id = pdf_writer::Ref::new(3);

        pdf.catalog(catalog_id).pages(pages_id);
        pdf.pages(pages_id).kids([page_id]).count(1);
        let mut page = pdf.page(page_id);
        page.media_box(pdf_writer::Rect::new(0.0, 0.0, A4_WIDTH, A4_HEIGHT));
        page.parent(pages_id);
        page.finish();

        pdf.finish()
    }

    fn page_resources(document: &lopdf::Document, page_number: u32) -> Dictionary {
        let page_id = document.get_pages().get(&page_number).copied().unwrap();
        let resources = document
            .get_dictionary(page_id)
            .unwrap()
            .get(b"Resources")
            .unwrap();
        match resources {
            Object::Dictionary(dictionary) => dictionary.clone(),
            Object::Reference(id) => document.get_dictionary(*id).unwrap().clone(),
            other => panic!("unexpected resources object: {other:?}"),
        }
    }

    fn page_content_contains_operator(
        document: &lopdf::Document,
        page_number: u32,
        operator: &str,
    ) -> bool {
        let page_id = document.get_pages().get(&page_number).copied().unwrap();
        document
            .get_page_contents(page_id)
            .into_iter()
            .filter_map(|content_id| document.get_object(content_id).ok())
            .filter_map(|object| object.as_stream().ok())
            .filter_map(|stream| lopdf::content::Content::decode(&stream.content).ok())
            .flat_map(|content| content.operations)
            .any(|operation| operation.operator == operator)
    }

    fn page_xobject_subtypes(document: &lopdf::Document, page_number: u32) -> Vec<Vec<u8>> {
        let resources = page_resources(document, page_number);
        let Ok(xobjects) = resources.get(b"XObject").and_then(Object::as_dict) else {
            return Vec::new();
        };
        xobjects
            .iter()
            .filter_map(|(_, object)| object.as_reference().ok())
            .filter_map(|id| document.get_object(id).ok())
            .filter_map(|object| object.as_stream().ok())
            .filter_map(|stream| stream.dict.get(b"Subtype").and_then(Object::as_name).ok())
            .map(|name| name.to_vec())
            .collect()
    }

    fn page_xobject_count(document: &lopdf::Document, page_number: u32) -> usize {
        let resources = page_resources(document, page_number);
        resources
            .get(b"XObject")
            .and_then(Object::as_dict)
            .map(|dictionary| dictionary.len())
            .unwrap_or(0)
    }

    fn page_content_text_contains(
        document: &lopdf::Document,
        page_number: u32,
        expected: &str,
    ) -> bool {
        let page_id = document.get_pages().get(&page_number).copied().unwrap();
        String::from_utf8_lossy(&document.get_page_content(page_id).unwrap()).contains(expected)
    }

    fn page_form_xobject_operators(document: &lopdf::Document, page_number: u32) -> Vec<String> {
        let resources = page_resources(document, page_number);
        let Ok(xobjects) = resources.get(b"XObject").and_then(Object::as_dict) else {
            return Vec::new();
        };
        let mut operators = Vec::new();
        let mut seen = BTreeSet::new();
        for (_, object) in xobjects.iter() {
            if let Ok(id) = object.as_reference() {
                collect_form_xobject_operators(document, id, &mut seen, &mut operators);
            }
        }
        operators
    }

    fn collect_form_xobject_operators(
        document: &lopdf::Document,
        object_id: lopdf::ObjectId,
        seen: &mut BTreeSet<lopdf::ObjectId>,
        operators: &mut Vec<String>,
    ) {
        if !seen.insert(object_id) {
            return;
        }
        let Ok(stream) = document
            .get_object(object_id)
            .and_then(lopdf::Object::as_stream)
        else {
            return;
        };
        if stream
            .dict
            .get(b"Subtype")
            .and_then(lopdf::Object::as_name)
            .ok()
            != Some(b"Form".as_slice())
        {
            return;
        }
        if let Ok(content) = stream.get_plain_content() {
            if let Ok(content) = lopdf::content::Content::decode(&content) {
                operators.extend(
                    content
                        .operations
                        .into_iter()
                        .map(|operation| operation.operator),
                );
            }
        }
        let Ok(resources) = stream.dict.get(b"Resources").and_then(Object::as_dict) else {
            return;
        };
        let Ok(xobjects) = resources.get(b"XObject").and_then(Object::as_dict) else {
            return;
        };
        for (_, object) in xobjects.iter() {
            if let Ok(id) = object.as_reference() {
                collect_form_xobject_operators(document, id, seen, operators);
            }
        }
    }

    fn simple_svg() -> &'static [u8] {
        br##"<svg xmlns="http://www.w3.org/2000/svg" width="120" height="80">
            <rect x="10" y="10" width="100" height="60" fill="#16a34a"/>
        </svg>"##
    }

    #[derive(Default)]
    struct RecordingRunner {
        executed: Vec<String>,
        fail_on: Option<&'static str>,
        error: Option<OxideError>,
    }

    impl OperatorRunner for RecordingRunner {
        fn run(&mut self, task: &TaskSpec, _inputs: &[Artifact]) -> Result<Artifact, OxideError> {
            self.executed.push(task.id.as_str().to_owned());
            if self.fail_on == Some(task.id.as_str()) {
                return Err(self.error.take().unwrap());
            }

            Ok(Artifact::bytes(task.id.as_str().as_bytes()))
        }
    }

    struct SlowRunner;

    impl OperatorRunner for SlowRunner {
        fn run(&mut self, _task: &TaskSpec, _inputs: &[Artifact]) -> Result<Artifact, OxideError> {
            std::thread::sleep(std::time::Duration::from_millis(5));
            Ok(Artifact::bytes(b"finished"))
        }
    }
}
