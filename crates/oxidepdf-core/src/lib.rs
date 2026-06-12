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
    ArtifactRef, InputSpec, OperatorSpec, OutputSpec, ResourceLimits, TaskId, TaskSpec, Workflow,
    WorkflowMetadata, WorkflowVersion, WORKFLOW_SCHEMA_VERSION,
};

use lopdf::{dictionary, Dictionary, Object, Stream};
use pdf_writer::Finish;
use read_fonts::TableProvider;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use x509_cert::{der::Decode, Certificate};

const A4_WIDTH: f32 = 595.0;
const A4_HEIGHT: f32 = 842.0;

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

/// PDF password, encryption, and permission operations.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PdfSecurityOptions {
    /// Explicit operation name. Stage 18 implements concrete operations.
    pub operation: String,
}

/// PDF comparison operations.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PdfCompareOptions {
    /// Explicit comparison mode. Stage 19 implements concrete modes.
    pub mode: String,
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

/// Options for merge.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MergeOptions {}

/// Options for split.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SplitOptions {
    /// Page range, for example `1,3-5`.
    pub pages: String,
}

/// Options for reorder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReorderOptions {
    /// Explicit page sequence, for example `3,1,2`.
    pub pages: String,
}

/// Options for rotate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RotateOptions {
    /// Page range, for example `1,3-5`.
    pub pages: String,
    /// Rotation in degrees. Validation happens in the workflow validator.
    pub degrees: i16,
}

/// Options for page-selection edits.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageSelectionOptions {
    /// Page range, for example `1,3-5`.
    pub pages: String,
}

/// Options for deleting structurally blank pages.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DeleteBlankPagesOptions {}

/// Options for cropping pages.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CropPagesOptions {
    /// Page range, for example `1,3-5`.
    pub pages: Option<String>,
    /// Left coordinate of the new CropBox.
    pub left: f32,
    /// Bottom coordinate of the new CropBox.
    pub bottom: f32,
    /// Right coordinate of the new CropBox.
    pub right: f32,
    /// Top coordinate of the new CropBox.
    pub top: f32,
}

/// Options for scaling pages.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScalePagesOptions {
    /// Page range, for example `1,3-5`.
    pub pages: Option<String>,
    /// Scale factor applied to page boxes and page contents.
    pub factor: f32,
}

/// Options for combining pages into one tall page.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SinglePageOptions {}

/// Options for N-up page layout.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NUpOptions {
    /// Number of columns on each output page.
    pub columns: u32,
    /// Number of rows on each output page.
    pub rows: u32,
}

/// Options for booklet imposition.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct BookletOptions {}

/// Options for adding page numbers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PageNumbersOptions {
    /// Page range, for example `1,3-5`. Defaults to all pages.
    pub pages: Option<String>,
    /// First number written on the first selected page.
    pub start: u32,
    /// Text before the number.
    pub prefix: String,
    /// Text after the number.
    pub suffix: String,
    /// Font size in PDF points.
    pub font_size: f32,
    /// Page number placement.
    pub position: PageNumberPosition,
}

impl Default for PageNumbersOptions {
    fn default() -> Self {
        Self {
            pages: None,
            start: 1,
            prefix: String::new(),
            suffix: String::new(),
            font_size: 12.0,
            position: PageNumberPosition::BottomCenter,
        }
    }
}

/// Page number placement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PageNumberPosition {
    /// Top-left corner.
    TopLeft,
    /// Top-center edge.
    TopCenter,
    /// Top-right corner.
    TopRight,
    /// Bottom-left corner.
    BottomLeft,
    /// Bottom-center edge.
    BottomCenter,
    /// Bottom-right corner.
    BottomRight,
}

/// Options for image-to-PDF conversion.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ImageToPdfOptions {
    /// Layout mode such as `fit`, `fill`, or `original_size`.
    pub layout: Option<String>,
}

/// Options for SVG-to-PDF conversion.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SvgToPdfOptions {
    /// User-selected rasterization mode. Defaults to vector output when false.
    pub rasterize: bool,
}

/// Options for text extraction.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ExtractTextOptions {
    /// Output format, initially `plain`.
    pub format: Option<String>,
}

/// Options for watermarking.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WatermarkOptions {
    /// Watermark kind.
    pub kind: WatermarkKind,
    /// Text for text watermarks.
    pub text: Option<String>,
    /// Font family name discovered via fontdb.
    pub font: Option<String>,
    /// Explicit font file for text watermarks.
    pub font_path: Option<PathBuf>,
    /// Font size in PDF points.
    pub font_size: Option<f32>,
    /// Opacity from 0.0 to 1.0.
    pub opacity: Option<f32>,
    /// Rotation in degrees.
    pub rotation: Option<f32>,
    /// Position such as `center`.
    pub position: Option<String>,
    /// Page range, for example `1,3-5`. Defaults to all pages.
    pub pages: Option<String>,
    /// Scale for image and SVG watermarks.
    pub scale: Option<f32>,
    /// Rasterize SVG before watermarking. Defaults to vector output when false.
    #[serde(default)]
    pub rasterize: bool,
}

/// Watermark content kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WatermarkKind {
    /// Text watermark.
    Text,
    /// Image watermark.
    Image,
    /// SVG watermark.
    Svg,
}

/// Options for rendering.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RenderOptions {
    /// One-based page number.
    pub page: u32,
    /// Optional output format such as `png`.
    pub format: Option<String>,
    /// Optional render scale.
    pub scale: Option<f32>,
}

/// Options for signature and certificate operations.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SignatureOptions {
    /// Requested signature operation.
    pub mode: SignatureMode,
    /// PEM file containing explicit trust anchors for chain validation.
    pub trust_anchors: Option<PathBuf>,
}

/// Requested signature operation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignatureMode {
    /// Verify PDF signatures and embedded certificate material.
    #[default]
    Verify,
}

/// Top-level signature verification report emitted as JSON.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignatureVerificationReport {
    /// Overall verification verdict.
    pub verdict: SignatureVerdict,
    /// Number of trust anchors accepted from the explicit PEM input.
    pub trust_anchor_count: usize,
    /// Per-signature reports.
    pub signatures: Vec<SignatureEntryReport>,
    /// Top-level diagnostics.
    pub diagnostics: Vec<SignatureDiagnostic>,
}

/// Stable signature verification verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignatureVerdict {
    /// All required checks completed and the signature chains to a trust anchor.
    Trusted,
    /// At least one completed check proved the signature invalid.
    Invalid,
    /// Verification completed but trust could not be established offline.
    Indeterminate,
    /// The input uses a signature feature not supported by this build.
    Unsupported,
}

/// Per-signature report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignatureEntryReport {
    /// Optional field name from the PDF form tree.
    pub field_name: Option<String>,
    /// Signature dictionary SubFilter value.
    pub subfilter: Option<String>,
    /// ByteRange structural status.
    pub byte_range: ByteRangeVerification,
    /// Contents coverage status.
    pub contents: ContentsVerification,
    /// CMS parse/validation status.
    pub cms_status: SignatureCheckStatus,
    /// Signed content digest status.
    pub digest_status: SignatureCheckStatus,
    /// Signer signature mathematics status.
    pub signature_status: SignatureCheckStatus,
    /// Certificate chain status.
    pub certificate_chain_status: SignatureCheckStatus,
    /// Offline revocation status.
    pub revocation_status: SignatureCheckStatus,
    /// Timestamp token validation status.
    pub timestamp_status: SignatureCheckStatus,
    /// Per-signature diagnostics.
    pub diagnostics: Vec<SignatureDiagnostic>,
}

/// ByteRange check result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ByteRangeVerification {
    /// Parsed ByteRange values.
    pub values: Option<[u64; 4]>,
    /// Whether the ranges are in input bounds.
    pub in_bounds: bool,
    /// Whether the ranges are ordered and non-overlapping.
    pub ordered_non_overlapping: bool,
    /// Length of the unsigned gap between signed ranges.
    pub gap_len: Option<u64>,
    /// Total covered bytes.
    pub covered_len: Option<u64>,
}

/// Contents coverage check result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentsVerification {
    /// Number of bytes in the signature Contents value.
    pub byte_len: Option<usize>,
    /// Whether the ByteRange gap can contain the Contents placeholder.
    pub covered_by_gap: bool,
}

/// Status for an individual signature check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignatureCheckStatus {
    /// Stable status code.
    pub status: SignatureCheckState,
    /// Non-sensitive detail.
    pub detail: String,
}

/// Stable signature check state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignatureCheckState {
    /// Check completed successfully.
    Passed,
    /// Check completed and failed.
    Failed,
    /// Check could not establish a definite result.
    Indeterminate,
    /// Check is not implemented in this build.
    Unsupported,
}

/// Non-sensitive signature diagnostic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignatureDiagnostic {
    /// Stable diagnostic code.
    pub code: String,
    /// Non-sensitive diagnostic message.
    pub message: String,
}

/// Artifact produced or consumed by workflow tasks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Artifact {
    /// PDF artifact placeholder.
    Pdf(PdfArtifact),
    /// Image artifact placeholder.
    Image(ImageArtifact),
    /// Text artifact.
    Text(TextArtifact),
    /// SVG artifact.
    Svg(SvgArtifact),
    /// Raw bytes.
    Bytes(BytesArtifact),
}

impl Artifact {
    /// Creates a raw byte artifact.
    pub fn bytes(bytes: impl AsRef<[u8]>) -> Self {
        Self::Bytes(BytesArtifact {
            bytes: bytes.as_ref().to_vec(),
        })
    }

    /// Creates a PDF artifact.
    pub fn pdf(bytes: impl AsRef<[u8]>) -> Self {
        Self::Pdf(PdfArtifact {
            bytes: bytes.as_ref().to_vec(),
        })
    }

    /// Creates an image artifact.
    pub fn image(bytes: impl AsRef<[u8]>) -> Self {
        Self::Image(ImageArtifact {
            bytes: bytes.as_ref().to_vec(),
        })
    }

    /// Creates an SVG artifact.
    pub fn svg(bytes: impl AsRef<[u8]>) -> Self {
        Self::Svg(SvgArtifact {
            bytes: bytes.as_ref().to_vec(),
        })
    }
}

/// PDF artifact placeholder for later operators.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdfArtifact {
    /// Serialized bytes until object-level artifacts are added.
    pub bytes: Vec<u8>,
}

/// Image artifact placeholder for later operators.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageArtifact {
    /// Encoded image bytes.
    pub bytes: Vec<u8>,
}

/// Text artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextArtifact {
    /// Extracted or generated text.
    pub text: String,
    /// Page-level extraction diagnostics reserved for structured output.
    pub diagnostics: Vec<TextExtractionDiagnostic>,
}

/// Page-level diagnostic emitted by text extraction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextExtractionDiagnostic {
    /// One-based page number.
    pub page: u32,
    /// Stable diagnostic code.
    pub code: TextExtractionDiagnosticCode,
    /// Non-sensitive diagnostic message.
    pub message: String,
}

/// Stable text extraction diagnostic code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextExtractionDiagnosticCode {
    /// Page has no extractable text layer.
    NoTextLayer,
}

/// SVG artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SvgArtifact {
    /// SVG document bytes.
    pub bytes: Vec<u8>,
}

/// Byte artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BytesArtifact {
    /// Raw bytes.
    pub bytes: Vec<u8>,
}

/// In-memory artifact store used by the serial executor.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ArtifactStore {
    artifacts: BTreeMap<ArtifactRef, Artifact>,
}

impl ArtifactStore {
    /// Creates an empty store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts or replaces an artifact.
    pub fn insert(&mut self, id: ArtifactRef, artifact: Artifact) -> Option<Artifact> {
        self.artifacts.insert(id, artifact)
    }

    /// Returns an artifact by id.
    pub fn get(&self, id: &ArtifactRef) -> Option<&Artifact> {
        self.artifacts.get(id)
    }
}

/// Validated workflow execution plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionPlan {
    /// Task ids in topological execution order.
    pub task_order: Vec<TaskId>,
}

/// Result of a successful workflow execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionResult {
    /// Validated execution plan used for this run.
    pub plan: ExecutionPlan,
    /// Artifact store containing inputs and task outputs.
    pub store: ArtifactStore,
}

/// Operator implementation boundary used by the serial executor.
pub trait OperatorRunner {
    /// Runs a task against resolved input artifacts.
    fn run(&mut self, task: &TaskSpec, inputs: &[Artifact]) -> Result<Artifact, OxideError>;
}

/// Operator runner for object-level PDF page editing.
#[derive(Debug, Clone, Default)]
pub struct PdfOperatorRunner {
    limits: ResourceLimits,
}

impl PdfOperatorRunner {
    /// Creates a runner using explicit workflow resource limits.
    pub fn with_limits(limits: ResourceLimits) -> Self {
        Self { limits }
    }
}

impl OperatorRunner for PdfOperatorRunner {
    fn run(&mut self, task: &TaskSpec, inputs: &[Artifact]) -> Result<Artifact, OxideError> {
        let artifact = match &task.op {
            OperatorSpec::PdfEdit(options) => run_pdf_edit(options, inputs, &self.limits),
            OperatorSpec::PdfInspect(options) => run_pdf_inspect(options, inputs, &self.limits),
            OperatorSpec::PdfSecurity(options) => run_pdf_security(options),
            OperatorSpec::PdfCompare(options) => run_pdf_compare(options),
            OperatorSpec::PdfSign(options) => run_pdf_sign(options, inputs, &self.limits),
        }?;
        enforce_artifact_output_bytes(&artifact, &self.limits)?;
        Ok(artifact)
    }
}

fn run_pdf_edit(
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

fn run_pdf_inspect(
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

fn run_pdf_security(options: &PdfSecurityOptions) -> Result<Artifact, OxideError> {
    Err(OxideError::UnsupportedPdfFeature {
        feature: format!("pdf_security operation '{}'", options.operation),
    })
}

fn run_pdf_compare(options: &PdfCompareOptions) -> Result<Artifact, OxideError> {
    Err(OxideError::UnsupportedPdfFeature {
        feature: format!("pdf_compare mode '{}'", options.mode),
    })
}

fn run_pdf_sign(
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

/// Merges multiple PDF artifacts into a single PDF.
pub fn merge_pdf_artifacts(inputs: &[Artifact]) -> Result<PdfArtifact, OxideError> {
    merge_pdf_artifacts_with_limits(inputs, &ResourceLimits::default())
}

/// Merges multiple PDF artifacts into a single PDF while enforcing resource limits.
pub fn merge_pdf_artifacts_with_limits(
    inputs: &[Artifact],
    limits: &ResourceLimits,
) -> Result<PdfArtifact, OxideError> {
    if inputs.len() < 2 {
        return Err(OxideError::InvalidInput {
            reason: "merge requires at least two PDF inputs".to_owned(),
        });
    }

    let mut documents = Vec::with_capacity(inputs.len());
    let mut total_pages = 0usize;
    for input in inputs {
        let bytes = pdf_bytes(input)?;
        enforce_input_bytes(bytes.len(), limits)?;
        let document = load_pdf(bytes)?;
        total_pages = total_pages
            .checked_add(document.get_pages().len())
            .ok_or_else(|| resource_limit("max_pages"))?;
        enforce_max_pages(total_pages, limits)?;
        documents.push(document);
    }

    let bytes = merge_documents(documents)?;
    enforce_output_bytes(bytes.len(), limits)?;
    Ok(PdfArtifact { bytes })
}

/// Splits a PDF by keeping the specified one-based pages.
pub fn split_pdf(input: &[u8], pages: &str) -> Result<PdfArtifact, OxideError> {
    split_pdf_with_limits(input, pages, &ResourceLimits::default())
}

/// Splits a PDF by keeping the specified one-based pages while enforcing resource limits.
pub fn split_pdf_with_limits(
    input: &[u8],
    pages: &str,
    limits: &ResourceLimits,
) -> Result<PdfArtifact, OxideError> {
    enforce_input_bytes(input.len(), limits)?;
    let mut document = load_pdf(input)?;
    enforce_max_pages(document.get_pages().len(), limits)?;
    let selected_pages = parse_page_range(pages, document.get_pages().len() as u32)?;
    keep_pages(&mut document, &selected_pages)?;
    let bytes = save_pdf(document)?;
    enforce_output_bytes(bytes.len(), limits)?;
    Ok(PdfArtifact { bytes })
}

/// Extracts selected PDF pages.
pub fn extract_pdf_pages(input: &[u8], pages: &str) -> Result<PdfArtifact, OxideError> {
    extract_pdf_pages_with_limits(input, pages, &ResourceLimits::default())
}

/// Extracts selected PDF pages while enforcing resource limits.
pub fn extract_pdf_pages_with_limits(
    input: &[u8],
    pages: &str,
    limits: &ResourceLimits,
) -> Result<PdfArtifact, OxideError> {
    split_pdf_with_limits(input, pages, limits)
}

/// Reorders a PDF by an explicit one-based page sequence.
pub fn reorder_pdf(input: &[u8], pages: &str) -> Result<PdfArtifact, OxideError> {
    reorder_pdf_with_limits(input, pages, &ResourceLimits::default())
}

/// Reorders a PDF by an explicit one-based page sequence while enforcing resource limits.
pub fn reorder_pdf_with_limits(
    input: &[u8],
    pages: &str,
    limits: &ResourceLimits,
) -> Result<PdfArtifact, OxideError> {
    enforce_input_bytes(input.len(), limits)?;
    let mut document = load_pdf(input)?;
    enforce_max_pages(document.get_pages().len(), limits)?;
    let selected_pages = parse_page_range(pages, document.get_pages().len() as u32)?;
    keep_pages(&mut document, &selected_pages)?;
    let bytes = save_pdf(document)?;
    enforce_output_bytes(bytes.len(), limits)?;
    Ok(PdfArtifact { bytes })
}

/// Rotates selected PDF pages by 90, 180, or 270 degrees.
pub fn rotate_pdf(input: &[u8], pages: &str, degrees: i16) -> Result<PdfArtifact, OxideError> {
    rotate_pdf_with_limits(input, pages, degrees, &ResourceLimits::default())
}

/// Rotates selected PDF pages by 90, 180, or 270 degrees while enforcing resource limits.
pub fn rotate_pdf_with_limits(
    input: &[u8],
    pages: &str,
    degrees: i16,
    limits: &ResourceLimits,
) -> Result<PdfArtifact, OxideError> {
    enforce_input_bytes(input.len(), limits)?;
    let mut document = load_pdf(input)?;
    enforce_max_pages(document.get_pages().len(), limits)?;
    let selected_pages = parse_page_range(pages, document.get_pages().len() as u32)?;
    let degrees = normalize_rotation(degrees)?;
    let pages = document.get_pages();

    for page_number in selected_pages {
        let page_id = pages
            .get(&page_number)
            .ok_or_else(|| OxideError::InvalidInput {
                reason: format!("page {page_number} is out of range"),
            })?;
        let page_dict = document
            .get_object_mut(*page_id)
            .and_then(lopdf::Object::as_dict_mut)
            .map_err(|_| OxideError::ParsePdf)?;
        let current_rotation = page_dict
            .get(b"Rotate")
            .and_then(lopdf::Object::as_i64)
            .unwrap_or(0);
        page_dict.set(
            "Rotate",
            (current_rotation + i64::from(degrees)).rem_euclid(360),
        );
    }

    let bytes = save_pdf(document)?;
    enforce_output_bytes(bytes.len(), limits)?;
    Ok(PdfArtifact { bytes })
}

/// Deletes selected PDF pages.
pub fn delete_pdf_pages(input: &[u8], pages: &str) -> Result<PdfArtifact, OxideError> {
    delete_pdf_pages_with_limits(input, pages, &ResourceLimits::default())
}

/// Deletes selected PDF pages while enforcing resource limits.
pub fn delete_pdf_pages_with_limits(
    input: &[u8],
    pages: &str,
    limits: &ResourceLimits,
) -> Result<PdfArtifact, OxideError> {
    enforce_input_bytes(input.len(), limits)?;
    let mut document = load_pdf(input)?;
    let page_count = document.get_pages().len() as u32;
    enforce_max_pages(page_count as usize, limits)?;
    let deleted_pages = parse_page_range(pages, page_count)?;
    if deleted_pages.len() as u32 == page_count {
        return Err(OxideError::InvalidInput {
            reason: "delete_pages must leave at least one page".to_owned(),
        });
    }
    let kept_pages = (1..=page_count)
        .filter(|page| !deleted_pages.contains(page))
        .collect::<Vec<_>>();
    keep_pages(&mut document, &kept_pages)?;
    let bytes = save_pdf(document)?;
    enforce_output_bytes(bytes.len(), limits)?;
    Ok(PdfArtifact { bytes })
}

/// Deletes structurally blank pages.
pub fn delete_blank_pdf_pages(
    input: &[u8],
    options: &DeleteBlankPagesOptions,
) -> Result<PdfArtifact, OxideError> {
    delete_blank_pdf_pages_with_limits(input, options, &ResourceLimits::default())
}

/// Deletes structurally blank pages while enforcing resource limits.
pub fn delete_blank_pdf_pages_with_limits(
    input: &[u8],
    _options: &DeleteBlankPagesOptions,
    limits: &ResourceLimits,
) -> Result<PdfArtifact, OxideError> {
    enforce_input_bytes(input.len(), limits)?;
    let mut document = load_pdf(input)?;
    let page_count = document.get_pages().len() as u32;
    enforce_max_pages(page_count as usize, limits)?;
    let page_map = document.get_pages();
    let mut blank_pages = Vec::new();
    for (page_number, page_id) in page_map {
        if page_is_structurally_blank(&document, page_id)? {
            blank_pages.push(page_number);
        }
    }
    if blank_pages.is_empty() {
        return Err(OxideError::InvalidInput {
            reason: "PDF contains no structurally blank pages".to_owned(),
        });
    }
    if blank_pages.len() as u32 == page_count {
        return Err(OxideError::InvalidInput {
            reason: "delete_blank_pages must leave at least one page".to_owned(),
        });
    }
    let kept_pages = (1..=page_count)
        .filter(|page| !blank_pages.contains(page))
        .collect::<Vec<_>>();
    keep_pages(&mut document, &kept_pages)?;
    let bytes = save_pdf(document)?;
    enforce_output_bytes(bytes.len(), limits)?;
    Ok(PdfArtifact { bytes })
}

/// Crops selected PDF pages.
pub fn crop_pdf_pages(input: &[u8], options: &CropPagesOptions) -> Result<PdfArtifact, OxideError> {
    crop_pdf_pages_with_limits(input, options, &ResourceLimits::default())
}

/// Crops selected PDF pages while enforcing resource limits.
pub fn crop_pdf_pages_with_limits(
    input: &[u8],
    options: &CropPagesOptions,
    limits: &ResourceLimits,
) -> Result<PdfArtifact, OxideError> {
    let crop_box = validated_rect(options.left, options.bottom, options.right, options.top)?;
    enforce_input_bytes(input.len(), limits)?;
    let mut document = load_pdf(input)?;
    let page_count = document.get_pages().len() as u32;
    enforce_max_pages(page_count as usize, limits)?;
    let selected_pages = selected_or_all_pages(options.pages.as_deref(), page_count)?;
    let pages = document.get_pages();
    for page_number in selected_pages {
        let page_id = pages
            .get(&page_number)
            .copied()
            .ok_or_else(|| OxideError::InvalidInput {
                reason: format!("page {page_number} is out of range"),
            })?;
        let page = document
            .get_object_mut(page_id)
            .and_then(Object::as_dict_mut)
            .map_err(|_| OxideError::ParsePdf)?;
        page.set("CropBox", crop_box_object(crop_box));
    }
    let bytes = save_pdf(document)?;
    enforce_output_bytes(bytes.len(), limits)?;
    Ok(PdfArtifact { bytes })
}

/// Scales selected PDF pages.
pub fn scale_pdf_pages(
    input: &[u8],
    options: &ScalePagesOptions,
) -> Result<PdfArtifact, OxideError> {
    scale_pdf_pages_with_limits(input, options, &ResourceLimits::default())
}

/// Scales selected PDF pages while enforcing resource limits.
pub fn scale_pdf_pages_with_limits(
    input: &[u8],
    options: &ScalePagesOptions,
    limits: &ResourceLimits,
) -> Result<PdfArtifact, OxideError> {
    if !options.factor.is_finite() || options.factor <= 0.0 {
        return Err(OxideError::InvalidInput {
            reason: "scale factor must be greater than zero".to_owned(),
        });
    }
    enforce_input_bytes(input.len(), limits)?;
    let mut document = load_pdf(input)?;
    let page_count = document.get_pages().len() as u32;
    enforce_max_pages(page_count as usize, limits)?;
    let selected_pages = selected_or_all_pages(options.pages.as_deref(), page_count)?;
    let pages = document.get_pages();
    for page_number in selected_pages {
        let page_id = pages
            .get(&page_number)
            .copied()
            .ok_or_else(|| OxideError::InvalidInput {
                reason: format!("page {page_number} is out of range"),
            })?;
        scale_page_boxes(&mut document, page_id, options.factor)?;
        prepend_page_transform(&mut document, page_id, options.factor)?;
    }
    let bytes = save_pdf(document)?;
    enforce_output_bytes(bytes.len(), limits)?;
    Ok(PdfArtifact { bytes })
}

/// Combines all pages into one tall page.
pub fn pdf_to_single_page(
    input: &[u8],
    options: &SinglePageOptions,
) -> Result<PdfArtifact, OxideError> {
    pdf_to_single_page_with_limits(input, options, &ResourceLimits::default())
}

/// Combines all pages into one tall page while enforcing resource limits.
pub fn pdf_to_single_page_with_limits(
    input: &[u8],
    _options: &SinglePageOptions,
    limits: &ResourceLimits,
) -> Result<PdfArtifact, OxideError> {
    enforce_input_bytes(input.len(), limits)?;
    let mut document = load_pdf(input)?;
    let page_ids = document.get_pages().into_values().collect::<Vec<_>>();
    enforce_max_pages(page_ids.len(), limits)?;
    if page_ids.len() == 1 {
        return Err(OxideError::InvalidInput {
            reason: "single_page requires at least two pages".to_owned(),
        });
    }

    let mut max_width = 0.0f32;
    let mut total_height = 0.0f32;
    let mut page_sizes = Vec::with_capacity(page_ids.len());
    for page_id in &page_ids {
        let (width, height) = page_size(&document, *page_id)?;
        if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
            return Err(OxideError::ParsePdf);
        }
        max_width = max_width.max(width);
        total_height += height;
        page_sizes.push((width, height));
    }

    let first_page = page_ids[0];
    let mut offset = total_height;
    let mut operations = Vec::new();
    let mut merged_resources = Dictionary::new();
    for (page_id, (_width, height)) in page_ids.iter().zip(page_sizes.iter()) {
        offset -= height;
        let content = document
            .get_page_content(*page_id)
            .map_err(|_| OxideError::ParsePdf)?;
        operations.push(lopdf::content::Operation::new("q", vec![]));
        operations.push(lopdf::content::Operation::new(
            "cm",
            vec![
                Object::Real(1.0),
                Object::Real(0.0),
                Object::Real(0.0),
                Object::Real(1.0),
                Object::Real(0.0),
                Object::Real(offset),
            ],
        ));
        operations.extend(
            lopdf::content::Content::decode(&content)
                .map_err(|_| OxideError::ParsePdf)?
                .operations,
        );
        operations.push(lopdf::content::Operation::new("Q", vec![]));
        merge_page_resources_into(&document, *page_id, &mut merged_resources)?;
    }

    let merged_content = lopdf::content::Content { operations }
        .encode()
        .map_err(|_| OxideError::WritePdf)?;
    let content_id = document.add_object(Stream::new(Dictionary::new(), merged_content));
    {
        let page = document
            .get_object_mut(first_page)
            .and_then(Object::as_dict_mut)
            .map_err(|_| OxideError::ParsePdf)?;
        page.set("Contents", Object::Reference(content_id));
        page.set(
            "MediaBox",
            crop_box_object([0.0, 0.0, max_width, total_height]),
        );
        page.set(
            "CropBox",
            crop_box_object([0.0, 0.0, max_width, total_height]),
        );
        page.set("Resources", Object::Dictionary(merged_resources));
    }
    rebuild_pages_tree(&mut document, &[first_page])?;
    let bytes = save_pdf(document)?;
    enforce_output_bytes(bytes.len(), limits)?;
    Ok(PdfArtifact { bytes })
}

/// Lays multiple source pages on each output page.
pub fn nup_pdf_pages(input: &[u8], options: &NUpOptions) -> Result<PdfArtifact, OxideError> {
    nup_pdf_pages_with_limits(input, options, &ResourceLimits::default())
}

/// Lays multiple source pages on each output page while enforcing resource limits.
pub fn nup_pdf_pages_with_limits(
    input: &[u8],
    options: &NUpOptions,
    limits: &ResourceLimits,
) -> Result<PdfArtifact, OxideError> {
    validate_nup_options(options)?;
    enforce_input_bytes(input.len(), limits)?;
    let source = load_pdf(input)?;
    let page_ids = source.get_pages().into_values().collect::<Vec<_>>();
    enforce_max_pages(page_ids.len(), limits)?;
    let layout = page_layout_from_first_page(&source, &page_ids)?;
    let slots_per_page = (options.columns * options.rows) as usize;
    let output_count = page_ids.len().div_ceil(slots_per_page);
    enforce_max_pages(output_count, limits)?;
    let order = (0..page_ids.len()).collect::<Vec<_>>();

    let bytes = impose_pages(
        &source,
        &page_ids,
        &order,
        layout,
        options.columns,
        options.rows,
    )?;
    enforce_output_bytes(bytes.len(), limits)?;
    Ok(PdfArtifact { bytes })
}

/// Arranges pages for booklet printing.
pub fn booklet_pdf_pages(
    input: &[u8],
    options: &BookletOptions,
) -> Result<PdfArtifact, OxideError> {
    booklet_pdf_pages_with_limits(input, options, &ResourceLimits::default())
}

/// Arranges pages for booklet printing while enforcing resource limits.
pub fn booklet_pdf_pages_with_limits(
    input: &[u8],
    _options: &BookletOptions,
    limits: &ResourceLimits,
) -> Result<PdfArtifact, OxideError> {
    enforce_input_bytes(input.len(), limits)?;
    let source = load_pdf(input)?;
    let page_ids = source.get_pages().into_values().collect::<Vec<_>>();
    enforce_max_pages(page_ids.len(), limits)?;
    if page_ids.len() < 2 {
        return Err(OxideError::InvalidInput {
            reason: "booklet requires at least two pages".to_owned(),
        });
    }
    let layout = page_layout_from_first_page(&source, &page_ids)?;
    let sheet_count = page_ids.len().div_ceil(4);
    enforce_max_pages(sheet_count * 2, limits)?;
    let order = booklet_page_order(page_ids.len());

    let bytes = impose_pages(&source, &page_ids, &order, layout, 2, 1)?;
    enforce_output_bytes(bytes.len(), limits)?;
    Ok(PdfArtifact { bytes })
}

/// Adds page numbers to selected PDF pages.
pub fn add_pdf_page_numbers(
    input: &[u8],
    options: &PageNumbersOptions,
) -> Result<PdfArtifact, OxideError> {
    add_pdf_page_numbers_with_limits(input, options, &ResourceLimits::default())
}

/// Adds page numbers to selected PDF pages while enforcing resource limits.
pub fn add_pdf_page_numbers_with_limits(
    input: &[u8],
    options: &PageNumbersOptions,
    limits: &ResourceLimits,
) -> Result<PdfArtifact, OxideError> {
    validate_page_number_options(options)?;
    enforce_input_bytes(input.len(), limits)?;
    let mut document = load_pdf(input)?;
    let page_count = document.get_pages().len() as u32;
    enforce_max_pages(page_count as usize, limits)?;
    let selected_pages = selected_or_all_pages(options.pages.as_deref(), page_count)?;
    add_standard_font_resource(&mut document, &selected_pages, b"OxPnF1".to_vec())?;
    let page_map = document.get_pages();
    for (index, page_number) in selected_pages.iter().enumerate() {
        let page_id = *page_map
            .get(page_number)
            .ok_or_else(|| OxideError::InvalidInput {
                reason: format!("page {page_number} is out of range"),
            })?;
        let (page_width, page_height) = page_size(&document, page_id)?;
        let label = format!(
            "{}{}{}",
            options.prefix,
            options.start + index as u32,
            options.suffix
        );
        let content = page_number_content(
            &label,
            page_width,
            page_height,
            options.font_size,
            options.position,
        )?;
        document
            .add_page_contents(page_id, content)
            .map_err(|_| OxideError::WritePdf)?;
    }
    let bytes = save_pdf(document)?;
    enforce_output_bytes(bytes.len(), limits)?;
    Ok(PdfArtifact { bytes })
}

/// Scans PDF bytes for signature dictionary markers for research prototypes.
///
/// This does not cryptographically verify signatures, certificates, digests,
/// revocation data, timestamp tokens, or PAdES policy. Until the formal
/// signature implementation is added, production workflows must use
/// `PdfSignOptions::Verify`, which returns a structured unsupported status
/// for checks that are not implemented in this verification slice.
pub fn inspect_pdf_signature_markers_for_research(
    input: &[u8],
) -> Result<SignatureResearchReport, OxideError> {
    ensure_pdf_magic(input)?;

    Ok(SignatureResearchReport {
        signature_dictionary_count: count_subslice(input, b"/Type /Sig"),
        byte_ranges: parse_byte_ranges_for_research(input),
        subfilters: parse_name_values_after_token(input, b"/SubFilter"),
    })
}

/// Verifies PDF signature structure and emits a JSON report.
pub fn verify_pdf_signatures(
    input: &[u8],
    options: &SignatureOptions,
    limits: &ResourceLimits,
) -> Result<TextArtifact, OxideError> {
    enforce_input_bytes(input.len(), limits)?;
    let trust_anchor_count = trust_anchor_count(options.trust_anchors.as_deref())?;
    let document = load_pdf(input)?;
    enforce_max_pages(document.get_pages().len(), limits)?;
    let mut diagnostics = Vec::new();
    let signatures = discover_pdf_signature_dictionaries(&document)?
        .into_iter()
        .map(|dictionary| signature_entry_report(input, dictionary))
        .collect::<Vec<_>>();

    if signatures.is_empty() {
        diagnostics.push(signature_diagnostic(
            "no_signatures",
            "PDF contains no signature dictionaries",
        ));
    }

    let verdict = overall_signature_verdict(&signatures, &diagnostics);
    let report = SignatureVerificationReport {
        verdict,
        trust_anchor_count,
        signatures,
        diagnostics,
    };
    let text = serde_json::to_string_pretty(&report).map_err(|_| OxideError::Internal)?;
    enforce_output_bytes(text.len(), limits)?;

    Ok(TextArtifact {
        text,
        diagnostics: Vec::new(),
    })
}

#[derive(Debug, Clone)]
struct DiscoveredSignatureDictionary<'a> {
    field_name: Option<String>,
    dictionary: &'a Dictionary,
}

/// Non-production report returned by the signature research scanner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureResearchReport {
    /// Count of literal `/Type /Sig` markers.
    pub signature_dictionary_count: usize,
    /// Parsed `/ByteRange [...]` arrays found by the scanner.
    pub byte_ranges: Vec<ByteRangeResearch>,
    /// Literal `/SubFilter /Name` values seen in the PDF bytes.
    pub subfilters: Vec<String>,
}

/// Non-production structural summary of a PDF signature ByteRange.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ByteRangeResearch {
    /// First signed range start offset.
    pub first_start: u64,
    /// First signed range length.
    pub first_len: u64,
    /// Second signed range start offset.
    pub second_start: u64,
    /// Second signed range length.
    pub second_len: u64,
    /// Whether both ranges are inside the input byte length.
    pub in_bounds: bool,
    /// Whether the two ranges are non-overlapping and ordered.
    pub ordered_non_overlapping: bool,
    /// Length of the unsigned gap between the two signed ranges, if ordered.
    pub gap_len: Option<u64>,
    /// Total number of bytes covered by the two signed ranges.
    pub covered_len: Option<u64>,
}

/// Converts image artifacts into a PDF with one image per page.
pub fn image_artifacts_to_pdf(
    inputs: &[Artifact],
    options: &ImageToPdfOptions,
    limits: &ResourceLimits,
) -> Result<PdfArtifact, OxideError> {
    if inputs.is_empty() {
        return Err(OxideError::InvalidInput {
            reason: "img2pdf requires at least one image input".to_owned(),
        });
    }
    enforce_max_pages(inputs.len(), limits)?;

    let mut images = Vec::with_capacity(inputs.len());
    let mut total_pixels = 0u64;
    for input in inputs {
        let bytes = image_bytes(input)?;
        enforce_input_bytes(bytes.len(), limits)?;
        let decoded = decode_image(bytes)?;
        let pixels = u64::from(decoded.width) * u64::from(decoded.height);
        total_pixels = total_pixels
            .checked_add(pixels)
            .ok_or_else(|| resource_limit("max_pixels"))?;
        enforce_max_pixels(total_pixels, limits)?;
        images.push(decoded);
    }

    let layout = ImageLayout::from_options(options)?;
    let bytes = write_images_pdf(&images, layout)?;
    enforce_output_bytes(bytes.len(), limits)?;
    Ok(PdfArtifact { bytes })
}

/// Converts an SVG artifact into a PDF. Defaults to vector output.
pub fn svg_to_pdf(
    input: &[u8],
    options: &SvgToPdfOptions,
    limits: &ResourceLimits,
) -> Result<PdfArtifact, OxideError> {
    enforce_input_bytes(input.len(), limits)?;
    let tree = parse_svg(input)?;
    let pixels = svg_pixel_count(&tree)?;
    enforce_max_pixels(pixels, limits)?;
    enforce_max_pages(1, limits)?;

    let bytes = if options.rasterize {
        let image = rasterize_svg(&tree)?;
        write_images_pdf(&[image], ImageLayout::OriginalSize)?
    } else {
        let conversion_options = svg2pdf::ConversionOptions {
            embed_text: false,
            ..svg2pdf::ConversionOptions::default()
        };
        svg2pdf::to_pdf(&tree, conversion_options, svg2pdf::PageOptions::default())
            .map_err(|_| OxideError::WritePdf)?
    };

    enforce_output_bytes(bytes.len(), limits)?;
    Ok(PdfArtifact { bytes })
}

/// Renders a one-based PDF page to PNG bytes.
pub fn render_pdf_page(
    input: &[u8],
    options: &RenderOptions,
    limits: &ResourceLimits,
) -> Result<ImageArtifact, OxideError> {
    enforce_input_bytes(input.len(), limits)?;
    ensure_pdf_magic(input)?;
    let format = options.format.as_deref().unwrap_or("png");
    if format != "png" {
        return Err(OxideError::InvalidInput {
            reason: format!("unsupported render format '{format}'"),
        });
    }
    if options.page == 0 {
        return Err(OxideError::InvalidInput {
            reason: "page number must be one or greater".to_owned(),
        });
    }
    let scale = options.scale.unwrap_or(1.0);
    if !scale.is_finite() || scale <= 0.0 {
        return Err(OxideError::InvalidInput {
            reason: "render scale must be greater than zero".to_owned(),
        });
    }

    let pdf = hayro::hayro_syntax::Pdf::new(input.to_vec()).map_err(|_| OxideError::RenderPdf)?;
    let page_count = pdf.pages().len();
    enforce_max_pages(page_count, limits)?;
    let page_index = usize::try_from(options.page - 1).map_err(|_| OxideError::InvalidInput {
        reason: format!("page {} is out of range 1-{page_count}", options.page),
    })?;
    let page = pdf
        .pages()
        .get(page_index)
        .ok_or_else(|| OxideError::InvalidInput {
            reason: format!("page {} is out of range 1-{page_count}", options.page),
        })?;

    let cache = hayro::RenderCache::new();
    let interpreter_settings = hayro::hayro_interpret::InterpreterSettings::default();
    let render_settings = hayro::RenderSettings {
        x_scale: scale,
        y_scale: scale,
        bg_color: hayro::vello_cpu::color::palette::css::WHITE,
        ..Default::default()
    };
    let pixmap = hayro::render(page, &cache, &interpreter_settings, &render_settings);
    let bytes = pixmap.into_png().map_err(|_| OxideError::RenderPdf)?;
    if bytes.is_empty() {
        return Err(OxideError::RenderPdf);
    }
    enforce_output_bytes(bytes.len(), limits)?;

    Ok(ImageArtifact { bytes })
}

/// Extracts plain text from a PDF and records page-level diagnostics.
pub fn extract_text_from_pdf(
    input: &[u8],
    options: &ExtractTextOptions,
    limits: &ResourceLimits,
) -> Result<TextArtifact, OxideError> {
    enforce_input_bytes(input.len(), limits)?;
    ensure_pdf_magic(input)?;
    let format = options.format.as_deref().unwrap_or("plain");
    if format != "plain" {
        return Err(OxideError::InvalidInput {
            reason: format!("unsupported text extraction format '{format}'"),
        });
    }

    let pages =
        pdf_extract::extract_text_from_mem_by_pages(input).map_err(map_pdf_extract_error)?;
    if pages.is_empty() {
        return Err(OxideError::InvalidInput {
            reason: "PDF contains no pages".to_owned(),
        });
    }
    enforce_max_pages(pages.len(), limits)?;

    let diagnostics = pages
        .iter()
        .enumerate()
        .filter_map(|(index, page)| match page.trim().is_empty() {
            true => Some(TextExtractionDiagnostic {
                page: (index + 1) as u32,
                code: TextExtractionDiagnosticCode::NoTextLayer,
                message: "page has no extractable text layer".to_owned(),
            }),
            false => None,
        })
        .collect::<Vec<_>>();
    if diagnostics.len() == pages.len() {
        return Err(OxideError::InvalidInput {
            reason: "PDF has no extractable text layer".to_owned(),
        });
    }

    let artifact = TextArtifact {
        text: pages.concat(),
        diagnostics,
    };
    enforce_output_bytes(artifact.text.len(), limits)?;
    Ok(artifact)
}

/// Adds a text, image, or SVG watermark to selected PDF pages.
pub fn watermark_pdf_artifacts(
    inputs: &[Artifact],
    options: &WatermarkOptions,
    limits: &ResourceLimits,
) -> Result<PdfArtifact, OxideError> {
    let (pdf_input, watermark_input) = watermark_inputs(inputs, options.kind)?;
    enforce_input_bytes(pdf_input.len(), limits)?;
    let mut document = load_pdf(pdf_input)?;
    let page_count = document.get_pages().len() as u32;
    enforce_max_pages(page_count as usize, limits)?;
    let pages = match options.pages.as_deref() {
        Some(pages) => parse_page_range(pages, page_count)?,
        None => (1..=page_count).collect(),
    };
    let settings = WatermarkSettings::from_options(options)?;

    match options.kind {
        WatermarkKind::Text => {
            let text = options
                .text
                .as_deref()
                .filter(|text| !text.is_empty())
                .ok_or_else(|| OxideError::InvalidInput {
                    reason: "text watermark requires non-empty text".to_owned(),
                })?;
            let font = resolve_watermark_font(options)?;
            append_text_watermark(&mut document, &pages, text, &font, settings)?;
        }
        WatermarkKind::Image => {
            let image = decode_limited_image(
                watermark_input.ok_or_else(|| OxideError::InvalidInput {
                    reason: "image watermark requires an image input".to_owned(),
                })?,
                limits,
            )?;
            append_image_watermark(&mut document, &pages, &image, settings)?;
        }
        WatermarkKind::Svg => {
            let svg = watermark_input.ok_or_else(|| OxideError::InvalidInput {
                reason: "SVG watermark requires an SVG input".to_owned(),
            })?;
            enforce_input_bytes(svg.len(), limits)?;
            let tree = parse_svg(svg)?;
            let pixels = svg_pixel_count(&tree)?;
            enforce_max_pixels(pixels, limits)?;
            if options.rasterize {
                let image = rasterize_svg(&tree)?;
                append_image_watermark(&mut document, &pages, &image, settings)?;
            } else {
                append_svg_watermark(&mut document, &pages, &tree, settings)?;
            }
        }
    }

    let bytes = save_pdf(document)?;
    enforce_output_bytes(bytes.len(), limits)?;
    Ok(PdfArtifact { bytes })
}

/// Validates a workflow and returns a topological execution plan.
pub fn validate_workflow(workflow: &Workflow) -> Result<ExecutionPlan, OxideError> {
    check_resource_limit_entrypoint(&workflow.limits)?;
    let ids = collect_ids(workflow)?;
    validate_task_references(workflow, &ids)?;
    validate_output_references(workflow, &ids)?;
    let task_order = topological_sort(workflow)?;

    Ok(ExecutionPlan { task_order })
}

/// Executes a workflow serially.
pub fn execute_workflow(
    workflow: &Workflow,
    mut store: ArtifactStore,
    runner: &mut impl OperatorRunner,
) -> Result<ExecutionResult, OxideError> {
    let plan = validate_workflow(workflow)?;
    enforce_workflow_input_limits(workflow, &store)?;
    let started_at = Instant::now();
    let timeout = workflow.limits.timeout_ms.map(Duration::from_millis);
    let tasks_by_id = workflow
        .tasks
        .iter()
        .map(|task| (task.id.clone(), task))
        .collect::<BTreeMap<_, _>>();

    for task_id in &plan.task_order {
        enforce_timeout(started_at, timeout)?;
        let task = tasks_by_id.get(task_id).ok_or_else(|| {
            invalid_workflow(format!(
                "task '{}' disappeared after validation",
                task_id.as_str()
            ))
        })?;
        let inputs = task
            .inputs
            .iter()
            .map(|input| {
                store.get(input).cloned().ok_or_else(|| {
                    invalid_workflow(format!(
                        "artifact '{}' is missing at execution time",
                        input.as_str()
                    ))
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let artifact = runner.run(task, &inputs)?;
        enforce_timeout(started_at, timeout)?;
        store.insert(ArtifactRef::new(task.id.as_str()), artifact);
    }

    Ok(ExecutionResult { plan, store })
}

fn enforce_workflow_input_limits(
    workflow: &Workflow,
    store: &ArtifactStore,
) -> Result<(), OxideError> {
    let mut total_input_bytes = 0u64;
    for input in &workflow.inputs {
        let artifact = store.get(&input.id).ok_or_else(|| {
            invalid_workflow(format!(
                "input artifact '{}' is missing at execution time",
                input.id.as_str()
            ))
        })?;
        let size = artifact_size(artifact);
        enforce_input_bytes(size, &workflow.limits)?;
        total_input_bytes = total_input_bytes
            .checked_add(size as u64)
            .ok_or_else(|| resource_limit("max_total_input_bytes"))?;
        if workflow
            .limits
            .max_total_input_bytes
            .is_some_and(|limit| total_input_bytes > limit)
        {
            return Err(resource_limit("max_total_input_bytes"));
        }
    }

    Ok(())
}

fn collect_ids(workflow: &Workflow) -> Result<BTreeSet<ArtifactRef>, OxideError> {
    let mut ids = BTreeSet::new();
    for input in &workflow.inputs {
        insert_unique_id(&mut ids, &input.id)?;
    }
    for task in &workflow.tasks {
        insert_unique_id(&mut ids, &ArtifactRef::new(task.id.as_str()))?;
    }
    for output in &workflow.outputs {
        insert_unique_id(&mut ids, &output.id)?;
    }

    Ok(ids)
}

fn insert_unique_id(ids: &mut BTreeSet<ArtifactRef>, id: &ArtifactRef) -> Result<(), OxideError> {
    if id.as_str().is_empty() {
        return Err(invalid_workflow("artifact id must not be empty"));
    }
    if !ids.insert(id.clone()) {
        return Err(invalid_workflow(format!(
            "duplicate artifact id '{}'",
            id.as_str()
        )));
    }

    Ok(())
}

fn validate_task_references(
    workflow: &Workflow,
    ids: &BTreeSet<ArtifactRef>,
) -> Result<(), OxideError> {
    for task in &workflow.tasks {
        if task.inputs.is_empty() {
            return Err(invalid_workflow(format!(
                "task '{}' must declare at least one input",
                task.id.as_str()
            )));
        }
        for input in &task.inputs {
            if !ids.contains(input) {
                return Err(invalid_workflow(format!(
                    "task '{}' references missing artifact '{}'",
                    task.id.as_str(),
                    input.as_str()
                )));
            }
        }
    }

    Ok(())
}

fn validate_output_references(
    workflow: &Workflow,
    ids: &BTreeSet<ArtifactRef>,
) -> Result<(), OxideError> {
    for output in &workflow.outputs {
        if !ids.contains(&output.from) {
            return Err(invalid_workflow(format!(
                "output '{}' references missing artifact '{}'",
                output.id.as_str(),
                output.from.as_str()
            )));
        }
    }

    Ok(())
}

fn topological_sort(workflow: &Workflow) -> Result<Vec<TaskId>, OxideError> {
    let task_ids = workflow
        .tasks
        .iter()
        .map(|task| task.id.clone())
        .collect::<BTreeSet<_>>();
    let mut incoming_count = workflow
        .tasks
        .iter()
        .map(|task| (task.id.clone(), 0usize))
        .collect::<BTreeMap<_, _>>();
    let mut dependents = BTreeMap::<TaskId, Vec<TaskId>>::new();

    for task in &workflow.tasks {
        for input in &task.inputs {
            let dependency = TaskId::new(input.as_str());
            if task_ids.contains(&dependency) {
                *incoming_count.get_mut(&task.id).ok_or_else(|| {
                    invalid_workflow(format!("task '{}' is missing", task.id.as_str()))
                })? += 1;
                dependents
                    .entry(dependency)
                    .or_default()
                    .push(task.id.clone());
            }
        }
    }

    let mut ready = incoming_count
        .iter()
        .filter_map(|(id, count)| (*count == 0).then_some(id.clone()))
        .collect::<VecDeque<_>>();
    let mut task_order = Vec::with_capacity(workflow.tasks.len());

    while let Some(task_id) = ready.pop_front() {
        task_order.push(task_id.clone());
        if let Some(children) = dependents.get(&task_id) {
            for child in children {
                let child_count = incoming_count.get_mut(child).ok_or_else(|| {
                    invalid_workflow(format!("task '{}' is missing", child.as_str()))
                })?;
                *child_count -= 1;
                if *child_count == 0 {
                    ready.push_back(child.clone());
                }
            }
        }
    }

    if task_order.len() != workflow.tasks.len() {
        return Err(invalid_workflow("workflow task graph contains a cycle"));
    }

    Ok(task_order)
}

fn check_resource_limit_entrypoint(limits: &ResourceLimits) -> Result<(), OxideError> {
    let numeric_limits = [
        limits.max_input_bytes,
        limits.max_total_input_bytes,
        limits.max_pixels,
        limits.max_output_bytes,
        limits.timeout_ms,
    ];

    if numeric_limits.into_iter().flatten().any(|limit| limit == 0) || limits.max_pages == Some(0) {
        return Err(OxideError::ResourceLimitExceeded {
            limit: "resource limit must be greater than zero".to_owned(),
        });
    }

    Ok(())
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

fn watermark_inputs(
    inputs: &[Artifact],
    kind: WatermarkKind,
) -> Result<(&[u8], Option<&[u8]>), OxideError> {
    match kind {
        WatermarkKind::Text => {
            if inputs.len() != 1 {
                return Err(OxideError::InvalidInput {
                    reason: "text watermark requires exactly one PDF input".to_owned(),
                });
            }
            Ok((pdf_bytes(&inputs[0])?, None))
        }
        WatermarkKind::Image | WatermarkKind::Svg => {
            if inputs.len() != 2 {
                return Err(OxideError::InvalidInput {
                    reason: "image and SVG watermarks require PDF input and watermark input"
                        .to_owned(),
                });
            }
            let pdf = pdf_bytes(&inputs[0])?;
            let watermark = match kind {
                WatermarkKind::Image => image_bytes(&inputs[1])?,
                WatermarkKind::Svg => svg_bytes(&inputs[1])?,
                WatermarkKind::Text => unreachable!(),
            };
            Ok((pdf, Some(watermark)))
        }
    }
}

fn pdf_bytes(artifact: &Artifact) -> Result<&[u8], OxideError> {
    match artifact {
        Artifact::Pdf(pdf) => Ok(&pdf.bytes),
        Artifact::Bytes(bytes) => Ok(&bytes.bytes),
        _ => Err(OxideError::InvalidInput {
            reason: "expected PDF input artifact".to_owned(),
        }),
    }
}

fn image_bytes(artifact: &Artifact) -> Result<&[u8], OxideError> {
    match artifact {
        Artifact::Image(image) => Ok(&image.bytes),
        Artifact::Bytes(bytes) => Ok(&bytes.bytes),
        _ => Err(OxideError::InvalidInput {
            reason: "expected image input artifact".to_owned(),
        }),
    }
}

fn svg_bytes(artifact: &Artifact) -> Result<&[u8], OxideError> {
    match artifact {
        Artifact::Svg(svg) => Ok(&svg.bytes),
        Artifact::Bytes(bytes) => Ok(&bytes.bytes),
        _ => Err(OxideError::InvalidInput {
            reason: "expected SVG input artifact".to_owned(),
        }),
    }
}

fn trust_anchor_count(path: Option<&std::path::Path>) -> Result<usize, OxideError> {
    let path = path.ok_or_else(|| OxideError::InvalidInput {
        reason: "signature verification requires explicit trust anchors".to_owned(),
    })?;
    let pem = std::fs::read(path).map_err(|_| OxideError::Io)?;
    let pem = std::str::from_utf8(&pem).map_err(|_| OxideError::InvalidInput {
        reason: "trust anchors file contains no valid PEM certificates".to_owned(),
    })?;
    let count = parsed_trust_anchor_count(pem)?;
    if count == 0 {
        return Err(OxideError::InvalidInput {
            reason: "trust anchors file contains no valid PEM certificates".to_owned(),
        });
    }

    Ok(count)
}

fn parsed_trust_anchor_count(pem: &str) -> Result<usize, OxideError> {
    const BEGIN: &str = "-----BEGIN CERTIFICATE-----";
    const END: &str = "-----END CERTIFICATE-----";

    let mut rest = pem;
    let mut count = 0usize;
    while let Some(begin) = rest.find(BEGIN) {
        rest = &rest[begin..];
        let Some(end) = rest.find(END) else {
            return Err(OxideError::InvalidInput {
                reason: "trust anchors file contains no valid PEM certificates".to_owned(),
            });
        };
        let block_end = end + END.len();
        let block = &rest[..block_end];
        let (label, der) =
            pem_rfc7468::decode_vec(block.as_bytes()).map_err(|_| OxideError::InvalidInput {
                reason: "trust anchors file contains no valid PEM certificates".to_owned(),
            })?;
        if label != "CERTIFICATE" {
            return Err(OxideError::InvalidInput {
                reason: "trust anchors file contains no valid PEM certificates".to_owned(),
            });
        }
        Certificate::from_der(&der).map_err(|_| OxideError::InvalidInput {
            reason: "trust anchors file contains no valid PEM certificates".to_owned(),
        })?;
        count += 1;
        rest = &rest[block_end..];
    }

    Ok(count)
}

fn discover_pdf_signature_dictionaries(
    document: &lopdf::Document,
) -> Result<Vec<DiscoveredSignatureDictionary<'_>>, OxideError> {
    let mut signatures = Vec::new();
    if let Ok(catalog) = document.catalog() {
        if let Ok(acroform) = catalog
            .get(b"AcroForm")
            .and_then(|object| deref_dictionary(document, object))
        {
            if let Ok(fields) = acroform.get(b"Fields").and_then(lopdf::Object::as_array) {
                for field in fields {
                    collect_signature_fields(document, field, None, &mut signatures)?;
                }
            }
        }
    }
    for (_, page_id) in document.get_pages() {
        let Ok(page) = document
            .get_object(page_id)
            .and_then(lopdf::Object::as_dict)
        else {
            continue;
        };
        let Ok(annots) = page.get(b"Annots").and_then(lopdf::Object::as_array) else {
            continue;
        };
        for annot in annots {
            collect_signature_fields(document, annot, None, &mut signatures)?;
        }
    }
    signatures.dedup_by_key(|signature| std::ptr::from_ref(signature.dictionary) as usize);

    Ok(signatures)
}

fn collect_signature_fields<'a>(
    document: &'a lopdf::Document,
    object: &'a lopdf::Object,
    inherited_name: Option<String>,
    signatures: &mut Vec<DiscoveredSignatureDictionary<'a>>,
) -> Result<(), OxideError> {
    let dictionary = deref_dictionary(document, object).map_err(|_| OxideError::ParsePdf)?;
    let field_name = dictionary
        .get(b"T")
        .ok()
        .and_then(pdf_string)
        .or(inherited_name);
    if dictionary.get(b"FT").and_then(lopdf::Object::as_name).ok() == Some(b"Sig") {
        if let Ok(value) = dictionary.get(b"V") {
            if let Ok(signature_dictionary) = deref_dictionary(document, value) {
                signatures.push(DiscoveredSignatureDictionary {
                    field_name: field_name.clone(),
                    dictionary: signature_dictionary,
                });
            }
        } else if dictionary.get(b"ByteRange").is_ok() {
            signatures.push(DiscoveredSignatureDictionary {
                field_name: field_name.clone(),
                dictionary,
            });
        }
    } else if dictionary.get(b"ByteRange").is_ok() && dictionary.get(b"Contents").is_ok() {
        signatures.push(DiscoveredSignatureDictionary {
            field_name: field_name.clone(),
            dictionary,
        });
    }
    if let Ok(kids) = dictionary.get(b"Kids").and_then(lopdf::Object::as_array) {
        for kid in kids {
            collect_signature_fields(document, kid, field_name.clone(), signatures)?;
        }
    }

    Ok(())
}

fn deref_dictionary<'a>(
    document: &'a lopdf::Document,
    object: &'a lopdf::Object,
) -> lopdf::Result<&'a Dictionary> {
    match object {
        lopdf::Object::Reference(id) => document.get_object(*id).and_then(lopdf::Object::as_dict),
        lopdf::Object::Dictionary(dictionary) => Ok(dictionary),
        _ => object.as_dict(),
    }
}

fn signature_entry_report(
    input: &[u8],
    discovered: DiscoveredSignatureDictionary<'_>,
) -> SignatureEntryReport {
    let mut diagnostics = Vec::new();
    let subfilter = discovered
        .dictionary
        .get(b"SubFilter")
        .ok()
        .and_then(pdf_name);
    if !matches!(
        subfilter.as_deref(),
        Some("adbe.pkcs7.detached") | Some("ETSI.CAdES.detached")
    ) {
        diagnostics.push(signature_diagnostic(
            "unsupported_subfilter",
            "signature SubFilter is not supported",
        ));
    }

    let byte_range = byte_range_verification(input, discovered.dictionary, &mut diagnostics);
    let contents = contents_verification(discovered.dictionary, &byte_range, &mut diagnostics);

    SignatureEntryReport {
        field_name: discovered.field_name,
        subfilter,
        byte_range,
        contents,
        cms_status: signature_check(
            SignatureCheckState::Unsupported,
            "CMS SignedData parsing is not implemented in this verification slice",
        ),
        digest_status: signature_check(
            SignatureCheckState::Indeterminate,
            "signed byte digest is not checked until CMS messageDigest parsing is available",
        ),
        signature_status: signature_check(
            SignatureCheckState::Unsupported,
            "signer signature mathematics is not implemented in this verification slice",
        ),
        certificate_chain_status: signature_check(
            SignatureCheckState::Indeterminate,
            "certificate chain cannot be trusted until signer certificate extraction is available",
        ),
        revocation_status: signature_check(
            SignatureCheckState::Indeterminate,
            "offline revocation status is not confirmed; no network lookup is performed",
        ),
        timestamp_status: signature_check(
            SignatureCheckState::Unsupported,
            "timestamp token validation is not implemented in this verification slice",
        ),
        diagnostics,
    }
}

fn byte_range_verification(
    input: &[u8],
    dictionary: &Dictionary,
    diagnostics: &mut Vec<SignatureDiagnostic>,
) -> ByteRangeVerification {
    let Some(values) = dictionary
        .get(b"ByteRange")
        .ok()
        .and_then(byte_range_values)
    else {
        diagnostics.push(signature_diagnostic(
            "missing_byte_range",
            "signature dictionary is missing ByteRange",
        ));
        return ByteRangeVerification {
            values: None,
            in_bounds: false,
            ordered_non_overlapping: false,
            gap_len: None,
            covered_len: None,
        };
    };
    let research = byte_range_research(
        values[0],
        values[1],
        values[2],
        values[3],
        input.len() as u64,
    );
    if !research.in_bounds {
        diagnostics.push(signature_diagnostic(
            "byte_range_out_of_bounds",
            "ByteRange references bytes outside the input",
        ));
    }
    if !research.ordered_non_overlapping {
        diagnostics.push(signature_diagnostic(
            "byte_range_not_ordered",
            "ByteRange entries are not ordered and non-overlapping",
        ));
    }

    ByteRangeVerification {
        values: Some(values),
        in_bounds: research.in_bounds,
        ordered_non_overlapping: research.ordered_non_overlapping,
        gap_len: research.gap_len,
        covered_len: research.covered_len,
    }
}

fn contents_verification(
    dictionary: &Dictionary,
    byte_range: &ByteRangeVerification,
    diagnostics: &mut Vec<SignatureDiagnostic>,
) -> ContentsVerification {
    let Some(byte_len) = dictionary
        .get(b"Contents")
        .ok()
        .and_then(pdf_string_bytes_len)
    else {
        diagnostics.push(signature_diagnostic(
            "missing_contents",
            "signature dictionary is missing Contents",
        ));
        return ContentsVerification {
            byte_len: None,
            covered_by_gap: false,
        };
    };
    let covered_by_gap = byte_range
        .gap_len
        .is_some_and(|gap_len| gap_len >= byte_len as u64);
    if !covered_by_gap {
        diagnostics.push(signature_diagnostic(
            "contents_not_covered_by_gap",
            "signature Contents is larger than the unsigned ByteRange gap",
        ));
    }

    ContentsVerification {
        byte_len: Some(byte_len),
        covered_by_gap,
    }
}

fn byte_range_values(object: &lopdf::Object) -> Option<[u64; 4]> {
    let array = object.as_array().ok()?;
    if array.len() != 4 {
        return None;
    }
    let mut values = [0u64; 4];
    for (index, object) in array.iter().enumerate() {
        let value = object.as_i64().ok()?;
        values[index] = u64::try_from(value).ok()?;
    }

    Some(values)
}

fn pdf_name(object: &lopdf::Object) -> Option<String> {
    object
        .as_name()
        .ok()
        .map(|name| String::from_utf8_lossy(name).into_owned())
}

fn pdf_string(object: &lopdf::Object) -> Option<String> {
    object
        .as_str()
        .ok()
        .map(|value| String::from_utf8_lossy(value).into_owned())
}

fn pdf_string_bytes_len(object: &lopdf::Object) -> Option<usize> {
    object.as_str().ok().map(<[u8]>::len)
}

fn signature_check(status: SignatureCheckState, detail: impl Into<String>) -> SignatureCheckStatus {
    SignatureCheckStatus {
        status,
        detail: detail.into(),
    }
}

fn signature_diagnostic(
    code: impl Into<String>,
    message: impl Into<String>,
) -> SignatureDiagnostic {
    SignatureDiagnostic {
        code: code.into(),
        message: message.into(),
    }
}

fn overall_signature_verdict(
    signatures: &[SignatureEntryReport],
    diagnostics: &[SignatureDiagnostic],
) -> SignatureVerdict {
    if signatures.is_empty()
        || diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "no_signatures")
    {
        return SignatureVerdict::Indeterminate;
    }
    if signatures.iter().any(|signature| {
        signature
            .diagnostics
            .iter()
            .any(is_invalid_signature_diagnostic)
    }) {
        return SignatureVerdict::Invalid;
    }
    if signatures.iter().any(|signature| {
        signature.cms_status.status == SignatureCheckState::Unsupported
            || signature.signature_status.status == SignatureCheckState::Unsupported
            || signature.timestamp_status.status == SignatureCheckState::Unsupported
    }) {
        return SignatureVerdict::Unsupported;
    }

    SignatureVerdict::Indeterminate
}

fn is_invalid_signature_diagnostic(diagnostic: &SignatureDiagnostic) -> bool {
    matches!(
        diagnostic.code.as_str(),
        "missing_byte_range"
            | "byte_range_out_of_bounds"
            | "byte_range_not_ordered"
            | "missing_contents"
            | "contents_not_covered_by_gap"
    )
}

fn load_pdf(input: &[u8]) -> Result<lopdf::Document, OxideError> {
    ensure_pdf_magic(input)?;
    let document = lopdf::Document::load_mem(input).map_err(map_lopdf_read_error)?;
    if document.is_encrypted() {
        return Err(OxideError::EncryptedPdf);
    }
    if document.get_pages().is_empty() {
        return Err(OxideError::InvalidInput {
            reason: "PDF contains no pages".to_owned(),
        });
    }

    Ok(document)
}

fn ensure_pdf_magic(input: &[u8]) -> Result<(), OxideError> {
    if input.starts_with(b"%PDF-") {
        return Ok(());
    }

    Err(OxideError::InvalidInput {
        reason: "expected PDF input magic bytes".to_owned(),
    })
}

fn count_subslice(haystack: &[u8], needle: &[u8]) -> usize {
    if needle.is_empty() {
        return 0;
    }

    haystack
        .windows(needle.len())
        .filter(|window| *window == needle)
        .count()
}

fn parse_byte_ranges_for_research(input: &[u8]) -> Vec<ByteRangeResearch> {
    let mut ranges = Vec::new();
    let mut offset = 0usize;
    while let Some(relative) = find_subslice(&input[offset..], b"/ByteRange") {
        let token_start = offset + relative;
        offset = token_start + b"/ByteRange".len();
        let Some(open_relative) = input[offset..].iter().position(|byte| *byte == b'[') else {
            continue;
        };
        let array_start = offset + open_relative + 1;
        let Some(close_relative) = input[array_start..].iter().position(|byte| *byte == b']')
        else {
            continue;
        };
        let array_end = array_start + close_relative;
        offset = array_end + 1;
        let numbers = input[array_start..array_end]
            .split(|byte| byte.is_ascii_whitespace())
            .filter(|part| !part.is_empty())
            .filter_map(parse_ascii_u64)
            .collect::<Vec<_>>();
        if numbers.len() < 4 {
            continue;
        }
        ranges.push(byte_range_research(
            numbers[0],
            numbers[1],
            numbers[2],
            numbers[3],
            input.len() as u64,
        ));
    }

    ranges
}

fn byte_range_research(
    first_start: u64,
    first_len: u64,
    second_start: u64,
    second_len: u64,
    input_len: u64,
) -> ByteRangeResearch {
    let first_end = first_start.checked_add(first_len);
    let second_end = second_start.checked_add(second_len);
    let in_bounds = first_end.is_some_and(|end| end <= input_len)
        && second_end.is_some_and(|end| end <= input_len);
    let ordered_non_overlapping = first_end.is_some_and(|end| end <= second_start);
    let gap_len = ordered_non_overlapping.then(|| second_start - first_end.unwrap_or(second_start));
    let covered_len = first_len.checked_add(second_len);

    ByteRangeResearch {
        first_start,
        first_len,
        second_start,
        second_len,
        in_bounds,
        ordered_non_overlapping,
        gap_len,
        covered_len,
    }
}

fn parse_name_values_after_token(input: &[u8], token: &[u8]) -> Vec<String> {
    let mut values = Vec::new();
    let mut offset = 0usize;
    while let Some(relative) = find_subslice(&input[offset..], token) {
        let value_start = offset + relative + token.len();
        offset = value_start;
        let Some(name_start_relative) = input[value_start..].iter().position(|byte| *byte == b'/')
        else {
            continue;
        };
        let name_start = value_start + name_start_relative + 1;
        let name_end = input[name_start..]
            .iter()
            .position(|byte| is_pdf_delimiter_or_whitespace(*byte))
            .map_or(input.len(), |end| name_start + end);
        if name_end > name_start {
            values.push(String::from_utf8_lossy(&input[name_start..name_end]).into_owned());
        }
        offset = name_end;
    }

    values
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn parse_ascii_u64(input: &[u8]) -> Option<u64> {
    if input.is_empty() || input.iter().any(|byte| !byte.is_ascii_digit()) {
        return None;
    }

    input.iter().try_fold(0u64, |acc, byte| {
        acc.checked_mul(10)?.checked_add(u64::from(byte - b'0'))
    })
}

fn is_pdf_delimiter_or_whitespace(byte: u8) -> bool {
    byte.is_ascii_whitespace() || matches!(byte, b'/' | b'<' | b'>' | b'[' | b']' | b'(' | b')')
}

fn save_pdf(mut document: lopdf::Document) -> Result<Vec<u8>, OxideError> {
    let mut output = Vec::new();
    document.prune_objects();
    document.renumber_objects();
    document
        .save_to(&mut output)
        .map_err(|_| OxideError::WritePdf)?;

    Ok(output)
}

fn map_lopdf_read_error(error: lopdf::Error) -> OxideError {
    match error {
        lopdf::Error::Decryption(_) | lopdf::Error::UnsupportedSecurityHandler(_) => {
            OxideError::EncryptedPdf
        }
        _ => OxideError::ParsePdf,
    }
}

fn map_pdf_extract_error(error: pdf_extract::OutputError) -> OxideError {
    let message = error.to_string().to_ascii_lowercase();
    if message.contains("encrypted")
        || message.contains("decryption")
        || message.contains("incorrect password")
        || message.contains("security handler")
    {
        OxideError::EncryptedPdf
    } else {
        OxideError::ExtractText
    }
}

fn parse_page_range(pages: &str, page_count: u32) -> Result<Vec<u32>, OxideError> {
    if pages.trim().is_empty() {
        return Err(OxideError::InvalidInput {
            reason: "page range must not be empty".to_owned(),
        });
    }

    let mut selected = Vec::new();
    for part in pages.split(',') {
        let part = part.trim();
        if part.is_empty() {
            return Err(OxideError::InvalidInput {
                reason: "page range contains an empty item".to_owned(),
            });
        }

        if let Some((start, end)) = part.split_once('-') {
            let start = parse_page_number(start.trim(), page_count)?;
            let end = parse_page_number(end.trim(), page_count)?;
            if start > end {
                return Err(OxideError::InvalidInput {
                    reason: format!("page range '{part}' must be ascending"),
                });
            }
            selected.extend(start..=end);
        } else {
            selected.push(parse_page_number(part, page_count)?);
        }
    }
    let unique_pages = selected.iter().copied().collect::<BTreeSet<_>>();
    if unique_pages.len() != selected.len() {
        return Err(OxideError::InvalidInput {
            reason: "page range must not contain duplicate pages".to_owned(),
        });
    }

    Ok(selected)
}

fn parse_page_number(value: &str, page_count: u32) -> Result<u32, OxideError> {
    let page = value.parse::<u32>().map_err(|_| OxideError::InvalidInput {
        reason: format!("invalid page number '{value}'"),
    })?;
    if page == 0 || page > page_count {
        return Err(OxideError::InvalidInput {
            reason: format!("page {page} is out of range 1-{page_count}"),
        });
    }

    Ok(page)
}

fn selected_or_all_pages(pages: Option<&str>, page_count: u32) -> Result<Vec<u32>, OxideError> {
    match pages {
        Some(pages) => parse_page_range(pages, page_count),
        None => Ok((1..=page_count).collect()),
    }
}

fn validated_rect(left: f32, bottom: f32, right: f32, top: f32) -> Result<[f32; 4], OxideError> {
    if [left, bottom, right, top]
        .into_iter()
        .any(|value| !value.is_finite())
    {
        return Err(OxideError::InvalidInput {
            reason: "page box coordinates must be finite".to_owned(),
        });
    }
    if left >= right || bottom >= top {
        return Err(OxideError::InvalidInput {
            reason: "page box coordinates must satisfy left < right and bottom < top".to_owned(),
        });
    }

    Ok([left, bottom, right, top])
}

fn crop_box_object(rect: [f32; 4]) -> Object {
    Object::Array(rect.into_iter().map(Object::Real).collect())
}

fn normalize_rotation(degrees: i16) -> Result<i16, OxideError> {
    match degrees.rem_euclid(360) {
        90 => Ok(90),
        180 => Ok(180),
        270 => Ok(270),
        _ => Err(OxideError::InvalidInput {
            reason: "rotation must be 90, 180, or 270 degrees".to_owned(),
        }),
    }
}

fn keep_pages(document: &mut lopdf::Document, selected_pages: &[u32]) -> Result<(), OxideError> {
    let page_count = document.get_pages().len() as u32;
    if selected_pages.is_empty() {
        return Err(OxideError::InvalidInput {
            reason: "at least one page must be selected".to_owned(),
        });
    }
    let pages_before_delete = document.get_pages();
    let selected_page_ids = selected_pages
        .iter()
        .map(|page| {
            pages_before_delete
                .get(page)
                .copied()
                .ok_or_else(|| OxideError::InvalidInput {
                    reason: format!("page {page} is out of range"),
                })
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mut delete_pages = (1..=page_count)
        .filter(|page| !selected_pages.contains(page))
        .collect::<Vec<_>>();
    delete_pages.sort_unstable_by(|left, right| right.cmp(left));
    document.delete_pages(&delete_pages);
    rebuild_pages_tree(document, &selected_page_ids)
}

fn merge_documents(documents: Vec<lopdf::Document>) -> Result<Vec<u8>, OxideError> {
    let mut next_id = 1;
    let mut merged = lopdf::Document::with_version("1.7");
    let mut document_pages = BTreeMap::new();
    let mut document_objects = BTreeMap::new();

    for mut document in documents {
        document.renumber_objects_with(next_id);
        next_id = document.max_id + 1;

        for page_id in document.get_pages().into_values() {
            let page = document
                .get_object(page_id)
                .cloned()
                .map_err(|_| OxideError::ParsePdf)?;
            document_pages.insert(page_id, page);
        }
        document_objects.extend(document.objects);
    }

    let mut catalog_object = None;
    let mut pages_object = None;
    for (object_id, object) in document_objects {
        match object.type_name().unwrap_or(b"") {
            b"Catalog" => {
                if catalog_object.is_none() {
                    catalog_object = Some((object_id, object));
                }
            }
            b"Pages" => {
                if pages_object.is_none() {
                    pages_object = Some((object_id, object));
                }
            }
            b"Page" | b"Outlines" | b"Outline" => {}
            _ => {
                merged.objects.insert(object_id, object);
            }
        }
    }

    let (pages_id, pages_object) = pages_object.ok_or(OxideError::ParsePdf)?;
    for (page_id, page) in &document_pages {
        let dictionary = page.as_dict().map_err(|_| OxideError::ParsePdf)?;
        let mut dictionary = dictionary.clone();
        dictionary.set("Parent", pages_id);
        merged
            .objects
            .insert(*page_id, lopdf::Object::Dictionary(dictionary));
    }

    let mut pages_dictionary = pages_object
        .as_dict()
        .map_err(|_| OxideError::ParsePdf)?
        .clone();
    pages_dictionary.set("Count", document_pages.len() as u32);
    pages_dictionary.set(
        "Kids",
        document_pages
            .keys()
            .copied()
            .map(lopdf::Object::Reference)
            .collect::<Vec<_>>(),
    );
    merged
        .objects
        .insert(pages_id, lopdf::Object::Dictionary(pages_dictionary));

    let (catalog_id, catalog_object) = catalog_object.ok_or(OxideError::ParsePdf)?;
    let mut catalog_dictionary = catalog_object
        .as_dict()
        .map_err(|_| OxideError::ParsePdf)?
        .clone();
    catalog_dictionary.set("Pages", pages_id);
    catalog_dictionary.remove(b"Outlines");
    merged
        .objects
        .insert(catalog_id, lopdf::Object::Dictionary(catalog_dictionary));
    merged.trailer.set("Root", catalog_id);
    merged.max_id = merged
        .objects
        .keys()
        .map(|(id, _)| *id)
        .max()
        .unwrap_or_default();

    save_pdf(merged)
}

#[derive(Debug, Clone)]
struct DecodedImage {
    width: u32,
    height: u32,
    rgb: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ImageLayout {
    Fit,
    OriginalSize,
}

impl ImageLayout {
    fn from_options(options: &ImageToPdfOptions) -> Result<Self, OxideError> {
        match options.layout.as_deref().unwrap_or("fit") {
            "fit" => Ok(Self::Fit),
            "original_size" => Ok(Self::OriginalSize),
            other => Err(OxideError::InvalidInput {
                reason: format!("unsupported image layout '{other}'"),
            }),
        }
    }
}

fn decode_image(input: &[u8]) -> Result<DecodedImage, OxideError> {
    let format = image::guess_format(input).map_err(|_| OxideError::ImageDecode)?;
    match format {
        image::ImageFormat::Jpeg | image::ImageFormat::Png | image::ImageFormat::WebP => {}
        _ => return Err(OxideError::ImageDecode),
    }
    let image = image::load_from_memory_with_format(input, format)
        .map_err(|_| OxideError::ImageDecode)?
        .to_rgb8();

    Ok(DecodedImage {
        width: image.width(),
        height: image.height(),
        rgb: image.into_raw(),
    })
}

fn decode_limited_image(input: &[u8], limits: &ResourceLimits) -> Result<DecodedImage, OxideError> {
    enforce_input_bytes(input.len(), limits)?;
    let decoded = decode_image(input)?;
    let pixels = u64::from(decoded.width) * u64::from(decoded.height);
    enforce_max_pixels(pixels, limits)?;
    Ok(decoded)
}

fn write_images_pdf(images: &[DecodedImage], layout: ImageLayout) -> Result<Vec<u8>, OxideError> {
    let mut next_ref = 1;
    let mut alloc_ref = || {
        let reference = pdf_writer::Ref::new(next_ref);
        next_ref += 1;
        reference
    };
    let catalog_id = alloc_ref();
    let pages_id = alloc_ref();
    let page_ids = (0..images.len()).map(|_| alloc_ref()).collect::<Vec<_>>();
    let image_ids = (0..images.len()).map(|_| alloc_ref()).collect::<Vec<_>>();
    let content_ids = (0..images.len()).map(|_| alloc_ref()).collect::<Vec<_>>();

    let mut pdf = pdf_writer::Pdf::new();
    pdf.catalog(catalog_id).pages(pages_id);
    pdf.pages(pages_id)
        .kids(page_ids.iter().copied())
        .count(images.len() as i32);

    for (((page_id, image_id), content_id), image) in page_ids
        .iter()
        .zip(image_ids.iter())
        .zip(content_ids.iter())
        .zip(images.iter())
    {
        let image_name = pdf_writer::Name(b"Im1");
        let (page_width, page_height, image_width, image_height, x, y) =
            image_placement(image, layout);

        let mut page = pdf.page(*page_id);
        page.media_box(pdf_writer::Rect::new(0.0, 0.0, page_width, page_height));
        page.parent(pages_id);
        page.contents(*content_id);
        page.resources().x_objects().pair(image_name, *image_id);
        page.finish();

        let mut image_object = pdf.image_xobject(*image_id, &image.rgb);
        image_object.width(image.width as i32);
        image_object.height(image.height as i32);
        image_object.color_space().device_rgb();
        image_object.bits_per_component(8);
        image_object.finish();

        let mut content = pdf_writer::Content::new();
        content.save_state();
        content.transform([image_width, 0.0, 0.0, image_height, x, y]);
        content.x_object(image_name);
        content.restore_state();
        pdf.stream(*content_id, &content.finish());
    }

    Ok(pdf.finish())
}

fn image_placement(image: &DecodedImage, layout: ImageLayout) -> (f32, f32, f32, f32, f32, f32) {
    let original_width = image.width as f32;
    let original_height = image.height as f32;
    match layout {
        ImageLayout::OriginalSize => (
            original_width,
            original_height,
            original_width,
            original_height,
            0.0,
            0.0,
        ),
        ImageLayout::Fit => {
            let scale = (A4_WIDTH / original_width)
                .min(A4_HEIGHT / original_height)
                .min(1.0);
            let image_width = original_width * scale;
            let image_height = original_height * scale;
            let x = (A4_WIDTH - image_width) / 2.0;
            let y = (A4_HEIGHT - image_height) / 2.0;
            (A4_WIDTH, A4_HEIGHT, image_width, image_height, x, y)
        }
    }
}

fn parse_svg(input: &[u8]) -> Result<svg2pdf::usvg::Tree, OxideError> {
    ensure_svg_magic(input)?;
    let options = svg2pdf::usvg::Options::default();
    svg2pdf::usvg::Tree::from_data(input, &options).map_err(|_| OxideError::SvgParse)
}

fn ensure_svg_magic(input: &[u8]) -> Result<(), OxideError> {
    let input = input
        .strip_prefix(&[0xEF, 0xBB, 0xBF])
        .unwrap_or(input)
        .trim_ascii_start();
    if input.starts_with(b"<svg") || input.starts_with(b"<?xml") {
        return Ok(());
    }

    Err(OxideError::SvgParse)
}

fn svg_pixel_count(tree: &svg2pdf::usvg::Tree) -> Result<u64, OxideError> {
    let size = tree.size().to_int_size();
    Ok(u64::from(size.width()) * u64::from(size.height()))
}

fn rasterize_svg(tree: &svg2pdf::usvg::Tree) -> Result<DecodedImage, OxideError> {
    let size = tree.size().to_int_size();
    let mut pixmap =
        resvg::tiny_skia::Pixmap::new(size.width(), size.height()).ok_or(OxideError::RenderPdf)?;
    resvg::render(
        tree,
        resvg::tiny_skia::Transform::identity(),
        &mut pixmap.as_mut(),
    );
    let mut rgb =
        Vec::with_capacity((u64::from(size.width()) * u64::from(size.height()) * 3) as usize);
    for pixel in pixmap.data().chunks_exact(4) {
        rgb.extend_from_slice(&[pixel[0], pixel[1], pixel[2]]);
    }

    Ok(DecodedImage {
        width: size.width(),
        height: size.height(),
        rgb,
    })
}

#[derive(Debug, Clone, Copy)]
struct WatermarkSettings {
    opacity: f32,
    rotation_degrees: f32,
    position: WatermarkPosition,
    scale: f32,
    font_size: f32,
}

impl WatermarkSettings {
    fn from_options(options: &WatermarkOptions) -> Result<Self, OxideError> {
        let opacity = options.opacity.unwrap_or(0.25);
        if !opacity.is_finite() || !(0.0..=1.0).contains(&opacity) {
            return Err(OxideError::InvalidInput {
                reason: "watermark opacity must be between 0.0 and 1.0".to_owned(),
            });
        }
        let rotation_degrees = options.rotation.unwrap_or(0.0);
        if !rotation_degrees.is_finite() {
            return Err(OxideError::InvalidInput {
                reason: "watermark rotation must be finite".to_owned(),
            });
        }
        let scale = options.scale.unwrap_or(0.35);
        if !scale.is_finite() || scale <= 0.0 {
            return Err(OxideError::InvalidInput {
                reason: "watermark scale must be greater than zero".to_owned(),
            });
        }
        let font_size = options.font_size.unwrap_or(48.0);
        if !font_size.is_finite() || font_size <= 0.0 {
            return Err(OxideError::InvalidInput {
                reason: "watermark font size must be greater than zero".to_owned(),
            });
        }

        Ok(Self {
            opacity,
            rotation_degrees,
            position: WatermarkPosition::parse(options.position.as_deref().unwrap_or("center"))?,
            scale,
            font_size,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WatermarkPosition {
    Center,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl WatermarkPosition {
    fn parse(value: &str) -> Result<Self, OxideError> {
        match value {
            "center" => Ok(Self::Center),
            "top_left" => Ok(Self::TopLeft),
            "top_right" => Ok(Self::TopRight),
            "bottom_left" => Ok(Self::BottomLeft),
            "bottom_right" => Ok(Self::BottomRight),
            other => Err(OxideError::InvalidInput {
                reason: format!("unsupported watermark position '{other}'"),
            }),
        }
    }
}

#[derive(Debug, Clone)]
struct ResolvedFont {
    resource_name: Vec<u8>,
    base_font: Vec<u8>,
    metrics: FontMetrics,
}

#[derive(Debug, Clone, Copy)]
struct FontMetrics {
    units_per_em: u16,
    ascent: i16,
    descent: i16,
}

fn resolve_watermark_font(options: &WatermarkOptions) -> Result<ResolvedFont, OxideError> {
    let (font_bytes, family_name) = if let Some(path) = &options.font_path {
        let bytes = std::fs::read(path).map_err(|_| OxideError::FontResolution)?;
        let mut db = fontdb::Database::new();
        db.load_font_data(bytes.clone());
        let face = db.faces().next().ok_or(OxideError::FontResolution)?;
        (bytes, sanitize_pdf_name(&face.families[0].0))
    } else {
        let family = options.font.as_deref().ok_or(OxideError::FontResolution)?;
        let mut db = fontdb::Database::new();
        db.load_system_fonts();
        let query = fontdb::Query {
            families: &[fontdb::Family::Name(family)],
            ..fontdb::Query::default()
        };
        let id = db.query(&query).ok_or(OxideError::FontResolution)?;
        let bytes = db
            .with_face_data(id, |data, _index| data.to_vec())
            .ok_or(OxideError::FontResolution)?;
        (bytes, sanitize_pdf_name(family))
    };

    let metrics = read_font_metrics(&font_bytes)?;
    Ok(ResolvedFont {
        resource_name: b"OxWmF1".to_vec(),
        base_font: family_name,
        metrics,
    })
}

fn read_font_metrics(bytes: &[u8]) -> Result<FontMetrics, OxideError> {
    let font = skrifa::FontRef::from_index(bytes, 0).map_err(|_| OxideError::FontResolution)?;
    let head = font.head().map_err(|_| OxideError::FontResolution)?;
    let hhea = font.hhea().map_err(|_| OxideError::FontResolution)?;
    Ok(FontMetrics {
        units_per_em: head.units_per_em(),
        ascent: hhea.ascender().into(),
        descent: hhea.descender().into(),
    })
}

fn sanitize_pdf_name(value: &str) -> Vec<u8> {
    let name = value
        .bytes()
        .filter(|byte| byte.is_ascii_alphanumeric() || *byte == b'-' || *byte == b'_')
        .collect::<Vec<_>>();
    if name.is_empty() {
        b"OxideWatermarkFont".to_vec()
    } else {
        name
    }
}

fn append_text_watermark(
    document: &mut lopdf::Document,
    pages: &[u32],
    text: &str,
    font: &ResolvedFont,
    settings: WatermarkSettings,
) -> Result<(), OxideError> {
    if !text.is_ascii() {
        return Err(OxideError::UnsupportedPdfFeature {
            feature: "non-ASCII text watermark".to_owned(),
        });
    }
    let font_id = document.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => Object::Name(font.base_font.clone()),
        "Encoding" => Object::Name(b"WinAnsiEncoding".to_vec()),
    });
    let gs_id = graphics_state(document, settings.opacity);
    let page_map = document.get_pages();

    for page_number in pages {
        let page_id = *page_map
            .get(page_number)
            .ok_or_else(|| OxideError::InvalidInput {
                reason: format!("page {page_number} is out of range"),
            })?;
        add_resource_dict_entry(
            document,
            page_id,
            b"Font",
            font.resource_name.clone(),
            Object::Reference(font_id),
        )?;
        add_resource_dict_entry(
            document,
            page_id,
            b"ExtGState",
            b"OxWmGS".to_vec(),
            Object::Reference(gs_id),
        )?;
        let (page_width, page_height) = page_size(document, page_id)?;
        let text_width = approximate_text_width(text, font.metrics, settings.font_size);
        let text_height = settings.font_size;
        let (x, y) = watermark_origin(
            settings.position,
            page_width,
            page_height,
            text_width,
            text_height,
        );
        let content = text_watermark_content(text, &font.resource_name, settings, x, y)?;
        document
            .add_page_contents(page_id, content)
            .map_err(|_| OxideError::WritePdf)?;
    }

    Ok(())
}

fn page_is_structurally_blank(
    document: &lopdf::Document,
    page_id: lopdf::ObjectId,
) -> Result<bool, OxideError> {
    let page = document
        .get_object(page_id)
        .and_then(Object::as_dict)
        .map_err(|_| OxideError::ParsePdf)?;
    let has_content = match page.get(b"Contents") {
        Ok(Object::Array(items)) => !items.is_empty(),
        Ok(Object::Stream(stream)) => !stream.content.is_empty(),
        Ok(Object::Reference(id)) => {
            let stream = document
                .get_object(*id)
                .and_then(Object::as_stream)
                .map_err(|_| OxideError::ParsePdf)?;
            !stream.content.is_empty()
        }
        Ok(Object::Null) | Err(_) => false,
        Ok(_) => true,
    };
    if has_content {
        return Ok(false);
    }
    let has_resources = match page.get(b"Resources") {
        Ok(Object::Dictionary(dictionary)) => !dictionary.is_empty(),
        Ok(Object::Reference(id)) => {
            let dictionary = document
                .get_object(*id)
                .and_then(Object::as_dict)
                .map_err(|_| OxideError::ParsePdf)?;
            !dictionary.is_empty()
        }
        Ok(_) => true,
        Err(_) => false,
    };

    Ok(!has_resources)
}

fn scale_page_boxes(
    document: &mut lopdf::Document,
    page_id: lopdf::ObjectId,
    factor: f32,
) -> Result<(), OxideError> {
    let page = document
        .get_object_mut(page_id)
        .and_then(Object::as_dict_mut)
        .map_err(|_| OxideError::ParsePdf)?;
    for key in [
        b"MediaBox".as_slice(),
        b"CropBox",
        b"BleedBox",
        b"TrimBox",
        b"ArtBox",
    ] {
        if let Ok(object) = page.get_mut(key) {
            scale_box_object(object, factor)?;
        }
    }
    Ok(())
}

fn scale_box_object(object: &mut Object, factor: f32) -> Result<(), OxideError> {
    let values = object.as_array_mut().map_err(|_| OxideError::ParsePdf)?;
    if values.len() != 4 {
        return Err(OxideError::ParsePdf);
    }
    for value in values {
        *value = Object::Real(object_to_f32(value)? * factor);
    }
    Ok(())
}

fn prepend_page_transform(
    document: &mut lopdf::Document,
    page_id: lopdf::ObjectId,
    factor: f32,
) -> Result<(), OxideError> {
    let transform = lopdf::content::Content {
        operations: vec![
            lopdf::content::Operation::new("q", vec![]),
            lopdf::content::Operation::new(
                "cm",
                vec![
                    Object::Real(factor),
                    Object::Real(0.0),
                    Object::Real(0.0),
                    Object::Real(factor),
                    Object::Real(0.0),
                    Object::Real(0.0),
                ],
            ),
        ],
    }
    .encode()
    .map_err(|_| OxideError::WritePdf)?;
    let restore = lopdf::content::Content {
        operations: vec![lopdf::content::Operation::new("Q", vec![])],
    }
    .encode()
    .map_err(|_| OxideError::WritePdf)?;
    document
        .add_page_contents(page_id, transform)
        .map_err(|_| OxideError::WritePdf)?;
    document
        .add_page_contents(page_id, restore)
        .map_err(|_| OxideError::WritePdf)?;
    Ok(())
}

fn merge_page_resources_into(
    document: &lopdf::Document,
    page_id: lopdf::ObjectId,
    resources: &mut Dictionary,
) -> Result<(), OxideError> {
    let (direct_resources, inherited_resource_ids) = document
        .get_page_resources(page_id)
        .map_err(|_| OxideError::ParsePdf)?;
    for resource_id in inherited_resource_ids.iter().rev() {
        let inherited = document
            .get_dictionary(*resource_id)
            .map_err(|_| OxideError::ParsePdf)?;
        merge_resource_dictionary(resources, inherited);
    }
    if let Some(direct) = direct_resources {
        merge_resource_dictionary(resources, direct);
    }
    Ok(())
}

#[derive(Debug, Clone, Copy)]
struct PageLayout {
    output_width: f32,
    output_height: f32,
}

fn validate_nup_options(options: &NUpOptions) -> Result<(), OxideError> {
    if options.columns == 0 || options.rows == 0 {
        return Err(OxideError::InvalidInput {
            reason: "nup columns and rows must be greater than zero".to_owned(),
        });
    }
    if options.columns > 8 || options.rows > 8 {
        return Err(OxideError::InvalidInput {
            reason: "nup columns and rows must be 8 or less".to_owned(),
        });
    }
    Ok(())
}

fn validate_page_number_options(options: &PageNumbersOptions) -> Result<(), OxideError> {
    if options.start == 0 {
        return Err(OxideError::InvalidInput {
            reason: "page number start must be greater than zero".to_owned(),
        });
    }
    if !options.font_size.is_finite() || options.font_size <= 0.0 {
        return Err(OxideError::InvalidInput {
            reason: "page number font size must be greater than zero".to_owned(),
        });
    }
    if !options.prefix.is_ascii() || !options.suffix.is_ascii() {
        return Err(OxideError::InvalidInput {
            reason: "page number prefix and suffix must be ASCII".to_owned(),
        });
    }
    Ok(())
}

fn page_layout_from_first_page(
    document: &lopdf::Document,
    page_ids: &[lopdf::ObjectId],
) -> Result<PageLayout, OxideError> {
    let first_page = page_ids.first().copied().ok_or(OxideError::ParsePdf)?;
    let (source_width, source_height) = page_size(document, first_page)?;
    if !source_width.is_finite()
        || !source_height.is_finite()
        || source_width <= 0.0
        || source_height <= 0.0
    {
        return Err(OxideError::ParsePdf);
    }

    Ok(PageLayout {
        output_width: source_width,
        output_height: source_height,
    })
}

fn booklet_page_order(page_count: usize) -> Vec<usize> {
    let padded_count = page_count.div_ceil(4) * 4;
    let mut order = Vec::with_capacity(padded_count);
    for sheet in 0..(padded_count / 4) {
        let left_front = padded_count - sheet * 2 - 1;
        let right_front = sheet * 2;
        let left_back = sheet * 2 + 1;
        let right_back = padded_count - sheet * 2 - 2;
        order.extend([left_front, right_front, left_back, right_back]);
    }
    order
}

fn impose_pages(
    source: &lopdf::Document,
    page_ids: &[lopdf::ObjectId],
    order: &[usize],
    layout: PageLayout,
    columns: u32,
    rows: u32,
) -> Result<Vec<u8>, OxideError> {
    let mut target = lopdf::Document::with_version("1.7");
    let catalog_id = target.new_object_id();
    let pages_id = target.new_object_id();
    let mut output_page_ids = Vec::new();
    let mut imported = BTreeMap::new();
    let slots_per_page = (columns * rows) as usize;

    for chunk in order.chunks(slots_per_page) {
        let page_id = target.new_object_id();
        let content_id = target.new_object_id();
        let mut resources = Dictionary::new();
        let mut xobjects = Dictionary::new();
        let mut operations = Vec::new();

        for (slot, source_index) in chunk.iter().enumerate() {
            if *source_index >= page_ids.len() {
                continue;
            }
            let source_page_id = page_ids[*source_index];
            let (source_width, source_height) = page_size(source, source_page_id)?;
            let xobject_id =
                page_form_xobject_from_source(source, &mut target, source_page_id, &mut imported)?;
            let resource_name = format!("OxPg{slot}").into_bytes();
            xobjects.set(resource_name.clone(), Object::Reference(xobject_id));
            operations.extend(imposed_page_operations(
                &resource_name,
                slot,
                columns,
                rows,
                layout,
                source_width,
                source_height,
            )?);
        }

        resources.set("XObject", Object::Dictionary(xobjects));
        let content = lopdf::content::Content { operations }
            .encode()
            .map_err(|_| OxideError::WritePdf)?;
        target.objects.insert(
            content_id,
            Object::Stream(Stream::new(Dictionary::new(), content)),
        );
        target.objects.insert(
            page_id,
            Object::Dictionary(dictionary! {
                "Type" => "Page",
                "Parent" => pages_id,
                "MediaBox" => crop_box_object([0.0, 0.0, layout.output_width, layout.output_height]),
                "CropBox" => crop_box_object([0.0, 0.0, layout.output_width, layout.output_height]),
                "Resources" => Object::Dictionary(resources),
                "Contents" => Object::Reference(content_id),
            }),
        );
        output_page_ids.push(page_id);
    }

    target.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => Object::Array(output_page_ids.iter().copied().map(Object::Reference).collect()),
            "Count" => output_page_ids.len() as u32,
        }),
    );
    target.objects.insert(
        catalog_id,
        Object::Dictionary(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        }),
    );
    target.trailer.set("Root", catalog_id);
    save_pdf(target)
}

fn imposed_page_operations(
    resource_name: &[u8],
    slot: usize,
    columns: u32,
    rows: u32,
    layout: PageLayout,
    source_width: f32,
    source_height: f32,
) -> Result<Vec<lopdf::content::Operation>, OxideError> {
    if !source_width.is_finite()
        || !source_height.is_finite()
        || source_width <= 0.0
        || source_height <= 0.0
    {
        return Err(OxideError::ParsePdf);
    }
    let cell_width = layout.output_width / columns as f32;
    let cell_height = layout.output_height / rows as f32;
    let column = (slot as u32) % columns;
    let row_from_top = (slot as u32) / columns;
    let scale = (cell_width / source_width).min(cell_height / source_height);
    let width = source_width * scale;
    let height = source_height * scale;
    let x = column as f32 * cell_width + (cell_width - width) / 2.0;
    let y = layout.output_height - (row_from_top + 1) as f32 * cell_height
        + (cell_height - height) / 2.0;

    Ok(vec![
        lopdf::content::Operation::new("q", vec![]),
        lopdf::content::Operation::new(
            "cm",
            vec![
                Object::Real(scale),
                Object::Real(0.0),
                Object::Real(0.0),
                Object::Real(scale),
                Object::Real(x),
                Object::Real(y),
            ],
        ),
        lopdf::content::Operation::new("Do", vec![Object::Name(resource_name.to_vec())]),
        lopdf::content::Operation::new("Q", vec![]),
    ])
}

fn page_form_xobject_from_source(
    source: &lopdf::Document,
    target: &mut lopdf::Document,
    page_id: lopdf::ObjectId,
    imported: &mut BTreeMap<lopdf::ObjectId, lopdf::ObjectId>,
) -> Result<lopdf::ObjectId, OxideError> {
    let content = source
        .get_page_content(page_id)
        .map_err(|_| OxideError::ParsePdf)?;
    let resources = imported_page_resources_with_cache(source, target, page_id, imported)?;
    let (width, height) = page_size(source, page_id)?;
    if width <= 0.0 || height <= 0.0 {
        return Err(OxideError::ParsePdf);
    }
    let mut dictionary = dictionary! {
        "Type" => "XObject",
        "Subtype" => "Form",
        "BBox" => crop_box_object([0.0, 0.0, width, height]),
        "Matrix" => Object::Array(vec![
            Object::Real(1.0),
            Object::Real(0.0),
            Object::Real(0.0),
            Object::Real(1.0),
            Object::Real(0.0),
            Object::Real(0.0),
        ]),
    };
    dictionary.set("Resources", resources);
    Ok(target.add_object(Stream::new(dictionary, content)))
}

fn imported_page_resources_with_cache(
    source: &lopdf::Document,
    target: &mut lopdf::Document,
    page_id: lopdf::ObjectId,
    imported: &mut BTreeMap<lopdf::ObjectId, lopdf::ObjectId>,
) -> Result<Dictionary, OxideError> {
    let (direct_resources, inherited_resource_ids) = source
        .get_page_resources(page_id)
        .map_err(|_| OxideError::ParsePdf)?;
    let mut resources = Dictionary::new();
    for resource_id in inherited_resource_ids.iter().rev() {
        let inherited = source
            .get_dictionary(*resource_id)
            .map_err(|_| OxideError::ParsePdf)?;
        merge_resource_dictionary(&mut resources, inherited);
    }
    if let Some(direct) = direct_resources {
        merge_resource_dictionary(&mut resources, direct);
    }
    let mut resource_object = Object::Dictionary(resources);
    remap_imported_references(&mut resource_object, source, target, imported)?;
    resource_object
        .as_dict()
        .cloned()
        .map_err(|_| OxideError::ParsePdf)
}

fn add_standard_font_resource(
    document: &mut lopdf::Document,
    pages: &[u32],
    resource_name: Vec<u8>,
) -> Result<(), OxideError> {
    let font_id = document.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
        "Encoding" => "WinAnsiEncoding",
    });
    let page_map = document.get_pages();
    let page_ids = pages
        .iter()
        .map(|page| {
            page_map
                .get(page)
                .copied()
                .ok_or_else(|| OxideError::InvalidInput {
                    reason: format!("page {page} is out of range"),
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    for page_id in page_ids {
        add_resource_dict_entry(
            document,
            page_id,
            b"Font",
            resource_name.clone(),
            Object::Reference(font_id),
        )?;
    }
    Ok(())
}

fn page_number_content(
    label: &str,
    page_width: f32,
    page_height: f32,
    font_size: f32,
    position: PageNumberPosition,
) -> Result<Vec<u8>, OxideError> {
    let escaped_label = pdf_literal_ascii(label)?;
    let width = label.chars().count() as f32 * font_size * 0.5;
    let margin = 36.0;
    let (x, y) = match position {
        PageNumberPosition::TopLeft => (margin, page_height - margin),
        PageNumberPosition::TopCenter => ((page_width - width) / 2.0, page_height - margin),
        PageNumberPosition::TopRight => (page_width - margin - width, page_height - margin),
        PageNumberPosition::BottomLeft => (margin, margin),
        PageNumberPosition::BottomCenter => ((page_width - width) / 2.0, margin),
        PageNumberPosition::BottomRight => (page_width - margin - width, margin),
    };
    let content = format!("q BT /OxPnF1 {font_size} Tf {x} {y} Td ({escaped_label}) Tj ET Q\n");
    Ok(content.into_bytes())
}

fn pdf_literal_ascii(value: &str) -> Result<String, OxideError> {
    if !value.is_ascii() {
        return Err(OxideError::InvalidInput {
            reason: "page number text must be ASCII".to_owned(),
        });
    }
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '(' => escaped.push_str("\\("),
            ')' => escaped.push_str("\\)"),
            '\\' => escaped.push_str("\\\\"),
            _ => escaped.push(character),
        }
    }
    Ok(escaped)
}

fn append_image_watermark(
    document: &mut lopdf::Document,
    pages: &[u32],
    image: &DecodedImage,
    settings: WatermarkSettings,
) -> Result<(), OxideError> {
    let image_id = document.add_object(image_xobject(image));
    append_xobject_watermark(
        document,
        pages,
        image_id,
        image.width as f32,
        image.height as f32,
        b"OxWmIm".to_vec(),
        settings,
    )
}

fn append_svg_watermark(
    document: &mut lopdf::Document,
    pages: &[u32],
    tree: &svg2pdf::usvg::Tree,
    settings: WatermarkSettings,
) -> Result<(), OxideError> {
    let size = tree.size();
    let width = size.width();
    let height = size.height();
    let svg_id = svg_form_xobject(document, tree, width, height)?;
    append_xobject_watermark(
        document,
        pages,
        svg_id,
        width,
        height,
        b"OxWmSvg".to_vec(),
        settings,
    )
}

fn svg_form_xobject(
    target: &mut lopdf::Document,
    tree: &svg2pdf::usvg::Tree,
    width: f32,
    height: f32,
) -> Result<lopdf::ObjectId, OxideError> {
    let conversion_options = svg2pdf::ConversionOptions {
        embed_text: false,
        ..svg2pdf::ConversionOptions::default()
    };
    let bytes = svg2pdf::to_pdf(tree, conversion_options, svg2pdf::PageOptions::default())
        .map_err(|_| OxideError::WritePdf)?;
    let source = lopdf::Document::load_mem(&bytes).map_err(|_| OxideError::ParsePdf)?;
    let page_id = source
        .get_pages()
        .into_values()
        .next()
        .ok_or(OxideError::ParsePdf)?;
    let content = source
        .get_page_content(page_id)
        .map_err(|_| OxideError::ParsePdf)?;
    let resources = imported_page_resources(&source, target, page_id)?;

    let mut dict = dictionary! {
        "Type" => "XObject",
        "Subtype" => "Form",
        "BBox" => Object::Array(vec![
            Object::Real(0.0),
            Object::Real(0.0),
            Object::Real(width),
            Object::Real(height),
        ]),
        "Matrix" => Object::Array(vec![
            Object::Real(1.0),
            Object::Real(0.0),
            Object::Real(0.0),
            Object::Real(1.0),
            Object::Real(0.0),
            Object::Real(0.0),
        ]),
    };
    dict.set("Resources", resources);
    Ok(target.add_object(Stream::new(dict, content)))
}

fn imported_page_resources(
    source: &lopdf::Document,
    target: &mut lopdf::Document,
    page_id: lopdf::ObjectId,
) -> Result<Dictionary, OxideError> {
    let (direct_resources, inherited_resource_ids) = source
        .get_page_resources(page_id)
        .map_err(|_| OxideError::ParsePdf)?;
    let mut resources = Dictionary::new();
    for resource_id in inherited_resource_ids.iter().rev() {
        let inherited = source
            .get_dictionary(*resource_id)
            .map_err(|_| OxideError::ParsePdf)?;
        merge_resource_dictionary(&mut resources, inherited);
    }
    if let Some(direct) = direct_resources {
        merge_resource_dictionary(&mut resources, direct);
    }

    let mut resource_object = Object::Dictionary(resources);
    let mut imported = BTreeMap::new();
    remap_imported_references(&mut resource_object, source, target, &mut imported)?;
    resource_object
        .as_dict()
        .cloned()
        .map_err(|_| OxideError::ParsePdf)
}

fn merge_resource_dictionary(target: &mut Dictionary, source: &Dictionary) {
    for (key, value) in source.iter() {
        match (target.get_mut(key), value) {
            (Ok(Object::Dictionary(target_dict)), Object::Dictionary(source_dict)) => {
                merge_resource_dictionary(target_dict, source_dict);
            }
            _ => {
                target.set(key.clone(), value.clone());
            }
        }
    }
}

fn remap_imported_references(
    object: &mut Object,
    source: &lopdf::Document,
    target: &mut lopdf::Document,
    imported: &mut BTreeMap<lopdf::ObjectId, lopdf::ObjectId>,
) -> Result<(), OxideError> {
    match object {
        Object::Reference(source_id) => {
            let target_id = import_indirect_object(*source_id, source, target, imported)?;
            *source_id = target_id;
        }
        Object::Array(items) => {
            for item in items {
                remap_imported_references(item, source, target, imported)?;
            }
        }
        Object::Dictionary(dictionary) => {
            for (_, value) in dictionary.iter_mut() {
                remap_imported_references(value, source, target, imported)?;
            }
        }
        Object::Stream(stream) => {
            for (_, value) in stream.dict.iter_mut() {
                remap_imported_references(value, source, target, imported)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn import_indirect_object(
    source_id: lopdf::ObjectId,
    source: &lopdf::Document,
    target: &mut lopdf::Document,
    imported: &mut BTreeMap<lopdf::ObjectId, lopdf::ObjectId>,
) -> Result<lopdf::ObjectId, OxideError> {
    if let Some(target_id) = imported.get(&source_id) {
        return Ok(*target_id);
    }

    let target_id = target.new_object_id();
    imported.insert(source_id, target_id);
    let mut object = source
        .objects
        .get(&source_id)
        .cloned()
        .ok_or(OxideError::ParsePdf)?;
    remap_imported_references(&mut object, source, target, imported)?;
    target.set_object(target_id, object);
    Ok(target_id)
}

fn append_xobject_watermark(
    document: &mut lopdf::Document,
    pages: &[u32],
    xobject_id: lopdf::ObjectId,
    natural_width: f32,
    natural_height: f32,
    resource_name: Vec<u8>,
    settings: WatermarkSettings,
) -> Result<(), OxideError> {
    let gs_id = graphics_state(document, settings.opacity);
    let page_map = document.get_pages();
    for page_number in pages {
        let page_id = *page_map
            .get(page_number)
            .ok_or_else(|| OxideError::InvalidInput {
                reason: format!("page {page_number} is out of range"),
            })?;
        add_resource_dict_entry(
            document,
            page_id,
            b"XObject",
            resource_name.clone(),
            Object::Reference(xobject_id),
        )?;
        add_resource_dict_entry(
            document,
            page_id,
            b"ExtGState",
            b"OxWmGS".to_vec(),
            Object::Reference(gs_id),
        )?;
        let (page_width, page_height) = page_size(document, page_id)?;
        let scale = (page_width / natural_width)
            .min(page_height / natural_height)
            .min(1.0)
            * settings.scale;
        let width = natural_width * scale;
        let height = natural_height * scale;
        let (x, y) = watermark_origin(settings.position, page_width, page_height, width, height);
        let content = xobject_watermark_content(&resource_name, settings, x, y, width, height)?;
        document
            .add_page_contents(page_id, content)
            .map_err(|_| OxideError::WritePdf)?;
    }

    Ok(())
}

fn graphics_state(document: &mut lopdf::Document, opacity: f32) -> lopdf::ObjectId {
    document.add_object(dictionary! {
        "Type" => "ExtGState",
        "ca" => Object::Real(opacity),
        "CA" => Object::Real(opacity),
    })
}

fn add_resource_dict_entry(
    document: &mut lopdf::Document,
    page_id: lopdf::ObjectId,
    dict_name: &[u8],
    resource_name: Vec<u8>,
    value: Object,
) -> Result<(), OxideError> {
    let resources = document
        .get_or_create_resources(page_id)
        .and_then(Object::as_dict_mut)
        .map_err(|_| OxideError::WritePdf)?;
    if !resources.has(dict_name) {
        resources.set(dict_name.to_vec(), Dictionary::new());
    }
    let dictionary = resources
        .get_mut(dict_name)
        .and_then(Object::as_dict_mut)
        .map_err(|_| OxideError::WritePdf)?;
    dictionary.set(resource_name, value);
    Ok(())
}

fn page_size(
    document: &lopdf::Document,
    page_id: lopdf::ObjectId,
) -> Result<(f32, f32), OxideError> {
    let page = document
        .get_object(page_id)
        .and_then(Object::as_dict)
        .map_err(|_| OxideError::ParsePdf)?;
    let media_box = page
        .get(b"MediaBox")
        .and_then(Object::as_array)
        .map_err(|_| OxideError::ParsePdf)?;
    if media_box.len() != 4 {
        return Err(OxideError::ParsePdf);
    }
    let width = object_to_f32(&media_box[2])? - object_to_f32(&media_box[0])?;
    let height = object_to_f32(&media_box[3])? - object_to_f32(&media_box[1])?;
    Ok((width, height))
}

fn object_to_f32(object: &Object) -> Result<f32, OxideError> {
    match object {
        Object::Integer(value) => Ok(*value as f32),
        Object::Real(value) => Ok(*value),
        _ => Err(OxideError::ParsePdf),
    }
}

fn watermark_origin(
    position: WatermarkPosition,
    page_width: f32,
    page_height: f32,
    width: f32,
    height: f32,
) -> (f32, f32) {
    let margin = 36.0;
    match position {
        WatermarkPosition::Center => ((page_width - width) / 2.0, (page_height - height) / 2.0),
        WatermarkPosition::TopLeft => (margin, page_height - height - margin),
        WatermarkPosition::TopRight => (page_width - width - margin, page_height - height - margin),
        WatermarkPosition::BottomLeft => (margin, margin),
        WatermarkPosition::BottomRight => (page_width - width - margin, margin),
    }
}

fn approximate_text_width(text: &str, metrics: FontMetrics, font_size: f32) -> f32 {
    let em = f32::from(metrics.units_per_em.max(1));
    let height_units = i32::from(metrics.ascent) - i32::from(metrics.descent);
    let height_ratio = (height_units.max(1) as f32 / em).max(0.5);
    text.len() as f32 * font_size * 0.55 * height_ratio
}

fn text_watermark_content(
    text: &str,
    font_name: &[u8],
    settings: WatermarkSettings,
    x: f32,
    y: f32,
) -> Result<Vec<u8>, OxideError> {
    let matrix = rotation_matrix(settings.rotation_degrees, x, y);
    lopdf::content::Content {
        operations: vec![
            lopdf::content::Operation::new("q", vec![]),
            lopdf::content::Operation::new("gs", vec![Object::Name(b"OxWmGS".to_vec())]),
            lopdf::content::Operation::new(
                "cm",
                matrix.iter().copied().map(Object::Real).collect(),
            ),
            lopdf::content::Operation::new("BT", vec![]),
            lopdf::content::Operation::new(
                "Tf",
                vec![
                    Object::Name(font_name.to_vec()),
                    Object::Real(settings.font_size),
                ],
            ),
            lopdf::content::Operation::new("Td", vec![Object::Integer(0), Object::Integer(0)]),
            lopdf::content::Operation::new("Tj", vec![Object::string_literal(text)]),
            lopdf::content::Operation::new("ET", vec![]),
            lopdf::content::Operation::new("Q", vec![]),
        ],
    }
    .encode()
    .map_err(|_| OxideError::WritePdf)
}

fn xobject_watermark_content(
    resource_name: &[u8],
    settings: WatermarkSettings,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
) -> Result<Vec<u8>, OxideError> {
    let mut matrix = rotation_matrix(settings.rotation_degrees, x, y);
    matrix[0] *= width;
    matrix[1] *= width;
    matrix[2] *= height;
    matrix[3] *= height;
    lopdf::content::Content {
        operations: vec![
            lopdf::content::Operation::new("q", vec![]),
            lopdf::content::Operation::new("gs", vec![Object::Name(b"OxWmGS".to_vec())]),
            lopdf::content::Operation::new(
                "cm",
                matrix.iter().copied().map(Object::Real).collect(),
            ),
            lopdf::content::Operation::new("Do", vec![Object::Name(resource_name.to_vec())]),
            lopdf::content::Operation::new("Q", vec![]),
        ],
    }
    .encode()
    .map_err(|_| OxideError::WritePdf)
}

fn rotation_matrix(degrees: f32, x: f32, y: f32) -> [f32; 6] {
    let radians = degrees.to_radians();
    let cos = radians.cos();
    let sin = radians.sin();
    [cos, sin, -sin, cos, x, y]
}

fn image_xobject(image: &DecodedImage) -> Stream {
    let dict = dictionary! {
        "Type" => "XObject",
        "Subtype" => "Image",
        "Width" => image.width as i64,
        "Height" => image.height as i64,
        "ColorSpace" => "DeviceRGB",
        "BitsPerComponent" => 8,
    };
    Stream::new(dict, image.rgb.clone())
}

fn enforce_input_bytes(size: usize, limits: &ResourceLimits) -> Result<(), OxideError> {
    if let Some(limit) = limits.max_input_bytes {
        if size as u64 > limit {
            return Err(resource_limit("max_input_bytes"));
        }
    }

    Ok(())
}

fn enforce_max_pages(pages: usize, limits: &ResourceLimits) -> Result<(), OxideError> {
    if let Some(limit) = limits.max_pages {
        if pages as u32 > limit {
            return Err(resource_limit("max_pages"));
        }
    }

    Ok(())
}

fn enforce_max_pixels(pixels: u64, limits: &ResourceLimits) -> Result<(), OxideError> {
    if let Some(limit) = limits.max_pixels {
        if pixels > limit {
            return Err(resource_limit("max_pixels"));
        }
    }

    Ok(())
}

fn enforce_output_bytes(size: usize, limits: &ResourceLimits) -> Result<(), OxideError> {
    if let Some(limit) = limits.max_output_bytes {
        if size as u64 > limit {
            return Err(resource_limit("max_output_bytes"));
        }
    }

    Ok(())
}

fn enforce_artifact_output_bytes(
    artifact: &Artifact,
    limits: &ResourceLimits,
) -> Result<(), OxideError> {
    enforce_output_bytes(artifact_size(artifact), limits)
}

fn artifact_size(artifact: &Artifact) -> usize {
    match artifact {
        Artifact::Pdf(pdf) => pdf.bytes.len(),
        Artifact::Image(image) => image.bytes.len(),
        Artifact::Text(text) => text.text.len(),
        Artifact::Svg(svg) => svg.bytes.len(),
        Artifact::Bytes(bytes) => bytes.bytes.len(),
    }
}

fn enforce_timeout(started_at: Instant, timeout: Option<Duration>) -> Result<(), OxideError> {
    if timeout.is_some_and(|timeout| started_at.elapsed() > timeout) {
        return Err(resource_limit("timeout_ms"));
    }

    Ok(())
}

fn resource_limit(limit: impl Into<String>) -> OxideError {
    OxideError::ResourceLimitExceeded {
        limit: limit.into(),
    }
}

fn rebuild_pages_tree(
    document: &mut lopdf::Document,
    page_ids: &[lopdf::ObjectId],
) -> Result<(), OxideError> {
    let catalog = document.catalog().map_err(|_| OxideError::ParsePdf)?;
    let pages_id = catalog
        .get(b"Pages")
        .and_then(lopdf::Object::as_reference)
        .map_err(|_| OxideError::ParsePdf)?;
    {
        let pages_dictionary = document
            .get_object_mut(pages_id)
            .and_then(lopdf::Object::as_dict_mut)
            .map_err(|_| OxideError::ParsePdf)?;
        pages_dictionary.set("Count", page_ids.len() as u32);
        pages_dictionary.set(
            "Kids",
            page_ids
                .iter()
                .copied()
                .map(lopdf::Object::Reference)
                .collect::<Vec<_>>(),
        );
    }
    for page_id in page_ids {
        let page_dictionary = document
            .get_object_mut(*page_id)
            .and_then(lopdf::Object::as_dict_mut)
            .map_err(|_| OxideError::ParsePdf)?;
        page_dictionary.set("Parent", pages_id);
    }

    Ok(())
}

fn invalid_workflow(reason: impl Into<String>) -> OxideError {
    OxideError::InvalidWorkflow {
        reason: reason.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
