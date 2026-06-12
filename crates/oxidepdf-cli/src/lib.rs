#![forbid(unsafe_code)]

use clap::{CommandFactory, Parser, Subcommand};
use oxidepdf_core::{
    execute_workflow, AnnotationEditAction, AnnotationEditOptions, AnnotationInspectOptions,
    Artifact, ArtifactRef, ArtifactStore, AttachmentEditAction, AttachmentEditOptions,
    AttachmentExtractOptions, AttachmentInspectOptions, BookletOptions, ColorEditAction,
    ColorEditOptions, CompressionImageFormat, CompressionImageOptions, CompressionMode,
    CompressionOptions, CropPagesOptions, DeleteBlankPagesOptions, ExtractTextOptions,
    FormFieldValue, FormFillOptions, FormInspectOptions, ImageEditAction, ImageEditOptions,
    ImageExtractOptions, ImageInspectOptions, ImageToPdfOptions, InteractiveRemovalOptions,
    MergeOptions, MetadataEditAction, MetadataEditOptions, MetadataEntry, MetadataInspectOptions,
    NUpOptions, OperatorSpec, OutlineEditAction, OutlineEditOptions, OutlineInspectOptions,
    OutlineTree, OverlayKind, OverlayOptions, OxideError, PageNumberPosition, PageNumbersOptions,
    PageSelectionOptions, PdfCompareOptions, PdfEditOptions, PdfInspectOptions, PdfOperatorRunner,
    PdfSecurityOptions, PdfSignOptions, RenderOptions, ReorderOptions, RotateOptions,
    ScalePagesOptions, SignatureOptions, SinglePageOptions, SplitOptions, SvgToPdfOptions, TaskId,
    TaskSpec, WatermarkKind, WatermarkOptions, Workflow, WorkflowMetadata, WorkflowVersion,
};
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

/// OxidePDF command-line arguments.
#[derive(Debug, Parser)]
#[command(
    name = "oxidepdf",
    version,
    about = "Pure Rust PDF toolkit",
    long_about = "OxidePDF is a pure Rust PDF toolkit."
)]
pub struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Run a workflow document.
    Run(RunArgs),
    /// Edit or create PDF files.
    #[command(name = "pdf-edit")]
    #[command(subcommand)]
    PdfEdit(PdfEditCommand),
    /// Inspect or render PDF files.
    #[command(name = "pdf-inspect")]
    #[command(subcommand)]
    PdfInspect(PdfInspectCommand),
    /// Apply password, encryption, or permission operations.
    #[command(name = "pdf-security")]
    PdfSecurity(PdfSecurityArgs),
    /// Compare PDF files.
    #[command(name = "pdf-compare")]
    PdfCompare(PdfCompareArgs),
    /// Sign or verify PDF signature material.
    #[command(name = "pdf-sign")]
    #[command(subcommand)]
    PdfSign(PdfSignCommand),
    /// Inspect, set, delete, or validate document metadata.
    #[command(subcommand)]
    Metadata(MetadataCommand),
    /// Inspect, set, or delete document outlines.
    #[command(subcommand)]
    Outline(OutlineCommand),
    /// Add, list, extract, or delete embedded file attachments.
    #[command(name = "attach")]
    #[command(subcommand)]
    Attach(AttachCommand),
    /// List, add, or delete annotations.
    #[command(name = "annot")]
    #[command(subcommand)]
    Annot(AnnotCommand),
    /// Fill, unlock, inspect, or remove interactive forms.
    #[command(subcommand)]
    Form(FormCommand),
    /// Remove selected interactive document elements.
    #[command(subcommand)]
    Interactive(InteractiveCommand),
    /// Add a text stamp to a PDF.
    Stamp(StampArgs),
    /// Add visual signature appearance only.
    #[command(name = "signature-appearance")]
    SignatureAppearance(SignatureAppearanceArgs),
    /// Overlay one PDF page onto another PDF.
    #[command(name = "overlay-pdf")]
    OverlayPdf(OverlayPdfArgs),
    /// Inspect or edit image XObject resources.
    #[command(subcommand)]
    Image(ImageCommand),
    /// Edit simple page colors.
    #[command(subcommand)]
    Color(ColorCommand),
    /// Compress and optimize a PDF without implicit quality loss.
    Compress(CompressArgs),
}

#[derive(Debug, Subcommand)]
enum PdfEditCommand {
    /// Merge multiple PDFs into one output.
    Merge(MergeArgs),
    /// Keep selected pages from a PDF.
    #[command(name = "keep-pages")]
    KeepPages(PageSelectionArgs),
    /// Extract selected pages from a PDF.
    #[command(name = "extract-pages")]
    ExtractPages(PageSelectionArgs),
    /// Reorder pages in a PDF.
    #[command(name = "reorder-pages")]
    ReorderPages(PageSelectionArgs),
    /// Rotate selected PDF pages.
    #[command(name = "rotate-pages")]
    RotatePages(RotateArgs),
    /// Delete selected pages from a PDF.
    #[command(name = "delete-pages")]
    DeletePages(PageSelectionArgs),
    /// Delete structurally blank pages from a PDF.
    #[command(name = "delete-blank-pages")]
    DeleteBlankPages(DeleteBlankPagesArgs),
    /// Crop selected PDF pages.
    #[command(name = "crop-pages")]
    CropPages(CropPagesArgs),
    /// Scale selected PDF pages.
    #[command(name = "scale-pages")]
    ScalePages(ScalePagesArgs),
    /// Combine all pages into one tall page.
    #[command(name = "single-page")]
    SinglePage(SinglePageArgs),
    /// Lay multiple source pages on each output page.
    #[command(name = "nup")]
    NUp(NUpArgs),
    /// Arrange pages for booklet printing.
    #[command(name = "booklet")]
    Booklet(BookletArgs),
    /// Add page numbers to pages.
    #[command(name = "page-numbers")]
    PageNumbers(PageNumbersArgs),
    /// Convert one or more images into PDF pages.
    #[command(name = "img2pdf")]
    Img2pdf(ImageToPdfArgs),
    /// Convert an SVG document into a PDF.
    #[command(name = "svg2pdf")]
    Svg2pdf(SvgToPdfArgs),
    /// Add a text, image, or SVG watermark to a PDF.
    Watermark(WatermarkArgs),
    /// Compress and optimize a PDF.
    Compress(CompressArgs),
}

#[derive(Debug, Subcommand)]
enum PdfInspectCommand {
    /// Render a PDF page into a PNG image.
    Render(RenderArgs),
    /// Extract plain text from a PDF.
    #[command(name = "extract-text")]
    ExtractText(ExtractTextArgs),
}

#[derive(Debug, Subcommand)]
enum PdfSignCommand {
    /// Verify PDF signatures and certificates.
    Verify(VerifySignaturesArgs),
}

#[derive(Debug, Subcommand)]
enum MetadataCommand {
    Get(InspectOutputArgs),
    Set(MetadataSetArgs),
    Delete(MetadataDeleteArgs),
    Validate(InspectOutputArgs),
}

#[derive(Debug, Subcommand)]
enum OutlineCommand {
    Get(InspectOutputArgs),
    Set(OutlineSetArgs),
    Delete(EditOutputArgs),
}

#[derive(Debug, Subcommand)]
enum AttachCommand {
    Add(AttachAddArgs),
    List(InspectOutputArgs),
    Extract(AttachExtractArgs),
    Delete(AttachDeleteArgs),
}

#[derive(Debug, Subcommand)]
enum AnnotCommand {
    List(InspectOutputArgs),
    Add(AnnotAddArgs),
    Delete(AnnotDeleteArgs),
}

#[derive(Debug, Subcommand)]
enum FormCommand {
    Inspect(InspectOutputArgs),
    Fill(FormFillArgs),
    #[command(name = "unlock-readonly")]
    UnlockReadonly(EditOutputArgs),
    Remove(EditOutputArgs),
}

#[derive(Debug, Subcommand)]
enum InteractiveCommand {
    Remove(InteractiveRemoveArgs),
}

#[derive(Debug, Subcommand)]
enum ImageCommand {
    List(InspectOutputArgs),
    Add(ImageAddArgs),
    Replace(ImageReplaceArgs),
    Delete(ImageDeleteArgs),
    Extract(ImageExtractArgs),
}

#[derive(Debug, Subcommand)]
enum ColorCommand {
    Contrast(ColorContrastArgs),
    Invert(ColorEditArgs),
    Replace(ColorReplaceArgs),
}

#[derive(Debug, Parser)]
struct RunArgs {
    /// Workflow YAML or JSON file, or `-` to read from stdin.
    #[arg(long)]
    workflow: PathBuf,

    /// Overwrite output files when they already exist.
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Parser)]
struct MergeArgs {
    /// Input PDF files.
    #[arg(required = true, num_args = 2..)]
    inputs: Vec<PathBuf>,

    /// Output PDF file, or `-` to write to stdout.
    #[arg(short, long)]
    output: PathBuf,

    /// Overwrite output files when they already exist.
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Parser)]
struct PageSelectionArgs {
    /// Input PDF file, or `-` to read from stdin.
    input: PathBuf,

    /// Page range or sequence, for example `1,3-5`.
    #[arg(long)]
    pages: String,

    /// Output PDF file, or `-` to write to stdout.
    #[arg(short, long)]
    output: PathBuf,

    /// Overwrite output files when they already exist.
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Parser)]
struct RotateArgs {
    /// Input PDF file, or `-` to read from stdin.
    input: PathBuf,

    /// Page range, for example `1,3-5`.
    #[arg(long)]
    pages: String,

    /// Rotation in degrees. Must be 90, 180, or 270.
    #[arg(long)]
    degrees: i16,

    /// Output PDF file, or `-` to write to stdout.
    #[arg(short, long)]
    output: PathBuf,

    /// Overwrite output files when they already exist.
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Parser)]
struct DeleteBlankPagesArgs {
    /// Input PDF file, or `-` to read from stdin.
    input: PathBuf,

    /// Output PDF file, or `-` to write to stdout.
    #[arg(short, long)]
    output: PathBuf,

    /// Overwrite output files when they already exist.
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Parser)]
struct CropPagesArgs {
    /// Input PDF file, or `-` to read from stdin.
    input: PathBuf,

    /// Page range, for example `1,3-5`. Defaults to all pages.
    #[arg(long)]
    pages: Option<String>,

    /// Left coordinate of the new CropBox.
    #[arg(long)]
    left: f32,

    /// Bottom coordinate of the new CropBox.
    #[arg(long)]
    bottom: f32,

    /// Right coordinate of the new CropBox.
    #[arg(long)]
    right: f32,

    /// Top coordinate of the new CropBox.
    #[arg(long)]
    top: f32,

    /// Output PDF file, or `-` to write to stdout.
    #[arg(short, long)]
    output: PathBuf,

    /// Overwrite output files when they already exist.
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Parser)]
struct ScalePagesArgs {
    /// Input PDF file, or `-` to read from stdin.
    input: PathBuf,

    /// Page range, for example `1,3-5`. Defaults to all pages.
    #[arg(long)]
    pages: Option<String>,

    /// Scale factor applied to page boxes and page contents.
    #[arg(long)]
    factor: f32,

    /// Output PDF file, or `-` to write to stdout.
    #[arg(short, long)]
    output: PathBuf,

    /// Overwrite output files when they already exist.
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Parser)]
struct SinglePageArgs {
    /// Input PDF file, or `-` to read from stdin.
    input: PathBuf,

    /// Output PDF file, or `-` to write to stdout.
    #[arg(short, long)]
    output: PathBuf,

    /// Overwrite output files when they already exist.
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Parser)]
struct NUpArgs {
    /// Input PDF file, or `-` to read from stdin.
    input: PathBuf,

    /// Number of columns on each output page.
    #[arg(long)]
    columns: u32,

    /// Number of rows on each output page.
    #[arg(long)]
    rows: u32,

    /// Output PDF file, or `-` to write to stdout.
    #[arg(short, long)]
    output: PathBuf,

    /// Overwrite output files when they already exist.
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Parser)]
struct BookletArgs {
    /// Input PDF file, or `-` to read from stdin.
    input: PathBuf,

    /// Output PDF file, or `-` to write to stdout.
    #[arg(short, long)]
    output: PathBuf,

    /// Overwrite output files when they already exist.
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Parser)]
struct PageNumbersArgs {
    /// Input PDF file, or `-` to read from stdin.
    input: PathBuf,

    /// Page range, for example `1,3-5`. Defaults to all pages.
    #[arg(long)]
    pages: Option<String>,

    /// First number written on the first selected page.
    #[arg(long, default_value_t = 1)]
    start: u32,

    /// Text before the number.
    #[arg(long, default_value = "")]
    prefix: String,

    /// Text after the number.
    #[arg(long, default_value = "")]
    suffix: String,

    /// Font size in PDF points.
    #[arg(long, default_value_t = 12.0)]
    font_size: f32,

    /// Page number placement.
    #[arg(long, value_enum, default_value_t = CliPageNumberPosition::BottomCenter)]
    position: CliPageNumberPosition,

    /// Output PDF file, or `-` to write to stdout.
    #[arg(short, long)]
    output: PathBuf,

    /// Overwrite output files when they already exist.
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum CliPageNumberPosition {
    TopLeft,
    TopCenter,
    TopRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
}

impl From<CliPageNumberPosition> for PageNumberPosition {
    fn from(value: CliPageNumberPosition) -> Self {
        match value {
            CliPageNumberPosition::TopLeft => Self::TopLeft,
            CliPageNumberPosition::TopCenter => Self::TopCenter,
            CliPageNumberPosition::TopRight => Self::TopRight,
            CliPageNumberPosition::BottomLeft => Self::BottomLeft,
            CliPageNumberPosition::BottomCenter => Self::BottomCenter,
            CliPageNumberPosition::BottomRight => Self::BottomRight,
        }
    }
}

#[derive(Debug, Parser)]
struct CompressArgs {
    /// Input PDF file, or `-` to read from stdin.
    input: PathBuf,

    /// Output PDF file, or `-` to write to stdout.
    #[arg(short, long)]
    output: PathBuf,

    /// Compression mode: lossless or lossy.
    #[arg(long, value_enum, default_value_t = CliCompressionMode::Lossless)]
    mode: CliCompressionMode,

    /// Explicit image quality for lossy image recompression, 1-100.
    #[arg(long)]
    image_quality: Option<u8>,

    /// Explicit maximum image width for lossy image resampling.
    #[arg(long)]
    image_max_width: Option<u32>,

    /// Explicit maximum image height for lossy image resampling.
    #[arg(long)]
    image_max_height: Option<u32>,

    /// Explicit target image format for lossy image recompression.
    #[arg(long, value_enum)]
    image_format: Option<CliCompressionImageFormat>,

    /// Overwrite output files when they already exist.
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum CliCompressionMode {
    Lossless,
    Lossy,
}

impl From<CliCompressionMode> for CompressionMode {
    fn from(value: CliCompressionMode) -> Self {
        match value {
            CliCompressionMode::Lossless => Self::Lossless,
            CliCompressionMode::Lossy => Self::Lossy,
        }
    }
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum CliCompressionImageFormat {
    Jpeg,
    Png,
    Webp,
}

impl From<CliCompressionImageFormat> for CompressionImageFormat {
    fn from(value: CliCompressionImageFormat) -> Self {
        match value {
            CliCompressionImageFormat::Jpeg => Self::Jpeg,
            CliCompressionImageFormat::Png => Self::Png,
            CliCompressionImageFormat::Webp => Self::Webp,
        }
    }
}

#[derive(Debug, Parser)]
struct ImageToPdfArgs {
    /// Input PNG, JPEG, or WebP image files.
    #[arg(required = true, num_args = 1..)]
    inputs: Vec<PathBuf>,

    /// Output PDF file, or `-` to write to stdout.
    #[arg(short, long)]
    output: PathBuf,

    /// Page layout: `fit` or `original_size`.
    #[arg(long)]
    layout: Option<String>,

    /// Overwrite output files when they already exist.
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Parser)]
struct SvgToPdfArgs {
    /// Input SVG file, or `-` to read from stdin.
    input: PathBuf,

    /// Output PDF file, or `-` to write to stdout.
    #[arg(short, long)]
    output: PathBuf,

    /// Rasterize the SVG before placing it into the PDF.
    #[arg(long)]
    rasterize: bool,

    /// Overwrite output files when they already exist.
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Parser)]
struct RenderArgs {
    /// Input PDF file, or `-` to read from stdin.
    input: PathBuf,

    /// One-based page number to render.
    #[arg(long)]
    page: u32,

    /// Render scale. For 144 DPI output from a 72 DPI PDF, use 2.0.
    #[arg(long)]
    scale: Option<f32>,

    /// Output PNG file, or `-` to write to stdout.
    #[arg(short, long)]
    output: PathBuf,

    /// Overwrite output files when they already exist.
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Parser)]
struct ExtractTextArgs {
    /// Input PDF file, or `-` to read from stdin.
    input: PathBuf,

    /// Output text file, or `-` to write to stdout.
    #[arg(short, long)]
    output: PathBuf,

    /// Overwrite output files when they already exist.
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Parser)]
struct WatermarkArgs {
    /// Input PDF file, or `-` to read from stdin.
    input: PathBuf,

    /// Watermark kind: `text`, `image`, or `svg`.
    #[arg(long)]
    kind: String,

    /// Text content for text watermarks.
    #[arg(long)]
    text: Option<String>,

    /// Font family for text watermarks.
    #[arg(long)]
    font: Option<String>,

    /// Explicit font file for text watermarks.
    #[arg(long)]
    font_path: Option<PathBuf>,

    /// Font size in points for text watermarks.
    #[arg(long)]
    font_size: Option<f32>,

    /// Image or SVG watermark file.
    #[arg(long)]
    watermark: Option<PathBuf>,

    /// Page range, for example `1,3-5`.
    #[arg(long)]
    pages: Option<String>,

    /// Opacity from 0.0 to 1.0.
    #[arg(long)]
    opacity: Option<f32>,

    /// Rotation in degrees.
    #[arg(long)]
    rotation: Option<f32>,

    /// Position: `center`, `top_left`, `top_right`, `bottom_left`, or `bottom_right`.
    #[arg(long)]
    position: Option<String>,

    /// Scale for image and SVG watermarks.
    #[arg(long)]
    scale: Option<f32>,

    /// Rasterize SVG before watermarking.
    #[arg(long)]
    rasterize: bool,

    /// Output PDF file, or `-` to write to stdout.
    #[arg(short, long)]
    output: PathBuf,

    /// Overwrite output files when they already exist.
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Parser)]
struct VerifySignaturesArgs {
    /// Input PDF file, or `-` to read from stdin.
    input: PathBuf,

    /// PEM file containing explicit trust anchors.
    #[arg(long)]
    trust_anchors: PathBuf,

    /// Output report file, or `-` to write to stdout.
    #[arg(short, long)]
    output: PathBuf,

    /// Overwrite output files when they already exist.
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Parser)]
struct PdfSecurityArgs {
    /// Explicit security operation name.
    #[arg(long)]
    operation: String,

    /// Input PDF file, or `-` to read from stdin.
    input: PathBuf,

    /// Output report file, or `-` to write to stdout.
    #[arg(short, long)]
    output: PathBuf,

    /// Overwrite output files when they already exist.
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Parser)]
struct PdfCompareArgs {
    /// Explicit compare mode.
    #[arg(long)]
    mode: String,

    /// Left input PDF file.
    left: PathBuf,

    /// Right input PDF file.
    right: PathBuf,

    /// Output report file, or `-` to write to stdout.
    #[arg(short, long)]
    output: PathBuf,

    /// Overwrite output files when they already exist.
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Parser)]
struct InspectOutputArgs {
    /// Input PDF file, or `-` to read from stdin.
    input: PathBuf,

    /// Output JSON file, attachment file, or `-` to write to stdout.
    #[arg(short, long)]
    output: PathBuf,

    /// Overwrite output files when they already exist.
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Parser)]
struct EditOutputArgs {
    /// Input PDF file, or `-` to read from stdin.
    input: PathBuf,

    /// Output PDF file, or `-` to write to stdout.
    #[arg(short, long)]
    output: PathBuf,

    /// Overwrite output files when they already exist.
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Parser)]
struct MetadataSetArgs {
    /// Input PDF file, or `-` to read from stdin.
    input: PathBuf,

    /// Metadata entry in key=value form. May be repeated.
    #[arg(long = "entry", required = true)]
    entries: Vec<String>,

    /// Output PDF file, or `-` to write to stdout.
    #[arg(short, long)]
    output: PathBuf,

    /// Overwrite output files when they already exist.
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Parser)]
struct MetadataDeleteArgs {
    /// Input PDF file, or `-` to read from stdin.
    input: PathBuf,

    /// Metadata key to delete. May be repeated.
    #[arg(long = "key", required = true)]
    keys: Vec<String>,

    /// Output PDF file, or `-` to write to stdout.
    #[arg(short, long)]
    output: PathBuf,

    /// Overwrite output files when they already exist.
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Parser)]
struct OutlineSetArgs {
    /// Input PDF file, or `-` to read from stdin.
    input: PathBuf,

    /// JSON file containing an OutlineTree, or `-` to read from stdin.
    #[arg(long)]
    tree: PathBuf,

    /// Output PDF file, or `-` to write to stdout.
    #[arg(short, long)]
    output: PathBuf,

    /// Overwrite output files when they already exist.
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Parser)]
struct AttachAddArgs {
    /// Input PDF file, or `-` to read from stdin.
    input: PathBuf,

    /// File to embed.
    file: PathBuf,

    /// Attachment name stored in the PDF. Defaults to the file name.
    #[arg(long)]
    name: Option<String>,

    /// Attachment description.
    #[arg(long)]
    description: Option<String>,

    /// Output PDF file, or `-` to write to stdout.
    #[arg(short, long)]
    output: PathBuf,

    /// Overwrite output files when they already exist.
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Parser)]
struct AttachExtractArgs {
    /// Input PDF file, or `-` to read from stdin.
    input: PathBuf,

    /// Attachment name to extract.
    #[arg(long)]
    name: String,

    /// Output file, or `-` to write to stdout.
    #[arg(short, long)]
    output: PathBuf,

    /// Overwrite output files when they already exist.
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Parser)]
struct AttachDeleteArgs {
    /// Input PDF file, or `-` to read from stdin.
    input: PathBuf,

    /// Attachment name to delete.
    #[arg(long)]
    name: String,

    /// Output PDF file, or `-` to write to stdout.
    #[arg(short, long)]
    output: PathBuf,

    /// Overwrite output files when they already exist.
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Parser)]
struct AnnotAddArgs {
    /// Input PDF file, or `-` to read from stdin.
    input: PathBuf,

    /// One-based page number.
    #[arg(long)]
    page: u32,

    /// Stable annotation id.
    #[arg(long)]
    id: String,

    /// Text annotation contents.
    #[arg(long)]
    text: String,

    /// Output PDF file, or `-` to write to stdout.
    #[arg(short, long)]
    output: PathBuf,

    /// Overwrite output files when they already exist.
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Parser)]
struct AnnotDeleteArgs {
    /// Input PDF file, or `-` to read from stdin.
    input: PathBuf,

    /// Stable annotation id.
    #[arg(long)]
    id: String,

    /// Output PDF file, or `-` to write to stdout.
    #[arg(short, long)]
    output: PathBuf,

    /// Overwrite output files when they already exist.
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Parser)]
struct FormFillArgs {
    /// Input PDF file, or `-` to read from stdin.
    input: PathBuf,

    /// Field value in name=value form. May be repeated.
    #[arg(long = "field", required = true)]
    fields: Vec<String>,

    /// Output PDF file, or `-` to write to stdout.
    #[arg(short, long)]
    output: PathBuf,

    /// Overwrite output files when they already exist.
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Parser)]
struct InteractiveRemoveArgs {
    /// Input PDF file, or `-` to read from stdin.
    input: PathBuf,

    #[arg(long)]
    annotations: bool,
    #[arg(long)]
    forms: bool,
    #[arg(long)]
    actions: bool,
    #[arg(long)]
    javascript: bool,
    #[arg(long = "embedded-files")]
    embedded_files: bool,

    /// Output PDF file, or `-` to write to stdout.
    #[arg(short, long)]
    output: PathBuf,

    /// Overwrite output files when they already exist.
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Parser)]
struct StampArgs {
    /// Input PDF file, or `-` to read from stdin.
    input: PathBuf,
    #[arg(long)]
    text: String,
    #[arg(long)]
    font: Option<String>,
    #[arg(long)]
    font_path: Option<PathBuf>,
    #[arg(long)]
    font_size: Option<f32>,
    #[arg(long)]
    pages: Option<String>,
    #[arg(long)]
    opacity: Option<f32>,
    #[arg(long)]
    rotation: Option<f32>,
    #[arg(long)]
    position: Option<String>,
    #[arg(short, long)]
    output: PathBuf,
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Parser)]
struct SignatureAppearanceArgs {
    /// Input PDF file, or `-` to read from stdin.
    input: PathBuf,
    #[arg(long)]
    text: String,
    #[arg(long)]
    font: Option<String>,
    #[arg(long)]
    font_path: Option<PathBuf>,
    #[arg(long)]
    font_size: Option<f32>,
    #[arg(long)]
    pages: Option<String>,
    #[arg(long)]
    position: Option<String>,
    #[arg(short, long)]
    output: PathBuf,
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Parser)]
struct OverlayPdfArgs {
    /// Input PDF file, or `-` to read from stdin.
    input: PathBuf,
    /// Overlay PDF file.
    overlay: PathBuf,
    #[arg(long)]
    source_page: Option<u32>,
    #[arg(long)]
    pages: Option<String>,
    #[arg(long)]
    opacity: Option<f32>,
    #[arg(long)]
    scale: Option<f32>,
    #[arg(long)]
    position: Option<String>,
    #[arg(short, long)]
    output: PathBuf,
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Parser)]
struct ImageAddArgs {
    input: PathBuf,
    image: PathBuf,
    #[arg(long)]
    name: String,
    #[arg(long)]
    page: u32,
    #[arg(short, long)]
    output: PathBuf,
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Parser)]
struct ImageReplaceArgs {
    input: PathBuf,
    image: PathBuf,
    #[arg(long)]
    name: String,
    #[arg(short, long)]
    output: PathBuf,
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Parser)]
struct ImageDeleteArgs {
    input: PathBuf,
    #[arg(long)]
    name: String,
    #[arg(short, long)]
    output: PathBuf,
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Parser)]
struct ImageExtractArgs {
    input: PathBuf,
    #[arg(long)]
    name: String,
    #[arg(short, long)]
    output: PathBuf,
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Parser)]
struct ColorEditArgs {
    input: PathBuf,
    #[arg(long)]
    pages: Option<String>,
    #[arg(short, long)]
    output: PathBuf,
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Parser)]
struct ColorContrastArgs {
    input: PathBuf,
    #[arg(long)]
    factor: f32,
    #[arg(long)]
    pages: Option<String>,
    #[arg(short, long)]
    output: PathBuf,
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Parser)]
struct ColorReplaceArgs {
    input: PathBuf,
    #[arg(long)]
    from: String,
    #[arg(long)]
    to: String,
    #[arg(long)]
    pages: Option<String>,
    #[arg(short, long)]
    output: PathBuf,
    #[arg(long)]
    force: bool,
}

/// Parses CLI arguments and runs the requested command.
pub fn run() -> i32 {
    let args = std::env::args_os().collect::<Vec<_>>();
    let stdin_buffer = match stdin_for_args(args.clone()) {
        Ok(buffer) => buffer,
        Err(error) => {
            let _ = writeln!(io::stderr().lock(), "oxidepdf: {error}");
            return error.exit_code();
        }
    };
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    let stderr = io::stderr();
    let mut stderr = stderr.lock();

    run_with_io(args, &stdin_buffer, &mut stdout, &mut stderr)
}

/// Runs the CLI with injectable IO for tests.
pub fn run_with_io<I, S>(
    args: I,
    stdin: impl AsRef<[u8]>,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> i32
where
    I: IntoIterator<Item = S>,
    S: Into<std::ffi::OsString> + Clone,
{
    match run_with_io_result(args, stdin.as_ref(), stdout) {
        Ok(()) => 0,
        Err(error) => {
            let _ = writeln!(stderr, "oxidepdf: {error}");
            error.exit_code()
        }
    }
}

/// Returns the clap command definition for tests and generated help.
pub fn command() -> clap::Command {
    Cli::command()
}

fn stdin_for_args<I, S>(args: I) -> Result<Vec<u8>, CliError>
where
    I: IntoIterator<Item = S>,
    S: Into<std::ffi::OsString> + Clone,
{
    let cli = Cli::try_parse_from(args).map_err(CliError::Arguments)?;
    if cli_reads_stdin(&cli) {
        let mut stdin_buffer = Vec::new();
        io::stdin()
            .lock()
            .read_to_end(&mut stdin_buffer)
            .map_err(CliError::Input)?;
        Ok(stdin_buffer)
    } else {
        Ok(Vec::new())
    }
}

fn cli_reads_stdin(cli: &Cli) -> bool {
    match &cli.command {
        Some(Commands::Run(args)) => is_stdio(&args.workflow),
        Some(Commands::PdfEdit(command)) => pdf_edit_reads_stdin(command),
        Some(Commands::PdfInspect(command)) => pdf_inspect_reads_stdin(command),
        Some(Commands::PdfSecurity(args)) => is_stdio(&args.input),
        Some(Commands::PdfCompare(args)) => is_stdio(&args.left) || is_stdio(&args.right),
        Some(Commands::PdfSign(command)) => pdf_sign_reads_stdin(command),
        Some(Commands::Metadata(command)) => metadata_reads_stdin(command),
        Some(Commands::Outline(command)) => outline_reads_stdin(command),
        Some(Commands::Attach(command)) => attach_reads_stdin(command),
        Some(Commands::Annot(command)) => annot_reads_stdin(command),
        Some(Commands::Form(command)) => form_reads_stdin(command),
        Some(Commands::Interactive(command)) => interactive_reads_stdin(command),
        Some(Commands::Stamp(args)) => is_stdio(&args.input),
        Some(Commands::SignatureAppearance(args)) => is_stdio(&args.input),
        Some(Commands::OverlayPdf(args)) => is_stdio(&args.input) || is_stdio(&args.overlay),
        Some(Commands::Image(command)) => image_reads_stdin(command),
        Some(Commands::Color(command)) => color_reads_stdin(command),
        Some(Commands::Compress(args)) => is_stdio(&args.input),
        None => false,
    }
}

fn pdf_edit_reads_stdin(command: &PdfEditCommand) -> bool {
    match command {
        PdfEditCommand::Merge(args) => args.inputs.iter().any(|input| is_stdio(input)),
        PdfEditCommand::KeepPages(args)
        | PdfEditCommand::ExtractPages(args)
        | PdfEditCommand::ReorderPages(args) => is_stdio(&args.input),
        PdfEditCommand::RotatePages(args) => is_stdio(&args.input),
        PdfEditCommand::DeletePages(args) => is_stdio(&args.input),
        PdfEditCommand::DeleteBlankPages(args) => is_stdio(&args.input),
        PdfEditCommand::CropPages(args) => is_stdio(&args.input),
        PdfEditCommand::ScalePages(args) => is_stdio(&args.input),
        PdfEditCommand::SinglePage(args) => is_stdio(&args.input),
        PdfEditCommand::NUp(args) => is_stdio(&args.input),
        PdfEditCommand::Booklet(args) => is_stdio(&args.input),
        PdfEditCommand::PageNumbers(args) => is_stdio(&args.input),
        PdfEditCommand::Img2pdf(args) => args.inputs.iter().any(|input| is_stdio(input)),
        PdfEditCommand::Svg2pdf(args) => is_stdio(&args.input),
        PdfEditCommand::Watermark(args) => {
            is_stdio(&args.input) || args.watermark.as_ref().is_some_and(|path| is_stdio(path))
        }
        PdfEditCommand::Compress(args) => is_stdio(&args.input),
    }
}

fn pdf_inspect_reads_stdin(command: &PdfInspectCommand) -> bool {
    match command {
        PdfInspectCommand::Render(args) => is_stdio(&args.input),
        PdfInspectCommand::ExtractText(args) => is_stdio(&args.input),
    }
}

fn pdf_sign_reads_stdin(command: &PdfSignCommand) -> bool {
    match command {
        PdfSignCommand::Verify(args) => is_stdio(&args.input),
    }
}

fn metadata_reads_stdin(command: &MetadataCommand) -> bool {
    match command {
        MetadataCommand::Get(args) | MetadataCommand::Validate(args) => is_stdio(&args.input),
        MetadataCommand::Set(args) => is_stdio(&args.input),
        MetadataCommand::Delete(args) => is_stdio(&args.input),
    }
}

fn outline_reads_stdin(command: &OutlineCommand) -> bool {
    match command {
        OutlineCommand::Get(args) => is_stdio(&args.input),
        OutlineCommand::Set(args) => is_stdio(&args.input) || is_stdio(&args.tree),
        OutlineCommand::Delete(args) => is_stdio(&args.input),
    }
}

fn attach_reads_stdin(command: &AttachCommand) -> bool {
    match command {
        AttachCommand::Add(args) => is_stdio(&args.input) || is_stdio(&args.file),
        AttachCommand::List(args) => is_stdio(&args.input),
        AttachCommand::Extract(args) => is_stdio(&args.input),
        AttachCommand::Delete(args) => is_stdio(&args.input),
    }
}

fn annot_reads_stdin(command: &AnnotCommand) -> bool {
    match command {
        AnnotCommand::List(args) => is_stdio(&args.input),
        AnnotCommand::Add(args) => is_stdio(&args.input),
        AnnotCommand::Delete(args) => is_stdio(&args.input),
    }
}

fn form_reads_stdin(command: &FormCommand) -> bool {
    match command {
        FormCommand::Inspect(args) => is_stdio(&args.input),
        FormCommand::Fill(args) => is_stdio(&args.input),
        FormCommand::UnlockReadonly(args) | FormCommand::Remove(args) => is_stdio(&args.input),
    }
}

fn interactive_reads_stdin(command: &InteractiveCommand) -> bool {
    match command {
        InteractiveCommand::Remove(args) => is_stdio(&args.input),
    }
}

fn image_reads_stdin(command: &ImageCommand) -> bool {
    match command {
        ImageCommand::List(args) => is_stdio(&args.input),
        ImageCommand::Add(args) => is_stdio(&args.input) || is_stdio(&args.image),
        ImageCommand::Replace(args) => is_stdio(&args.input) || is_stdio(&args.image),
        ImageCommand::Delete(args) => is_stdio(&args.input),
        ImageCommand::Extract(args) => is_stdio(&args.input),
    }
}

fn color_reads_stdin(command: &ColorCommand) -> bool {
    match command {
        ColorCommand::Contrast(args) => is_stdio(&args.input),
        ColorCommand::Invert(args) => is_stdio(&args.input),
        ColorCommand::Replace(args) => is_stdio(&args.input),
    }
}

fn run_with_io_result<I, S>(args: I, stdin: &[u8], stdout: &mut impl Write) -> Result<(), CliError>
where
    I: IntoIterator<Item = S>,
    S: Into<std::ffi::OsString> + Clone,
{
    let cli = Cli::try_parse_from(args).map_err(CliError::Arguments)?;
    match cli.command {
        Some(Commands::Run(args)) => run_workflow(args, stdin, stdout),
        Some(Commands::PdfEdit(command)) => run_pdf_edit(command, stdin, stdout),
        Some(Commands::PdfInspect(command)) => run_pdf_inspect(command, stdin, stdout),
        Some(Commands::PdfSecurity(args)) => run_pdf_security(args, stdin, stdout),
        Some(Commands::PdfCompare(args)) => run_pdf_compare(args, stdin, stdout),
        Some(Commands::PdfSign(command)) => run_pdf_sign(command, stdin, stdout),
        Some(Commands::Metadata(command)) => run_metadata(command, stdin, stdout),
        Some(Commands::Outline(command)) => run_outline(command, stdin, stdout),
        Some(Commands::Attach(command)) => run_attach(command, stdin, stdout),
        Some(Commands::Annot(command)) => run_annot(command, stdin, stdout),
        Some(Commands::Form(command)) => run_form(command, stdin, stdout),
        Some(Commands::Interactive(command)) => run_interactive(command, stdin, stdout),
        Some(Commands::Stamp(args)) => run_stamp(args, stdin, stdout),
        Some(Commands::SignatureAppearance(args)) => run_signature_appearance(args, stdin, stdout),
        Some(Commands::OverlayPdf(args)) => run_overlay_pdf(args, stdin, stdout),
        Some(Commands::Image(command)) => run_image(command, stdin, stdout),
        Some(Commands::Color(command)) => run_color(command, stdin, stdout),
        Some(Commands::Compress(args)) => run_compress(args, stdin, stdout),
        None => Ok(()),
    }
}

fn run_pdf_edit(
    command: PdfEditCommand,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    match command {
        PdfEditCommand::Merge(args) => run_merge(args, stdin, stdout),
        PdfEditCommand::KeepPages(args) => {
            run_page_selection(args, stdin, stdout, PageCommand::KeepPages)
        }
        PdfEditCommand::ExtractPages(args) => {
            run_page_selection(args, stdin, stdout, PageCommand::ExtractPages)
        }
        PdfEditCommand::ReorderPages(args) => {
            run_page_selection(args, stdin, stdout, PageCommand::ReorderPages)
        }
        PdfEditCommand::RotatePages(args) => run_rotate(args, stdin, stdout),
        PdfEditCommand::DeletePages(args) => run_delete_pages(args, stdin, stdout),
        PdfEditCommand::DeleteBlankPages(args) => run_delete_blank_pages(args, stdin, stdout),
        PdfEditCommand::CropPages(args) => run_crop_pages(args, stdin, stdout),
        PdfEditCommand::ScalePages(args) => run_scale_pages(args, stdin, stdout),
        PdfEditCommand::SinglePage(args) => run_single_page(args, stdin, stdout),
        PdfEditCommand::NUp(args) => run_nup(args, stdin, stdout),
        PdfEditCommand::Booklet(args) => run_booklet(args, stdin, stdout),
        PdfEditCommand::PageNumbers(args) => run_page_numbers(args, stdin, stdout),
        PdfEditCommand::Img2pdf(args) => run_img2pdf(args, stdin, stdout),
        PdfEditCommand::Svg2pdf(args) => run_svg2pdf(args, stdin, stdout),
        PdfEditCommand::Watermark(args) => run_watermark(args, stdin, stdout),
        PdfEditCommand::Compress(args) => run_compress(args, stdin, stdout),
    }
}

fn run_pdf_inspect(
    command: PdfInspectCommand,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    match command {
        PdfInspectCommand::Render(args) => run_render(args, stdin, stdout),
        PdfInspectCommand::ExtractText(args) => run_extract_text(args, stdin, stdout),
    }
}

fn run_pdf_sign(
    command: PdfSignCommand,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    match command {
        PdfSignCommand::Verify(args) => run_verify_signatures(args, stdin, stdout),
    }
}

fn run_workflow(args: RunArgs, stdin: &[u8], stdout: &mut impl Write) -> Result<(), CliError> {
    let workflow_bytes = read_path_or_stdin(&args.workflow, stdin).map_err(CliError::Input)?;
    let workflow = parse_workflow(&workflow_bytes, &args.workflow)?;
    let store = load_inputs(&workflow, stdin)?;
    let mut runner = PdfOperatorRunner::with_limits(workflow.limits.clone());
    let result = execute_workflow(&workflow, store, &mut runner).map_err(CliError::Core)?;
    write_outputs(&workflow, &result.store, args.force, stdout)?;

    Ok(())
}

fn run_merge(args: MergeArgs, stdin: &[u8], stdout: &mut impl Write) -> Result<(), CliError> {
    let input_refs = (0..args.inputs.len())
        .map(|index| ArtifactRef::new(format!("input_{index}")))
        .collect::<Vec<_>>();
    let workflow = Workflow {
        version: WorkflowVersion::V1,
        inputs: args
            .inputs
            .into_iter()
            .zip(input_refs.iter())
            .map(|(path, id)| oxidepdf_core::InputSpec {
                id: id.clone(),
                path,
            })
            .collect(),
        tasks: vec![TaskSpec {
            id: TaskId::new("merge"),
            op: OperatorSpec::PdfEdit(PdfEditOptions::Merge(MergeOptions {})),
            inputs: input_refs,
        }],
        outputs: vec![oxidepdf_core::OutputSpec {
            id: ArtifactRef::new("output"),
            from: ArtifactRef::new("merge"),
            path: args.output,
        }],
        limits: Default::default(),
        metadata: WorkflowMetadata::default(),
    };
    execute_and_write_workflow(workflow, stdin, args.force, stdout)
}

fn run_page_selection(
    args: PageSelectionArgs,
    stdin: &[u8],
    stdout: &mut impl Write,
    command: PageCommand,
) -> Result<(), CliError> {
    let task_id = match command {
        PageCommand::KeepPages => "keep_pages",
        PageCommand::ExtractPages => "extract_pages",
        PageCommand::ReorderPages => "reorder_pages",
    };
    let op = match command {
        PageCommand::KeepPages => OperatorSpec::PdfEdit(PdfEditOptions::KeepPages(SplitOptions {
            pages: args.pages,
        })),
        PageCommand::ExtractPages => {
            OperatorSpec::PdfEdit(PdfEditOptions::ExtractPages(PageSelectionOptions {
                pages: args.pages,
            }))
        }
        PageCommand::ReorderPages => {
            OperatorSpec::PdfEdit(PdfEditOptions::ReorderPages(ReorderOptions {
                pages: args.pages,
            }))
        }
    };
    let workflow = one_input_workflow(args.input, args.output, task_id, op);

    execute_and_write_workflow(workflow, stdin, args.force, stdout)
}

fn run_rotate(args: RotateArgs, stdin: &[u8], stdout: &mut impl Write) -> Result<(), CliError> {
    let workflow = one_input_workflow(
        args.input,
        args.output,
        "rotate_pages",
        OperatorSpec::PdfEdit(PdfEditOptions::RotatePages(RotateOptions {
            pages: args.pages,
            degrees: args.degrees,
        })),
    );

    execute_and_write_workflow(workflow, stdin, args.force, stdout)
}

fn run_delete_pages(
    args: PageSelectionArgs,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    let workflow = one_input_workflow(
        args.input,
        args.output,
        "delete_pages",
        OperatorSpec::PdfEdit(PdfEditOptions::DeletePages(PageSelectionOptions {
            pages: args.pages,
        })),
    );

    execute_and_write_workflow(workflow, stdin, args.force, stdout)
}

fn run_delete_blank_pages(
    args: DeleteBlankPagesArgs,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    let workflow = one_input_workflow(
        args.input,
        args.output,
        "delete_blank_pages",
        OperatorSpec::PdfEdit(PdfEditOptions::DeleteBlankPages(
            DeleteBlankPagesOptions::default(),
        )),
    );

    execute_and_write_workflow(workflow, stdin, args.force, stdout)
}

fn run_compress(args: CompressArgs, stdin: &[u8], stdout: &mut impl Write) -> Result<(), CliError> {
    let images = compression_image_options(&args);
    let workflow = one_input_workflow(
        args.input,
        args.output,
        "compress",
        OperatorSpec::PdfEdit(PdfEditOptions::Compression(CompressionOptions {
            mode: args.mode.into(),
            images,
        })),
    );

    execute_and_write_workflow(workflow, stdin, args.force, stdout)
}

fn compression_image_options(args: &CompressArgs) -> Option<CompressionImageOptions> {
    if args.image_quality.is_none()
        && args.image_max_width.is_none()
        && args.image_max_height.is_none()
        && args.image_format.is_none()
    {
        return None;
    }

    Some(CompressionImageOptions {
        quality: args.image_quality,
        max_width: args.image_max_width,
        max_height: args.image_max_height,
        format: args.image_format.map(Into::into),
    })
}

fn run_crop_pages(
    args: CropPagesArgs,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    let workflow = one_input_workflow(
        args.input,
        args.output,
        "crop_pages",
        OperatorSpec::PdfEdit(PdfEditOptions::CropPages(CropPagesOptions {
            pages: args.pages,
            left: args.left,
            bottom: args.bottom,
            right: args.right,
            top: args.top,
        })),
    );

    execute_and_write_workflow(workflow, stdin, args.force, stdout)
}

fn run_scale_pages(
    args: ScalePagesArgs,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    let workflow = one_input_workflow(
        args.input,
        args.output,
        "scale_pages",
        OperatorSpec::PdfEdit(PdfEditOptions::ScalePages(ScalePagesOptions {
            pages: args.pages,
            factor: args.factor,
        })),
    );

    execute_and_write_workflow(workflow, stdin, args.force, stdout)
}

fn run_single_page(
    args: SinglePageArgs,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    let workflow = one_input_workflow(
        args.input,
        args.output,
        "single_page",
        OperatorSpec::PdfEdit(PdfEditOptions::SinglePage(SinglePageOptions::default())),
    );

    execute_and_write_workflow(workflow, stdin, args.force, stdout)
}

fn run_nup(args: NUpArgs, stdin: &[u8], stdout: &mut impl Write) -> Result<(), CliError> {
    let workflow = one_input_workflow(
        args.input,
        args.output,
        "nup",
        OperatorSpec::PdfEdit(PdfEditOptions::NUp(NUpOptions {
            columns: args.columns,
            rows: args.rows,
        })),
    );

    execute_and_write_workflow(workflow, stdin, args.force, stdout)
}

fn run_booklet(args: BookletArgs, stdin: &[u8], stdout: &mut impl Write) -> Result<(), CliError> {
    let workflow = one_input_workflow(
        args.input,
        args.output,
        "booklet",
        OperatorSpec::PdfEdit(PdfEditOptions::Booklet(BookletOptions::default())),
    );

    execute_and_write_workflow(workflow, stdin, args.force, stdout)
}

fn run_page_numbers(
    args: PageNumbersArgs,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    let workflow = one_input_workflow(
        args.input,
        args.output,
        "page_numbers",
        OperatorSpec::PdfEdit(PdfEditOptions::PageNumbers(PageNumbersOptions {
            pages: args.pages,
            start: args.start,
            prefix: args.prefix,
            suffix: args.suffix,
            font_size: args.font_size,
            position: args.position.into(),
        })),
    );

    execute_and_write_workflow(workflow, stdin, args.force, stdout)
}

fn run_img2pdf(
    args: ImageToPdfArgs,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    let input_refs = (0..args.inputs.len())
        .map(|index| ArtifactRef::new(format!("input_{index}")))
        .collect::<Vec<_>>();
    let workflow = Workflow {
        version: WorkflowVersion::V1,
        inputs: args
            .inputs
            .into_iter()
            .zip(input_refs.iter())
            .map(|(path, id)| oxidepdf_core::InputSpec {
                id: id.clone(),
                path,
            })
            .collect(),
        tasks: vec![TaskSpec {
            id: TaskId::new("img2pdf"),
            op: OperatorSpec::PdfEdit(PdfEditOptions::ImageToPdf(ImageToPdfOptions {
                layout: args.layout,
            })),
            inputs: input_refs,
        }],
        outputs: vec![oxidepdf_core::OutputSpec {
            id: ArtifactRef::new("output"),
            from: ArtifactRef::new("img2pdf"),
            path: args.output,
        }],
        limits: Default::default(),
        metadata: WorkflowMetadata::default(),
    };

    execute_and_write_workflow(workflow, stdin, args.force, stdout)
}

fn run_svg2pdf(args: SvgToPdfArgs, stdin: &[u8], stdout: &mut impl Write) -> Result<(), CliError> {
    let workflow = one_input_workflow(
        args.input,
        args.output,
        "svg2pdf",
        OperatorSpec::PdfEdit(PdfEditOptions::SvgToPdf(SvgToPdfOptions {
            rasterize: args.rasterize,
        })),
    );

    execute_and_write_workflow(workflow, stdin, args.force, stdout)
}

fn run_render(args: RenderArgs, stdin: &[u8], stdout: &mut impl Write) -> Result<(), CliError> {
    let workflow = one_input_workflow(
        args.input,
        args.output,
        "render",
        OperatorSpec::PdfInspect(PdfInspectOptions::Render(RenderOptions {
            page: args.page,
            format: Some("png".to_owned()),
            scale: args.scale,
        })),
    );

    execute_and_write_workflow(workflow, stdin, args.force, stdout)
}

fn run_extract_text(
    args: ExtractTextArgs,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    let workflow = one_input_workflow(
        args.input,
        args.output,
        "extract_text",
        OperatorSpec::PdfInspect(PdfInspectOptions::ExtractText(ExtractTextOptions {
            format: Some("plain".to_owned()),
        })),
    );

    execute_and_write_workflow(workflow, stdin, args.force, stdout)
}

fn run_watermark(
    args: WatermarkArgs,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    let kind = parse_watermark_kind(&args.kind)?;
    let mut input_specs = vec![oxidepdf_core::InputSpec {
        id: ArtifactRef::new("input"),
        path: args.input,
    }];
    let mut task_inputs = vec![ArtifactRef::new("input")];
    if matches!(kind, WatermarkKind::Image | WatermarkKind::Svg) {
        let watermark = args.watermark.ok_or_else(|| {
            CliError::Workflow("image and SVG watermarks require --watermark".to_owned())
        })?;
        input_specs.push(oxidepdf_core::InputSpec {
            id: ArtifactRef::new("watermark_input"),
            path: watermark,
        });
        task_inputs.push(ArtifactRef::new("watermark_input"));
    }

    let workflow = Workflow {
        version: WorkflowVersion::V1,
        inputs: input_specs,
        tasks: vec![TaskSpec {
            id: TaskId::new("watermark"),
            op: OperatorSpec::PdfEdit(PdfEditOptions::Watermark(WatermarkOptions {
                kind,
                text: args.text,
                font: args.font,
                font_path: args.font_path,
                font_size: args.font_size,
                opacity: args.opacity,
                rotation: args.rotation,
                position: args.position,
                pages: args.pages,
                scale: args.scale,
                rasterize: args.rasterize,
            })),
            inputs: task_inputs,
        }],
        outputs: vec![oxidepdf_core::OutputSpec {
            id: ArtifactRef::new("output"),
            from: ArtifactRef::new("watermark"),
            path: args.output,
        }],
        limits: Default::default(),
        metadata: WorkflowMetadata::default(),
    };

    execute_and_write_workflow(workflow, stdin, args.force, stdout)
}

fn run_verify_signatures(
    args: VerifySignaturesArgs,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    let workflow = one_input_workflow(
        args.input,
        args.output,
        "verify_signature",
        OperatorSpec::PdfSign(PdfSignOptions::Verify(SignatureOptions {
            mode: Default::default(),
            trust_anchors: Some(args.trust_anchors),
        })),
    );

    execute_and_write_workflow(workflow, stdin, args.force, stdout)
}

fn run_pdf_security(
    args: PdfSecurityArgs,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    let workflow = one_input_workflow(
        args.input,
        args.output,
        "pdf_security",
        OperatorSpec::PdfSecurity(PdfSecurityOptions {
            operation: args.operation,
        }),
    );

    execute_and_write_workflow(workflow, stdin, args.force, stdout)
}

fn run_pdf_compare(
    args: PdfCompareArgs,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    let workflow = Workflow {
        version: WorkflowVersion::V1,
        inputs: vec![
            oxidepdf_core::InputSpec {
                id: ArtifactRef::new("left"),
                path: args.left,
            },
            oxidepdf_core::InputSpec {
                id: ArtifactRef::new("right"),
                path: args.right,
            },
        ],
        tasks: vec![TaskSpec {
            id: TaskId::new("pdf_compare"),
            op: OperatorSpec::PdfCompare(PdfCompareOptions { mode: args.mode }),
            inputs: vec![ArtifactRef::new("left"), ArtifactRef::new("right")],
        }],
        outputs: vec![oxidepdf_core::OutputSpec {
            id: ArtifactRef::new("output"),
            from: ArtifactRef::new("pdf_compare"),
            path: args.output,
        }],
        limits: Default::default(),
        metadata: WorkflowMetadata::default(),
    };

    execute_and_write_workflow(workflow, stdin, args.force, stdout)
}

fn run_metadata(
    command: MetadataCommand,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    match command {
        MetadataCommand::Get(args) => execute_and_write_workflow(
            one_input_workflow(
                args.input,
                args.output,
                "metadata_get",
                OperatorSpec::PdfInspect(PdfInspectOptions::Metadata(
                    MetadataInspectOptions::default(),
                )),
            ),
            stdin,
            args.force,
            stdout,
        ),
        MetadataCommand::Set(args) => execute_and_write_workflow(
            one_input_workflow(
                args.input,
                args.output,
                "metadata_set",
                OperatorSpec::PdfEdit(PdfEditOptions::Metadata(MetadataEditOptions {
                    action: MetadataEditAction::Set,
                    entries: parse_metadata_entries(args.entries)?,
                    keys: Vec::new(),
                })),
            ),
            stdin,
            args.force,
            stdout,
        ),
        MetadataCommand::Delete(args) => execute_and_write_workflow(
            one_input_workflow(
                args.input,
                args.output,
                "metadata_delete",
                OperatorSpec::PdfEdit(PdfEditOptions::Metadata(MetadataEditOptions {
                    action: MetadataEditAction::Delete,
                    entries: Vec::new(),
                    keys: args.keys,
                })),
            ),
            stdin,
            args.force,
            stdout,
        ),
        MetadataCommand::Validate(args) => execute_and_write_workflow(
            one_input_workflow(
                args.input,
                args.output,
                "metadata_validate",
                OperatorSpec::PdfInspect(PdfInspectOptions::Metadata(
                    MetadataInspectOptions::default(),
                )),
            ),
            stdin,
            args.force,
            stdout,
        ),
    }
}

fn run_outline(
    command: OutlineCommand,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    match command {
        OutlineCommand::Get(args) => execute_and_write_workflow(
            one_input_workflow(
                args.input,
                args.output,
                "outline_get",
                OperatorSpec::PdfInspect(PdfInspectOptions::Outline(
                    OutlineInspectOptions::default(),
                )),
            ),
            stdin,
            args.force,
            stdout,
        ),
        OutlineCommand::Set(args) => {
            reject_shared_stdin_inputs(&args.input, &args.tree)?;
            let tree_bytes = read_path_or_stdin(&args.tree, stdin).map_err(CliError::Input)?;
            let tree: OutlineTree = serde_json::from_slice(&tree_bytes)
                .map_err(|error| CliError::Workflow(error.to_string()))?;
            execute_and_write_workflow(
                one_input_workflow(
                    args.input,
                    args.output,
                    "outline_set",
                    OperatorSpec::PdfEdit(PdfEditOptions::Outline(OutlineEditOptions {
                        action: OutlineEditAction::Set,
                        tree: Some(tree),
                    })),
                ),
                stdin,
                args.force,
                stdout,
            )
        }
        OutlineCommand::Delete(args) => execute_and_write_workflow(
            one_input_workflow(
                args.input,
                args.output,
                "outline_delete",
                OperatorSpec::PdfEdit(PdfEditOptions::Outline(OutlineEditOptions {
                    action: OutlineEditAction::Delete,
                    tree: None,
                })),
            ),
            stdin,
            args.force,
            stdout,
        ),
    }
}

fn run_attach(
    command: AttachCommand,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    match command {
        AttachCommand::Add(args) => {
            reject_shared_stdin_inputs(&args.input, &args.file)?;
            let name = match args.name {
                Some(name) => Some(name),
                None => Some(
                    args.file
                        .file_name()
                        .and_then(|name| name.to_str())
                        .ok_or_else(|| {
                            CliError::Workflow(
                                "attachment name must be explicit for this path".to_owned(),
                            )
                        })?
                        .to_owned(),
                ),
            };
            let workflow = two_input_workflow(
                args.input,
                args.file,
                args.output,
                "attach_add",
                OperatorSpec::PdfEdit(PdfEditOptions::Attachment(AttachmentEditOptions {
                    action: AttachmentEditAction::Add,
                    name,
                    description: args.description,
                })),
            );
            execute_and_write_workflow(workflow, stdin, args.force, stdout)
        }
        AttachCommand::List(args) => execute_and_write_workflow(
            one_input_workflow(
                args.input,
                args.output,
                "attach_list",
                OperatorSpec::PdfInspect(PdfInspectOptions::Attachments(
                    AttachmentInspectOptions::default(),
                )),
            ),
            stdin,
            args.force,
            stdout,
        ),
        AttachCommand::Extract(args) => execute_and_write_workflow(
            one_input_workflow(
                args.input,
                args.output,
                "attach_extract",
                OperatorSpec::PdfInspect(PdfInspectOptions::AttachmentExtract(
                    AttachmentExtractOptions { name: args.name },
                )),
            ),
            stdin,
            args.force,
            stdout,
        ),
        AttachCommand::Delete(args) => execute_and_write_workflow(
            one_input_workflow(
                args.input,
                args.output,
                "attach_delete",
                OperatorSpec::PdfEdit(PdfEditOptions::Attachment(AttachmentEditOptions {
                    action: AttachmentEditAction::Delete,
                    name: Some(args.name),
                    description: None,
                })),
            ),
            stdin,
            args.force,
            stdout,
        ),
    }
}

fn run_annot(command: AnnotCommand, stdin: &[u8], stdout: &mut impl Write) -> Result<(), CliError> {
    match command {
        AnnotCommand::List(args) => execute_and_write_workflow(
            one_input_workflow(
                args.input,
                args.output,
                "annot_list",
                OperatorSpec::PdfInspect(PdfInspectOptions::Annotations(
                    AnnotationInspectOptions::default(),
                )),
            ),
            stdin,
            args.force,
            stdout,
        ),
        AnnotCommand::Add(args) => execute_and_write_workflow(
            one_input_workflow(
                args.input,
                args.output,
                "annot_add",
                OperatorSpec::PdfEdit(PdfEditOptions::Annotation(AnnotationEditOptions {
                    action: AnnotationEditAction::AddText,
                    page: Some(args.page),
                    id: Some(args.id),
                    text: Some(args.text),
                })),
            ),
            stdin,
            args.force,
            stdout,
        ),
        AnnotCommand::Delete(args) => execute_and_write_workflow(
            one_input_workflow(
                args.input,
                args.output,
                "annot_delete",
                OperatorSpec::PdfEdit(PdfEditOptions::Annotation(AnnotationEditOptions {
                    action: AnnotationEditAction::Delete,
                    page: None,
                    id: Some(args.id),
                    text: None,
                })),
            ),
            stdin,
            args.force,
            stdout,
        ),
    }
}

fn run_form(command: FormCommand, stdin: &[u8], stdout: &mut impl Write) -> Result<(), CliError> {
    match command {
        FormCommand::Inspect(args) => execute_and_write_workflow(
            one_input_workflow(
                args.input,
                args.output,
                "form_inspect",
                OperatorSpec::PdfInspect(PdfInspectOptions::Forms(FormInspectOptions::default())),
            ),
            stdin,
            args.force,
            stdout,
        ),
        FormCommand::Fill(args) => execute_and_write_workflow(
            one_input_workflow(
                args.input,
                args.output,
                "form_fill",
                OperatorSpec::PdfEdit(PdfEditOptions::FormFill(FormFillOptions {
                    fields: parse_form_fields(args.fields)?,
                })),
            ),
            stdin,
            args.force,
            stdout,
        ),
        FormCommand::UnlockReadonly(args) => execute_and_write_workflow(
            one_input_workflow(
                args.input,
                args.output,
                "form_unlock_readonly",
                OperatorSpec::PdfEdit(PdfEditOptions::FormUnlockReadonly),
            ),
            stdin,
            args.force,
            stdout,
        ),
        FormCommand::Remove(args) => execute_and_write_workflow(
            one_input_workflow(
                args.input,
                args.output,
                "form_remove",
                OperatorSpec::PdfEdit(PdfEditOptions::FormRemove),
            ),
            stdin,
            args.force,
            stdout,
        ),
    }
}

fn run_interactive(
    command: InteractiveCommand,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    match command {
        InteractiveCommand::Remove(args) => execute_and_write_workflow(
            one_input_workflow(
                args.input,
                args.output,
                "interactive_remove",
                OperatorSpec::PdfEdit(PdfEditOptions::InteractiveRemove(
                    InteractiveRemovalOptions {
                        annotations: args.annotations,
                        forms: args.forms,
                        actions: args.actions,
                        javascript: args.javascript,
                        embedded_files: args.embedded_files,
                    },
                )),
            ),
            stdin,
            args.force,
            stdout,
        ),
    }
}

fn run_stamp(args: StampArgs, stdin: &[u8], stdout: &mut impl Write) -> Result<(), CliError> {
    execute_and_write_workflow(
        one_input_workflow(
            args.input,
            args.output,
            "stamp",
            OperatorSpec::PdfEdit(PdfEditOptions::Overlay(OverlayOptions {
                kind: OverlayKind::Stamp,
                text: Some(args.text),
                font: Some(args.font.unwrap_or_else(|| "Helvetica".to_owned())),
                font_path: args.font_path,
                font_size: args.font_size,
                opacity: args.opacity,
                rotation: args.rotation,
                position: args.position,
                pages: args.pages,
                scale: None,
                rasterize: false,
                source_page: None,
            })),
        ),
        stdin,
        args.force,
        stdout,
    )
}

fn run_signature_appearance(
    args: SignatureAppearanceArgs,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    execute_and_write_workflow(
        one_input_workflow(
            args.input,
            args.output,
            "signature_appearance",
            OperatorSpec::PdfEdit(PdfEditOptions::Overlay(OverlayOptions {
                kind: OverlayKind::SignatureAppearance,
                text: Some(args.text),
                font: Some(args.font.unwrap_or_else(|| "Helvetica".to_owned())),
                font_path: args.font_path,
                font_size: args.font_size,
                opacity: Some(1.0),
                rotation: None,
                position: args.position,
                pages: args.pages,
                scale: None,
                rasterize: false,
                source_page: None,
            })),
        ),
        stdin,
        args.force,
        stdout,
    )
}

fn run_overlay_pdf(
    args: OverlayPdfArgs,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    reject_shared_stdin_inputs(&args.input, &args.overlay)?;
    execute_and_write_workflow(
        two_input_workflow(
            args.input,
            args.overlay,
            args.output,
            "overlay_pdf",
            OperatorSpec::PdfEdit(PdfEditOptions::Overlay(OverlayOptions {
                kind: OverlayKind::PdfPage,
                text: None,
                font: None,
                font_path: None,
                font_size: None,
                opacity: args.opacity,
                rotation: None,
                position: args.position,
                pages: args.pages,
                scale: args.scale,
                rasterize: false,
                source_page: args.source_page,
            })),
        ),
        stdin,
        args.force,
        stdout,
    )
}

fn run_image(command: ImageCommand, stdin: &[u8], stdout: &mut impl Write) -> Result<(), CliError> {
    match command {
        ImageCommand::List(args) => execute_and_write_workflow(
            one_input_workflow(
                args.input,
                args.output,
                "image_list",
                OperatorSpec::PdfInspect(PdfInspectOptions::Images(ImageInspectOptions::default())),
            ),
            stdin,
            args.force,
            stdout,
        ),
        ImageCommand::Add(args) => {
            reject_shared_stdin_inputs(&args.input, &args.image)?;
            execute_and_write_workflow(
                two_input_workflow(
                    args.input,
                    args.image,
                    args.output,
                    "image_add",
                    OperatorSpec::PdfEdit(PdfEditOptions::ImageEdit(ImageEditOptions {
                        action: ImageEditAction::Add,
                        name: Some(args.name),
                        page: Some(args.page),
                    })),
                ),
                stdin,
                args.force,
                stdout,
            )
        }
        ImageCommand::Replace(args) => {
            reject_shared_stdin_inputs(&args.input, &args.image)?;
            execute_and_write_workflow(
                two_input_workflow(
                    args.input,
                    args.image,
                    args.output,
                    "image_replace",
                    OperatorSpec::PdfEdit(PdfEditOptions::ImageEdit(ImageEditOptions {
                        action: ImageEditAction::Replace,
                        name: Some(args.name),
                        page: None,
                    })),
                ),
                stdin,
                args.force,
                stdout,
            )
        }
        ImageCommand::Delete(args) => execute_and_write_workflow(
            one_input_workflow(
                args.input,
                args.output,
                "image_delete",
                OperatorSpec::PdfEdit(PdfEditOptions::ImageEdit(ImageEditOptions {
                    action: ImageEditAction::Delete,
                    name: Some(args.name),
                    page: None,
                })),
            ),
            stdin,
            args.force,
            stdout,
        ),
        ImageCommand::Extract(args) => execute_and_write_workflow(
            one_input_workflow(
                args.input,
                args.output,
                "image_extract",
                OperatorSpec::PdfInspect(PdfInspectOptions::ImageExtract(ImageExtractOptions {
                    name: args.name,
                })),
            ),
            stdin,
            args.force,
            stdout,
        ),
    }
}

fn run_color(command: ColorCommand, stdin: &[u8], stdout: &mut impl Write) -> Result<(), CliError> {
    match command {
        ColorCommand::Contrast(args) => run_color_edit(
            args.input,
            args.output,
            args.force,
            ColorEditOptions {
                action: ColorEditAction::Contrast,
                pages: args.pages,
                from: None,
                to: None,
                factor: Some(args.factor),
                rasterize_pages: false,
            },
            stdin,
            stdout,
        ),
        ColorCommand::Invert(args) => run_color_edit(
            args.input,
            args.output,
            args.force,
            ColorEditOptions {
                action: ColorEditAction::Invert,
                pages: args.pages,
                from: None,
                to: None,
                factor: None,
                rasterize_pages: false,
            },
            stdin,
            stdout,
        ),
        ColorCommand::Replace(args) => run_color_edit(
            args.input,
            args.output,
            args.force,
            ColorEditOptions {
                action: ColorEditAction::Replace,
                pages: args.pages,
                from: Some(parse_rgb(&args.from)?),
                to: Some(parse_rgb(&args.to)?),
                factor: None,
                rasterize_pages: false,
            },
            stdin,
            stdout,
        ),
    }
}

fn run_color_edit(
    input: PathBuf,
    output: PathBuf,
    force: bool,
    options: ColorEditOptions,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    execute_and_write_workflow(
        one_input_workflow(
            input,
            output,
            "color",
            OperatorSpec::PdfEdit(PdfEditOptions::Color(options)),
        ),
        stdin,
        force,
        stdout,
    )
}

fn parse_watermark_kind(value: &str) -> Result<WatermarkKind, CliError> {
    match value {
        "text" => Ok(WatermarkKind::Text),
        "image" => Ok(WatermarkKind::Image),
        "svg" => Ok(WatermarkKind::Svg),
        other => Err(CliError::Workflow(format!(
            "unsupported watermark kind '{other}'"
        ))),
    }
}

fn parse_rgb(value: &str) -> Result<[f32; 3], CliError> {
    let parts = value.split(',').collect::<Vec<_>>();
    if parts.len() != 3 {
        return Err(CliError::Workflow(
            "RGB color must use r,g,b components".to_owned(),
        ));
    }
    let mut rgb = [0.0; 3];
    for (index, part) in parts.iter().enumerate() {
        let component = part
            .parse::<f32>()
            .map_err(|_| CliError::Workflow("RGB color components must be numbers".to_owned()))?;
        if !(0.0..=1.0).contains(&component) {
            return Err(CliError::Workflow(
                "RGB color components must be between 0.0 and 1.0".to_owned(),
            ));
        }
        rgb[index] = component;
    }
    Ok(rgb)
}

fn execute_and_write_workflow(
    workflow: Workflow,
    stdin: &[u8],
    force: bool,
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    let store = load_inputs(&workflow, stdin)?;
    let mut runner = PdfOperatorRunner::with_limits(workflow.limits.clone());
    let result = execute_workflow(&workflow, store, &mut runner).map_err(CliError::Core)?;
    write_outputs(&workflow, &result.store, force, stdout)
}

fn one_input_workflow(
    input: PathBuf,
    output: PathBuf,
    task_id: &'static str,
    op: OperatorSpec,
) -> Workflow {
    Workflow {
        version: WorkflowVersion::V1,
        inputs: vec![oxidepdf_core::InputSpec {
            id: ArtifactRef::new("input"),
            path: input,
        }],
        tasks: vec![TaskSpec {
            id: TaskId::new(task_id),
            op,
            inputs: vec![ArtifactRef::new("input")],
        }],
        outputs: vec![oxidepdf_core::OutputSpec {
            id: ArtifactRef::new("output"),
            from: ArtifactRef::new(task_id),
            path: output,
        }],
        limits: Default::default(),
        metadata: WorkflowMetadata::default(),
    }
}

fn two_input_workflow(
    first: PathBuf,
    second: PathBuf,
    output: PathBuf,
    task_id: &'static str,
    op: OperatorSpec,
) -> Workflow {
    Workflow {
        version: WorkflowVersion::V1,
        inputs: vec![
            oxidepdf_core::InputSpec {
                id: ArtifactRef::new("input"),
                path: first,
            },
            oxidepdf_core::InputSpec {
                id: ArtifactRef::new("attachment"),
                path: second,
            },
        ],
        tasks: vec![TaskSpec {
            id: TaskId::new(task_id),
            op,
            inputs: vec![ArtifactRef::new("input"), ArtifactRef::new("attachment")],
        }],
        outputs: vec![oxidepdf_core::OutputSpec {
            id: ArtifactRef::new("output"),
            from: ArtifactRef::new(task_id),
            path: output,
        }],
        limits: Default::default(),
        metadata: WorkflowMetadata::default(),
    }
}

fn parse_metadata_entries(entries: Vec<String>) -> Result<Vec<MetadataEntry>, CliError> {
    entries
        .into_iter()
        .map(|entry| {
            let (key, value) = parse_key_value(&entry, "metadata entry")?;
            Ok(MetadataEntry { key, value })
        })
        .collect()
}

fn parse_form_fields(fields: Vec<String>) -> Result<Vec<FormFieldValue>, CliError> {
    fields
        .into_iter()
        .map(|field| {
            let (name, value) = parse_key_value(&field, "form field")?;
            Ok(FormFieldValue { name, value })
        })
        .collect()
}

fn parse_key_value(value: &str, label: &str) -> Result<(String, String), CliError> {
    let Some((key, value)) = value.split_once('=') else {
        return Err(CliError::Workflow(format!(
            "{label} must use key=value syntax"
        )));
    };
    if key.is_empty() {
        return Err(CliError::Workflow(format!("{label} key must not be empty")));
    }
    Ok((key.to_owned(), value.to_owned()))
}

fn reject_shared_stdin_inputs(first: &Path, second: &Path) -> Result<(), CliError> {
    if is_stdio(first) && is_stdio(second) {
        return Err(CliError::Workflow(
            "commands with two independent inputs cannot read both inputs from stdin".to_owned(),
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, Copy)]
enum PageCommand {
    KeepPages,
    ExtractPages,
    ReorderPages,
}

fn parse_workflow(bytes: &[u8], path: &Path) -> Result<Workflow, CliError> {
    if path.extension().and_then(|extension| extension.to_str()) == Some("json") {
        serde_json::from_slice(bytes).map_err(|error| CliError::Workflow(error.to_string()))
    } else {
        serde_yaml::from_slice(bytes).map_err(|error| CliError::Workflow(error.to_string()))
    }
}

fn load_inputs(workflow: &Workflow, stdin: &[u8]) -> Result<ArtifactStore, CliError> {
    let mut store = ArtifactStore::new();
    let mut total_input_bytes = 0u64;
    for input in &workflow.inputs {
        let bytes = read_path_or_stdin(&input.path, stdin).map_err(CliError::Input)?;
        enforce_cli_input_limits(bytes.len(), &mut total_input_bytes, &workflow.limits)?;
        store.insert(input.id.clone(), Artifact::bytes(bytes));
    }

    Ok(store)
}

fn write_outputs(
    workflow: &Workflow,
    store: &ArtifactStore,
    force: bool,
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    for output in &workflow.outputs {
        let artifact = store.get(&output.from).ok_or_else(|| {
            CliError::Core(OxideError::InvalidWorkflow {
                reason: format!(
                    "output '{}' references missing artifact '{}'",
                    output.id.as_str(),
                    output.from.as_str()
                ),
            })
        })?;
        let bytes = artifact_bytes(artifact)?;
        enforce_cli_output_limit(bytes.len(), &workflow.limits)?;
        if is_stdio(&output.path) {
            stdout.write_all(bytes).map_err(CliError::Io)?;
        } else {
            if output.path.exists() && !force {
                return Err(CliError::Workflow(format!(
                    "output file already exists: {}",
                    output.path.display()
                )));
            }
            fs::write(&output.path, bytes).map_err(CliError::Io)?;
        }
    }

    Ok(())
}

fn artifact_bytes(artifact: &Artifact) -> Result<&[u8], CliError> {
    match artifact {
        Artifact::Bytes(bytes) => Ok(&bytes.bytes),
        Artifact::Pdf(pdf) => Ok(&pdf.bytes),
        Artifact::Image(image) => Ok(&image.bytes),
        Artifact::Svg(svg) => Ok(&svg.bytes),
        Artifact::Text(text) => Ok(text.text.as_bytes()),
    }
}

fn read_path_or_stdin(path: &Path, stdin: &[u8]) -> io::Result<Vec<u8>> {
    if is_stdio(path) {
        Ok(stdin.to_vec())
    } else {
        fs::read(path)
    }
}

fn enforce_cli_input_limits(
    size: usize,
    total_input_bytes: &mut u64,
    limits: &oxidepdf_core::ResourceLimits,
) -> Result<(), CliError> {
    if limits
        .max_input_bytes
        .is_some_and(|limit| size as u64 > limit)
    {
        return Err(CliError::Core(OxideError::ResourceLimitExceeded {
            limit: "max_input_bytes".to_owned(),
        }));
    }
    *total_input_bytes = total_input_bytes.checked_add(size as u64).ok_or_else(|| {
        CliError::Core(OxideError::ResourceLimitExceeded {
            limit: "max_total_input_bytes".to_owned(),
        })
    })?;
    if limits
        .max_total_input_bytes
        .is_some_and(|limit| *total_input_bytes > limit)
    {
        return Err(CliError::Core(OxideError::ResourceLimitExceeded {
            limit: "max_total_input_bytes".to_owned(),
        }));
    }

    Ok(())
}

fn enforce_cli_output_limit(
    size: usize,
    limits: &oxidepdf_core::ResourceLimits,
) -> Result<(), CliError> {
    if limits
        .max_output_bytes
        .is_some_and(|limit| size as u64 > limit)
    {
        return Err(CliError::Core(OxideError::ResourceLimitExceeded {
            limit: "max_output_bytes".to_owned(),
        }));
    }

    Ok(())
}

fn is_stdio(path: &Path) -> bool {
    path == Path::new("-")
}

#[derive(Debug)]
enum CliError {
    Arguments(clap::Error),
    Workflow(String),
    Input(io::Error),
    Io(io::Error),
    Core(OxideError),
}

impl CliError {
    fn exit_code(&self) -> i32 {
        match self {
            Self::Arguments(error) => error.exit_code(),
            Self::Workflow(_) => 2,
            Self::Input(_) => 3,
            Self::Core(OxideError::InvalidWorkflow { .. }) => 2,
            Self::Core(OxideError::InvalidInput { .. })
            | Self::Core(OxideError::UnsupportedPdfFeature { .. })
            | Self::Core(OxideError::ParsePdf)
            | Self::Core(OxideError::RenderPdf)
            | Self::Core(OxideError::ExtractText)
            | Self::Core(OxideError::SvgParse)
            | Self::Core(OxideError::ImageDecode) => 3,
            Self::Core(OxideError::EncryptedPdf) | Self::Core(OxideError::IncorrectPassword) => 4,
            Self::Core(OxideError::ResourceLimitExceeded { .. }) => 5,
            Self::Io(_)
            | Self::Core(OxideError::WritePdf)
            | Self::Core(OxideError::FontResolution)
            | Self::Core(OxideError::Io)
            | Self::Core(OxideError::Internal) => 70,
        }
    }
}

impl std::fmt::Display for CliError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Arguments(error) => write!(formatter, "{error}"),
            Self::Workflow(error) => write!(formatter, "invalid workflow: {error}"),
            Self::Input(error) => write!(formatter, "input error: {}", sanitized_io_error(error)),
            Self::Io(error) => write!(formatter, "I/O error: {}", sanitized_io_error(error)),
            Self::Core(error) => write!(formatter, "{}: {error}", error.code()),
        }
    }
}

fn sanitized_io_error(error: &io::Error) -> &'static str {
    match error.kind() {
        io::ErrorKind::NotFound => "file not found",
        io::ErrorKind::PermissionDenied => "permission denied",
        io::ErrorKind::AlreadyExists => "file already exists",
        io::ErrorKind::InvalidInput => "invalid input",
        io::ErrorKind::InvalidData => "invalid data",
        io::ErrorKind::UnexpectedEof => "unexpected end of file",
        io::ErrorKind::WriteZero => "write failed",
        io::ErrorKind::Interrupted => "operation interrupted",
        io::ErrorKind::TimedOut => "operation timed out",
        _ => "operation failed",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::dictionary;
    use std::fs;

    #[test]
    fn clap_definition_is_valid() {
        command().debug_assert();
    }

    #[test]
    fn help_mentions_project_name() {
        let mut help = Vec::new();
        command().write_long_help(&mut help).unwrap();
        let help = String::from_utf8(help).unwrap();

        assert!(help.contains("OxidePDF"));
        assert!(help.contains("pure Rust PDF toolkit"));
        assert!(help.contains("pdf-sign"));
    }

    #[test]
    fn version_uses_package_version() {
        let command = command();
        let version = command.get_version().unwrap();

        assert_eq!(version, env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn help_returns_success_exit_code() {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let code = run_with_io(["oxidepdf", "--help"], [], &mut stdout, &mut stderr);

        assert_eq!(code, 0);
        assert_eq!(stdout, b"");
        assert!(String::from_utf8(stderr)
            .unwrap()
            .contains("Usage: oxidepdf"));
    }

    #[test]
    fn invalid_arguments_return_usage_exit_code() {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let code = run_with_io(["oxidepdf", "--missing"], [], &mut stdout, &mut stderr);

        assert_eq!(code, 2);
        assert_eq!(stdout, b"");
        assert!(String::from_utf8(stderr)
            .unwrap()
            .contains("unexpected argument"));
    }

    #[test]
    fn removed_legacy_top_level_commands_are_not_aliases() {
        for legacy_command in [
            "merge",
            "split",
            "reorder",
            "rotate",
            "img2pdf",
            "svg2pdf",
            "render",
            "extract-text",
            "watermark",
            "verify-signatures",
        ] {
            let mut stdout = Vec::new();
            let mut stderr = Vec::new();

            let code = run_with_io(
                ["oxidepdf", legacy_command, "--help"],
                [],
                &mut stdout,
                &mut stderr,
            );

            assert_eq!(code, 2, "{legacy_command} should not remain as an alias");
            assert_eq!(stdout, b"");
            assert!(
                String::from_utf8(stderr)
                    .unwrap()
                    .contains("unrecognized subcommand"),
                "{legacy_command} should fail at the CLI boundary"
            );
        }
    }

    #[test]
    fn render_file_input_does_not_require_stdin() {
        let stdin = stdin_for_args([
            "oxidepdf",
            "pdf-inspect",
            "render",
            "input.pdf",
            "--page",
            "1",
            "-o",
            "output.png",
        ])
        .unwrap();

        assert!(stdin.is_empty());
    }

    #[test]
    fn render_stdio_input_requires_stdin() {
        let cli = Cli::try_parse_from([
            "oxidepdf",
            "pdf-inspect",
            "render",
            "-",
            "--page",
            "1",
            "-o",
            "output.png",
        ])
        .unwrap();

        assert!(cli_reads_stdin(&cli));
    }

    #[test]
    fn extract_text_stdio_input_requires_stdin() {
        let cli = Cli::try_parse_from([
            "oxidepdf",
            "pdf-inspect",
            "extract-text",
            "-",
            "-o",
            "output.txt",
        ])
        .unwrap();

        assert!(cli_reads_stdin(&cli));
    }

    #[test]
    fn run_workflow_file_writes_input_artifact_to_output() {
        let dir = temp_dir("run_workflow_file_writes_input_artifact_to_output");
        let workflow = dir.join("workflow.yaml");
        let output = dir.join("out.bin");
        fs::write(
            &workflow,
            format!(
                r#"
                version: 1
                inputs:
                  - id: source
                    path: {}
                tasks: []
                outputs:
                  - id: final
                    from: source
                    path: {}
                "#,
                yaml_path(dir.join("input.bin")),
                yaml_path(&output)
            ),
        )
        .unwrap();
        fs::write(dir.join("input.bin"), b"input bytes").unwrap();
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let code = run_with_io(
            ["oxidepdf", "run", "--workflow", workflow.to_str().unwrap()],
            [],
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 0);
        assert_eq!(stdout, b"");
        assert_eq!(stderr, b"");
        assert_eq!(fs::read(output).unwrap(), b"input bytes");
    }

    #[test]
    fn run_workflow_input_stdin_and_output_stdout_keep_diagnostics_on_stderr() {
        let dir = temp_dir("run_workflow_input_stdin_and_output_stdout");
        let workflow_path = dir.join("workflow.yaml");
        let workflow = br#"
            version: 1
            inputs:
              - id: source
                path: "-"
            tasks: []
            outputs:
              - id: final
                from: source
                path: "-"
        "#;
        fs::write(&workflow_path, workflow).unwrap();
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let code = run_with_io(
            [
                "oxidepdf",
                "run",
                "--workflow",
                workflow_path.to_str().unwrap(),
            ],
            b"stdin bytes",
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 0);
        assert_eq!(stdout, b"stdin bytes");
        assert_eq!(stderr, b"");
    }

    #[test]
    fn run_workflow_document_can_be_read_from_stdin() {
        let workflow = br#"
            version: 1
            inputs: []
            tasks: []
            outputs: []
        "#;
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let code = run_with_io(
            ["oxidepdf", "run", "--workflow", "-"],
            workflow,
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 0);
        assert_eq!(stdout, b"");
        assert_eq!(stderr, b"");
    }

    #[test]
    fn pdf_parse_error_returns_input_exit_code_without_output() {
        let dir = temp_dir("pdf_parse_error_returns_input_exit_code");
        let workflow = dir.join("workflow.yaml");
        let output = dir.join("out.bin");
        fs::write(dir.join("input.bin"), b"input bytes").unwrap();
        fs::write(
            &workflow,
            format!(
                r#"
                version: 1
                inputs:
                  - id: source
                    path: {}
                tasks:
                  - id: rotate
                    op:
                      pdf_edit:
                        rotate_pages:
                          pages: "1"
                          degrees: 90
                    inputs: [source]
                outputs:
                  - id: final
                    from: rotate
                    path: {}
                "#,
                yaml_path(dir.join("input.bin")),
                yaml_path(&output)
            ),
        )
        .unwrap();
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let code = run_with_io(
            ["oxidepdf", "run", "--workflow", workflow.to_str().unwrap()],
            [],
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 3);
        assert_eq!(stdout, b"");
        assert!(!output.exists());
        let stderr = String::from_utf8(stderr).unwrap();
        assert!(stderr.contains("invalid_input"));
        assert!(stderr.contains("expected PDF"));
    }

    #[test]
    fn workflow_enforces_total_input_size_limit() {
        let dir = temp_dir("workflow_enforces_total_input_size_limit");
        let workflow = dir.join("workflow.yaml");
        let output = dir.join("out.bin");
        fs::write(dir.join("input_a.bin"), b"12345").unwrap();
        fs::write(dir.join("input_b.bin"), b"67890").unwrap();
        fs::write(
            &workflow,
            format!(
                r#"
                version: 1
                inputs:
                  - id: first
                    path: {}
                  - id: second
                    path: {}
                tasks: []
                outputs:
                  - id: final
                    from: first
                    path: {}
                limits:
                  max_total_input_bytes: 9
                "#,
                yaml_path(dir.join("input_a.bin")),
                yaml_path(dir.join("input_b.bin")),
                yaml_path(&output)
            ),
        )
        .unwrap();
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let code = run_with_io(
            ["oxidepdf", "run", "--workflow", workflow.to_str().unwrap()],
            [],
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 5);
        assert_eq!(stdout, b"");
        assert!(!output.exists());
        assert!(String::from_utf8(stderr)
            .unwrap()
            .contains("max_total_input_bytes"));
    }

    #[test]
    fn workflow_enforces_output_size_limit() {
        let dir = temp_dir("workflow_enforces_output_size_limit");
        let workflow = dir.join("workflow.yaml");
        let output = dir.join("out.bin");
        fs::write(dir.join("input.bin"), b"larger than limit").unwrap();
        fs::write(
            &workflow,
            format!(
                r#"
                version: 1
                inputs:
                  - id: source
                    path: {}
                tasks: []
                outputs:
                  - id: final
                    from: source
                    path: {}
                limits:
                  max_output_bytes: 1
                "#,
                yaml_path(dir.join("input.bin")),
                yaml_path(&output)
            ),
        )
        .unwrap();
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let code = run_with_io(
            ["oxidepdf", "run", "--workflow", workflow.to_str().unwrap()],
            [],
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 5);
        assert_eq!(stdout, b"");
        assert!(!output.exists());
        assert!(String::from_utf8(stderr)
            .unwrap()
            .contains("max_output_bytes"));
    }

    #[test]
    fn error_output_redacts_sensitive_material_and_paths() {
        let dir = temp_dir("error_output_redacts_sensitive_material_and_paths");
        let secret_dir = dir.join("secret-client-certificates");
        fs::create_dir_all(&secret_dir).unwrap();
        let missing = secret_dir.join("client-password-token.pem");
        let workflow = dir.join("workflow.yaml");
        fs::write(
            &workflow,
            format!(
                r#"
                version: 1
                inputs:
                  - id: source
                    path: {}
                tasks: []
                outputs:
                  - id: final
                    from: source
                    path: {}
                "#,
                yaml_path(&missing),
                yaml_path(dir.join("out.bin"))
            ),
        )
        .unwrap();
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let code = run_with_io(
            ["oxidepdf", "run", "--workflow", workflow.to_str().unwrap()],
            [],
            &mut stdout,
            &mut stderr,
        );
        let stderr = String::from_utf8(stderr).unwrap();

        assert_eq!(code, 3);
        assert_eq!(stdout, b"");
        assert!(!stderr.contains(dir.to_str().unwrap()));
        assert!(!stderr.to_ascii_lowercase().contains("password"));
        assert!(!stderr.to_ascii_lowercase().contains("token"));
        assert!(!stderr.to_ascii_lowercase().contains("certificate"));
        assert!(!stderr.contains(".pem"));
        assert!(!stderr.contains("stack backtrace"));
    }

    #[test]
    fn merge_command_writes_combined_pdf() {
        let dir = temp_dir("merge_command_writes_combined_pdf");
        let input = fixture_pdf();
        let output = dir.join("merged.pdf");
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let code = run_with_io(
            [
                "oxidepdf",
                "pdf-edit",
                "merge",
                input.to_str().unwrap(),
                input.to_str().unwrap(),
                "-o",
                output.to_str().unwrap(),
            ],
            [],
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 0);
        assert_eq!(stdout, b"");
        assert_eq!(stderr, b"");
        assert_eq!(pdf_page_count(&output), 6);
    }

    #[test]
    fn split_command_writes_selected_pages() {
        let dir = temp_dir("split_command_writes_selected_pages");
        let output = dir.join("split.pdf");
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let code = run_with_io(
            [
                "oxidepdf",
                "pdf-edit",
                "keep-pages",
                fixture_pdf().to_str().unwrap(),
                "--pages",
                "1,3",
                "-o",
                output.to_str().unwrap(),
            ],
            [],
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 0);
        assert_eq!(stdout, b"");
        assert_eq!(stderr, b"");
        assert_eq!(pdf_page_count(&output), 2);
    }

    #[test]
    fn rotate_command_updates_rotation() {
        let dir = temp_dir("rotate_command_updates_rotation");
        let output = dir.join("rotated.pdf");
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let code = run_with_io(
            [
                "oxidepdf",
                "pdf-edit",
                "rotate-pages",
                fixture_pdf().to_str().unwrap(),
                "--pages",
                "1",
                "--degrees",
                "90",
                "-o",
                output.to_str().unwrap(),
            ],
            [],
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 0);
        assert_eq!(stdout, b"");
        assert_eq!(stderr, b"");
        assert_eq!(pdf_page_rotation(&output, 1), 90);
    }

    #[test]
    fn delete_pages_command_removes_selected_pages() {
        let dir = temp_dir("delete_pages_command_removes_selected_pages");
        let output = dir.join("deleted.pdf");
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let code = run_with_io(
            [
                "oxidepdf",
                "pdf-edit",
                "delete-pages",
                fixture_pdf().to_str().unwrap(),
                "--pages",
                "2",
                "-o",
                output.to_str().unwrap(),
            ],
            [],
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 0);
        assert_eq!(stdout, b"");
        assert_eq!(stderr, b"");
        assert_eq!(pdf_page_count(&output), 2);
    }

    #[test]
    fn extract_pages_command_writes_selected_pages() {
        let dir = temp_dir("extract_pages_command_writes_selected_pages");
        let output = dir.join("extracted.pdf");
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let code = run_with_io(
            [
                "oxidepdf",
                "pdf-edit",
                "extract-pages",
                fixture_pdf().to_str().unwrap(),
                "--pages",
                "3,1",
                "-o",
                output.to_str().unwrap(),
            ],
            [],
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 0);
        assert_eq!(stdout, b"");
        assert_eq!(stderr, b"");
        assert_eq!(pdf_page_count(&output), 2);
    }

    #[test]
    fn crop_pages_command_sets_crop_box() {
        let dir = temp_dir("crop_pages_command_sets_crop_box");
        let output = dir.join("cropped.pdf");
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let code = run_with_io(
            [
                "oxidepdf",
                "pdf-edit",
                "crop-pages",
                fixture_pdf().to_str().unwrap(),
                "--pages",
                "1",
                "--left",
                "10",
                "--bottom",
                "20",
                "--right",
                "300",
                "--top",
                "400",
                "-o",
                output.to_str().unwrap(),
            ],
            [],
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 0);
        assert_eq!(stdout, b"");
        assert_eq!(stderr, b"");
        assert_eq!(
            pdf_page_box(&output, 1, b"CropBox"),
            [10.0, 20.0, 300.0, 400.0]
        );
    }

    #[test]
    fn scale_pages_command_scales_selected_page() {
        let dir = temp_dir("scale_pages_command_scales_selected_page");
        let output = dir.join("scaled.pdf");
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let code = run_with_io(
            [
                "oxidepdf",
                "pdf-edit",
                "scale-pages",
                fixture_pdf().to_str().unwrap(),
                "--pages",
                "1",
                "--factor",
                "0.5",
                "-o",
                output.to_str().unwrap(),
            ],
            [],
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 0);
        assert_eq!(stdout, b"");
        assert_eq!(stderr, b"");
        assert_eq!(pdf_page_box(&output, 1, b"MediaBox")[2], 306.0);
        assert_eq!(pdf_page_box(&output, 2, b"MediaBox")[2], 612.0);
    }

    #[test]
    fn delete_blank_pages_command_removes_structurally_blank_pages() {
        let dir = temp_dir("delete_blank_pages_command_removes_structurally_blank_pages");
        let input = dir.join("blank-and-marked.pdf");
        let output = dir.join("without-blank.pdf");
        fs::write(&input, pdf_with_blank_and_marked_page()).unwrap();
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let code = run_with_io(
            [
                "oxidepdf",
                "pdf-edit",
                "delete-blank-pages",
                input.to_str().unwrap(),
                "-o",
                output.to_str().unwrap(),
            ],
            [],
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 0);
        assert_eq!(stdout, b"");
        assert_eq!(stderr, b"");
        assert_eq!(pdf_page_count(&output), 1);
    }

    #[test]
    fn single_page_command_combines_pages() {
        let dir = temp_dir("single_page_command_combines_pages");
        let output = dir.join("single.pdf");
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let code = run_with_io(
            [
                "oxidepdf",
                "pdf-edit",
                "single-page",
                fixture_pdf().to_str().unwrap(),
                "-o",
                output.to_str().unwrap(),
            ],
            [],
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 0);
        assert_eq!(stdout, b"");
        assert_eq!(stderr, b"");
        assert_eq!(pdf_page_count(&output), 1);
        assert_eq!(pdf_page_box(&output, 1, b"MediaBox")[3], 2376.0);
    }

    #[test]
    fn nup_command_places_pages_on_fewer_output_pages() {
        let dir = temp_dir("nup_command_places_pages_on_fewer_output_pages");
        let output = dir.join("nup.pdf");
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let code = run_with_io(
            [
                "oxidepdf",
                "pdf-edit",
                "nup",
                fixture_pdf().to_str().unwrap(),
                "--columns",
                "2",
                "--rows",
                "2",
                "-o",
                output.to_str().unwrap(),
            ],
            [],
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 0);
        assert_eq!(stdout, b"");
        assert_eq!(stderr, b"");
        assert_eq!(pdf_page_count(&output), 1);
        assert_eq!(pdf_page_xobject_count(&output, 1), 3);
    }

    #[test]
    fn booklet_command_writes_imposed_pages() {
        let dir = temp_dir("booklet_command_writes_imposed_pages");
        let output = dir.join("booklet.pdf");
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let code = run_with_io(
            [
                "oxidepdf",
                "pdf-edit",
                "booklet",
                fixture_pdf().to_str().unwrap(),
                "-o",
                output.to_str().unwrap(),
            ],
            [],
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 0);
        assert_eq!(stdout, b"");
        assert_eq!(stderr, b"");
        assert_eq!(pdf_page_count(&output), 2);
        assert_eq!(pdf_page_xobject_count(&output, 2), 2);
    }

    #[test]
    fn page_numbers_command_writes_selected_page_labels() {
        let dir = temp_dir("page_numbers_command_writes_selected_page_labels");
        let output = dir.join("numbered.pdf");
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let code = run_with_io(
            [
                "oxidepdf",
                "pdf-edit",
                "page-numbers",
                fixture_pdf().to_str().unwrap(),
                "--pages",
                "2-3",
                "--start",
                "7",
                "--prefix",
                "p",
                "--position",
                "bottom-right",
                "-o",
                output.to_str().unwrap(),
            ],
            [],
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 0);
        assert_eq!(stdout, b"");
        assert_eq!(stderr, b"");
        assert!(!pdf_page_content_contains(&output, 1, "p7"));
        assert!(pdf_page_content_contains(&output, 2, "p7"));
        assert!(pdf_page_content_contains(&output, 3, "p8"));
    }

    #[test]
    fn img2pdf_command_writes_parseable_pdf() {
        let dir = temp_dir("img2pdf_command_writes_parseable_pdf");
        let output = dir.join("image.pdf");
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let code = run_with_io(
            [
                "oxidepdf",
                "pdf-edit",
                "img2pdf",
                fixture_jpg().to_str().unwrap(),
                "-o",
                output.to_str().unwrap(),
            ],
            [],
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 0);
        assert_eq!(stdout, b"");
        assert_eq!(stderr, b"");
        assert_eq!(pdf_page_count(&output), 1);
    }

    #[test]
    fn svg2pdf_command_writes_parseable_pdf() {
        let dir = temp_dir("svg2pdf_command_writes_parseable_pdf");
        let input = dir.join("input.svg");
        let output = dir.join("svg.pdf");
        fs::write(&input, simple_svg()).unwrap();
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let code = run_with_io(
            [
                "oxidepdf",
                "pdf-edit",
                "svg2pdf",
                input.to_str().unwrap(),
                "-o",
                output.to_str().unwrap(),
            ],
            [],
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 0);
        assert_eq!(stdout, b"");
        assert_eq!(stderr, b"");
        assert_eq!(pdf_page_count(&output), 1);
    }

    #[test]
    fn extract_text_command_writes_plain_text() {
        let dir = temp_dir("extract_text_command_writes_plain_text");
        let output = dir.join("extracted.txt");
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let code = run_with_io(
            [
                "oxidepdf",
                "pdf-inspect",
                "extract-text",
                fixture_pdf().to_str().unwrap(),
                "-o",
                output.to_str().unwrap(),
            ],
            [],
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 0);
        assert_eq!(stdout, b"");
        assert_eq!(stderr, b"");
        assert!(!fs::read_to_string(output).unwrap().trim().is_empty());
    }

    #[test]
    fn extract_text_command_rejects_pdf_without_text_layer() {
        let dir = temp_dir("extract_text_command_rejects_pdf_without_text_layer");
        let input = dir.join("image.pdf");
        let output = dir.join("extracted.txt");
        fs::write(&input, image_only_pdf()).unwrap();
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let code = run_with_io(
            [
                "oxidepdf",
                "pdf-inspect",
                "extract-text",
                input.to_str().unwrap(),
                "-o",
                output.to_str().unwrap(),
            ],
            [],
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 3);
        assert_eq!(stdout, b"");
        assert!(!output.exists());
        assert!(String::from_utf8(stderr)
            .unwrap()
            .contains("no extractable text layer"));
    }

    #[test]
    fn watermark_text_command_writes_parseable_pdf() {
        let dir = temp_dir("watermark_text_command_writes_parseable_pdf");
        let output = dir.join("watermarked.pdf");
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let code = run_with_io(
            [
                "oxidepdf",
                "pdf-edit",
                "watermark",
                fixture_pdf().to_str().unwrap(),
                "--kind",
                "text",
                "--text",
                "DRAFT",
                "--font",
                "DejaVu Sans",
                "--pages",
                "1",
                "-o",
                output.to_str().unwrap(),
            ],
            [],
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 0);
        assert_eq!(stdout, b"");
        assert_eq!(stderr, b"");
        assert_eq!(pdf_page_count(&output), 3);
        assert!(page_has_content_operator(&output, 1, "Tj"));
    }

    #[test]
    fn watermark_text_command_returns_font_resolution_for_missing_font() {
        let dir = temp_dir("watermark_text_command_returns_font_resolution_for_missing_font");
        let output = dir.join("watermarked.pdf");
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let code = run_with_io(
            [
                "oxidepdf",
                "pdf-edit",
                "watermark",
                fixture_pdf().to_str().unwrap(),
                "--kind",
                "text",
                "--text",
                "DRAFT",
                "--font",
                "Definitely Missing Font Family",
                "-o",
                output.to_str().unwrap(),
            ],
            [],
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 70);
        assert_eq!(stdout, b"");
        assert!(!output.exists());
        assert!(String::from_utf8(stderr)
            .unwrap()
            .contains("font_resolution"));
    }

    #[test]
    fn watermark_image_command_writes_parseable_pdf() {
        let dir = temp_dir("watermark_image_command_writes_parseable_pdf");
        let output = dir.join("watermarked.pdf");
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let code = run_with_io(
            [
                "oxidepdf",
                "pdf-edit",
                "watermark",
                fixture_pdf().to_str().unwrap(),
                "--kind",
                "image",
                "--watermark",
                fixture_jpg().to_str().unwrap(),
                "--pages",
                "2",
                "-o",
                output.to_str().unwrap(),
            ],
            [],
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 0);
        assert_eq!(stdout, b"");
        assert_eq!(stderr, b"");
        assert!(page_has_content_operator(&output, 2, "Do"));
    }

    #[test]
    fn watermark_svg_command_writes_parseable_pdf() {
        let dir = temp_dir("watermark_svg_command_writes_parseable_pdf");
        let input = dir.join("watermark.svg");
        let output = dir.join("watermarked.pdf");
        fs::write(&input, simple_svg()).unwrap();
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let code = run_with_io(
            [
                "oxidepdf",
                "pdf-edit",
                "watermark",
                fixture_pdf().to_str().unwrap(),
                "--kind",
                "svg",
                "--watermark",
                input.to_str().unwrap(),
                "--pages",
                "3",
                "-o",
                output.to_str().unwrap(),
            ],
            [],
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 0);
        assert_eq!(stdout, b"");
        assert_eq!(stderr, b"");
        assert!(page_has_content_operator(&output, 3, "Do"));
    }

    #[test]
    fn verify_signatures_command_writes_json_report() {
        let dir = temp_dir("verify_signatures_command_writes_json_report");
        let input = write_signature_pdf(&dir);
        let output = dir.join("signature-report.json");
        let trust_anchors = write_test_trust_anchors(&dir);
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let code = run_with_io(
            [
                "oxidepdf",
                "pdf-sign",
                "verify",
                input.to_str().unwrap(),
                "--trust-anchors",
                trust_anchors.to_str().unwrap(),
                "-o",
                output.to_str().unwrap(),
            ],
            [],
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 0);
        assert_eq!(stdout, b"");
        assert_eq!(stderr, b"");
        let report: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(output).unwrap()).unwrap();
        assert_eq!(report["verdict"], "unsupported");
        assert_eq!(report["trust_anchor_count"], 1);
        assert_eq!(report["signatures"][0]["field_name"], "Approval");
        assert_eq!(
            report["signatures"][0]["revocation_status"]["status"],
            "indeterminate"
        );
    }

    #[test]
    fn verify_signatures_command_requires_trust_anchors() {
        let dir = temp_dir("verify_signatures_command_requires_trust_anchors");
        let output = dir.join("signature-report.json");
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let code = run_with_io(
            [
                "oxidepdf",
                "pdf-sign",
                "verify",
                fixture_signature_pdf().to_str().unwrap(),
                "-o",
                output.to_str().unwrap(),
            ],
            [],
            &mut stdout,
            &mut stderr,
        );
        let stderr = String::from_utf8(stderr).unwrap();

        assert_eq!(code, 2);
        assert_eq!(stdout, b"");
        assert!(!output.exists());
        assert!(stderr.contains("--trust-anchors"));
    }

    #[test]
    fn metadata_commands_set_and_get_json_report() {
        let dir = temp_dir("metadata_commands_set_and_get_json_report");
        let input = dir.join("input.pdf");
        let edited = dir.join("metadata.pdf");
        let report = dir.join("metadata.json");
        fs::write(&input, empty_page_pdf()).unwrap();
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let code = run_with_io(
            [
                "oxidepdf",
                "metadata",
                "set",
                input.to_str().unwrap(),
                "--entry",
                "title=Quarterly Report",
                "--entry",
                "author=OxidePDF",
                "-o",
                edited.to_str().unwrap(),
            ],
            [],
            &mut stdout,
            &mut stderr,
        );
        assert_eq!(code, 0);
        assert_eq!(stderr, b"");

        let code = run_with_io(
            [
                "oxidepdf",
                "metadata",
                "get",
                edited.to_str().unwrap(),
                "-o",
                report.to_str().unwrap(),
            ],
            [],
            &mut stdout,
            &mut stderr,
        );
        assert_eq!(code, 0);
        let report: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(report).unwrap()).unwrap();
        assert_eq!(report["entries"]["title"], "Quarterly Report");
        assert_eq!(report["entries"]["author"], "OxidePDF");
    }

    #[test]
    fn attachment_commands_add_list_extract_and_delete() {
        let dir = temp_dir("attachment_commands_add_list_extract_and_delete");
        let input = dir.join("input.pdf");
        let note = dir.join("note.txt");
        let attached = dir.join("attached.pdf");
        let report = dir.join("attachments.json");
        let extracted = dir.join("extracted.txt");
        let deleted = dir.join("deleted.pdf");
        fs::write(&input, empty_page_pdf()).unwrap();
        fs::write(&note, b"attachment bytes").unwrap();
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let code = run_with_io(
            [
                "oxidepdf",
                "attach",
                "add",
                input.to_str().unwrap(),
                note.to_str().unwrap(),
                "--description",
                "Review note",
                "-o",
                attached.to_str().unwrap(),
            ],
            [],
            &mut stdout,
            &mut stderr,
        );
        assert_eq!(code, 0);
        assert_eq!(stderr, b"");

        let code = run_with_io(
            [
                "oxidepdf",
                "attach",
                "list",
                attached.to_str().unwrap(),
                "-o",
                report.to_str().unwrap(),
            ],
            [],
            &mut stdout,
            &mut stderr,
        );
        assert_eq!(code, 0);
        let report: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&report).unwrap()).unwrap();
        assert_eq!(report["attachments"][0]["name"], "note.txt");

        let code = run_with_io(
            [
                "oxidepdf",
                "attach",
                "extract",
                attached.to_str().unwrap(),
                "--name",
                "note.txt",
                "-o",
                extracted.to_str().unwrap(),
            ],
            [],
            &mut stdout,
            &mut stderr,
        );
        assert_eq!(code, 0);
        assert_eq!(fs::read(&extracted).unwrap(), b"attachment bytes");

        let code = run_with_io(
            [
                "oxidepdf",
                "attach",
                "delete",
                attached.to_str().unwrap(),
                "--name",
                "note.txt",
                "-o",
                deleted.to_str().unwrap(),
            ],
            [],
            &mut stdout,
            &mut stderr,
        );
        assert_eq!(code, 0);
        assert_eq!(pdf_page_count(&deleted), 1);
    }

    #[test]
    fn commands_with_two_inputs_reject_shared_stdin() {
        let dir = temp_dir("commands_with_two_inputs_reject_shared_stdin");
        let outline_output = dir.join("outline.pdf");
        let attach_output = dir.join("attached.pdf");
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let code = run_with_io(
            [
                "oxidepdf",
                "outline",
                "set",
                "-",
                "--tree",
                "-",
                "-o",
                outline_output.to_str().unwrap(),
            ],
            empty_page_pdf(),
            &mut stdout,
            &mut stderr,
        );
        assert_eq!(code, 2);
        assert!(!outline_output.exists());
        assert!(String::from_utf8(stderr.clone())
            .unwrap()
            .contains("cannot read both inputs from stdin"));

        stdout.clear();
        stderr.clear();
        let code = run_with_io(
            [
                "oxidepdf",
                "attach",
                "add",
                "-",
                "-",
                "--name",
                "note.txt",
                "-o",
                attach_output.to_str().unwrap(),
            ],
            empty_page_pdf(),
            &mut stdout,
            &mut stderr,
        );
        assert_eq!(code, 2);
        assert!(!attach_output.exists());
        assert!(String::from_utf8(stderr)
            .unwrap()
            .contains("cannot read both inputs from stdin"));
    }

    #[test]
    fn annotation_and_interactive_commands_remove_selected_elements() {
        let dir = temp_dir("annotation_and_interactive_commands_remove_selected_elements");
        let input = dir.join("input.pdf");
        let annotated = dir.join("annotated.pdf");
        let report = dir.join("annotations.json");
        let removed = dir.join("removed.pdf");
        let empty_report = dir.join("empty-annotations.json");
        fs::write(&input, empty_page_pdf()).unwrap();
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let code = run_with_io(
            [
                "oxidepdf",
                "annot",
                "add",
                input.to_str().unwrap(),
                "--page",
                "1",
                "--id",
                "review-note",
                "--text",
                "Review this page",
                "-o",
                annotated.to_str().unwrap(),
            ],
            [],
            &mut stdout,
            &mut stderr,
        );
        assert_eq!(code, 0);

        let code = run_with_io(
            [
                "oxidepdf",
                "annot",
                "list",
                annotated.to_str().unwrap(),
                "-o",
                report.to_str().unwrap(),
            ],
            [],
            &mut stdout,
            &mut stderr,
        );
        assert_eq!(code, 0);
        let report: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&report).unwrap()).unwrap();
        assert_eq!(report["annotations"][0]["id"], "review-note");

        let code = run_with_io(
            [
                "oxidepdf",
                "interactive",
                "remove",
                annotated.to_str().unwrap(),
                "--annotations",
                "-o",
                removed.to_str().unwrap(),
            ],
            [],
            &mut stdout,
            &mut stderr,
        );
        assert_eq!(code, 0);
        let code = run_with_io(
            [
                "oxidepdf",
                "annot",
                "list",
                removed.to_str().unwrap(),
                "-o",
                empty_report.to_str().unwrap(),
            ],
            [],
            &mut stdout,
            &mut stderr,
        );
        assert_eq!(code, 0);
        let report: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(empty_report).unwrap()).unwrap();
        assert!(report["annotations"].as_array().unwrap().is_empty());
    }

    #[test]
    fn form_commands_fill_inspect_unlock_and_remove() {
        let dir = temp_dir("form_commands_fill_inspect_unlock_and_remove");
        let input = dir.join("input.pdf");
        let filled = dir.join("filled.pdf");
        let report = dir.join("forms.json");
        let unlocked = dir.join("unlocked.pdf");
        let removed = dir.join("removed.pdf");
        let empty_report = dir.join("empty-forms.json");
        fs::write(&input, form_pdf(true)).unwrap();
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let code = run_with_io(
            [
                "oxidepdf",
                "form",
                "fill",
                input.to_str().unwrap(),
                "--field",
                "customer=Ada",
                "-o",
                filled.to_str().unwrap(),
            ],
            [],
            &mut stdout,
            &mut stderr,
        );
        assert_eq!(code, 0);

        let code = run_with_io(
            [
                "oxidepdf",
                "form",
                "inspect",
                filled.to_str().unwrap(),
                "-o",
                report.to_str().unwrap(),
            ],
            [],
            &mut stdout,
            &mut stderr,
        );
        assert_eq!(code, 0);
        let report: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&report).unwrap()).unwrap();
        assert_eq!(report["fields"][0]["value"], "Ada");
        assert_eq!(report["fields"][0]["readonly"], true);

        let code = run_with_io(
            [
                "oxidepdf",
                "form",
                "unlock-readonly",
                filled.to_str().unwrap(),
                "-o",
                unlocked.to_str().unwrap(),
            ],
            [],
            &mut stdout,
            &mut stderr,
        );
        assert_eq!(code, 0);

        let code = run_with_io(
            [
                "oxidepdf",
                "form",
                "remove",
                unlocked.to_str().unwrap(),
                "-o",
                removed.to_str().unwrap(),
            ],
            [],
            &mut stdout,
            &mut stderr,
        );
        assert_eq!(code, 0);
        let code = run_with_io(
            [
                "oxidepdf",
                "form",
                "inspect",
                removed.to_str().unwrap(),
                "-o",
                empty_report.to_str().unwrap(),
            ],
            [],
            &mut stdout,
            &mut stderr,
        );
        assert_eq!(code, 0);
        let report: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(empty_report).unwrap()).unwrap();
        assert!(report["fields"].as_array().unwrap().is_empty());
    }

    #[test]
    fn stamp_overlay_image_and_color_commands_write_expected_outputs() {
        let dir = temp_dir("stamp_overlay_image_and_color_commands_write_expected_outputs");
        let input = dir.join("input.pdf");
        let overlay = dir.join("overlay.pdf");
        let stamped = dir.join("stamped.pdf");
        let overlaid = dir.join("overlaid.pdf");
        let image_report = dir.join("images.json");
        let extracted = dir.join("image.rgb");
        let image_added = dir.join("image-added.pdf");
        let image_deleted = dir.join("image-deleted.pdf");
        let colored = dir.join("colored.pdf");
        fs::write(&input, empty_page_pdf()).unwrap();
        fs::write(&overlay, fixture_pdf_bytes()).unwrap();
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let code = run_with_io(
            [
                "oxidepdf",
                "stamp",
                input.to_str().unwrap(),
                "--text",
                "APPROVED",
                "--font",
                "Helvetica",
                "-o",
                stamped.to_str().unwrap(),
            ],
            [],
            &mut stdout,
            &mut stderr,
        );
        assert_eq!(code, 0);
        assert!(page_has_content_operator(&stamped, 1, "Tj"));

        let code = run_with_io(
            [
                "oxidepdf",
                "overlay-pdf",
                stamped.to_str().unwrap(),
                overlay.to_str().unwrap(),
                "--source-page",
                "1",
                "-o",
                overlaid.to_str().unwrap(),
            ],
            [],
            &mut stdout,
            &mut stderr,
        );
        assert_eq!(code, 0);
        assert!(page_has_content_operator(&overlaid, 1, "Do"));

        let code = run_with_io(
            [
                "oxidepdf",
                "image",
                "list",
                overlaid.to_str().unwrap(),
                "-o",
                image_report.to_str().unwrap(),
            ],
            [],
            &mut stdout,
            &mut stderr,
        );
        assert_eq!(code, 0);
        let report: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&image_report).unwrap()).unwrap();
        assert!(report["images"].as_array().unwrap().is_empty());

        let code = run_with_io(
            [
                "oxidepdf",
                "image",
                "add",
                overlaid.to_str().unwrap(),
                fixture_jpg().to_str().unwrap(),
                "--name",
                "Logo",
                "--page",
                "1",
                "-o",
                image_added.to_str().unwrap(),
            ],
            [],
            &mut stdout,
            &mut stderr,
        );
        assert_eq!(code, 0);

        let code = run_with_io(
            [
                "oxidepdf",
                "image",
                "extract",
                image_added.to_str().unwrap(),
                "--name",
                "Logo",
                "-o",
                extracted.to_str().unwrap(),
            ],
            [],
            &mut stdout,
            &mut stderr,
        );
        assert_eq!(code, 0);
        assert!(!fs::read(&extracted).unwrap().is_empty());

        let code = run_with_io(
            [
                "oxidepdf",
                "image",
                "delete",
                image_added.to_str().unwrap(),
                "--name",
                "Logo",
                "-o",
                image_deleted.to_str().unwrap(),
            ],
            [],
            &mut stdout,
            &mut stderr,
        );
        assert_eq!(code, 0);

        let color_input = dir.join("color.pdf");
        fs::write(&color_input, pdf_with_rgb_fill_content()).unwrap();
        let code = run_with_io(
            [
                "oxidepdf",
                "color",
                "invert",
                color_input.to_str().unwrap(),
                "-o",
                colored.to_str().unwrap(),
            ],
            [],
            &mut stdout,
            &mut stderr,
        );
        assert_eq!(code, 0);
        assert_eq!(pdf_rgb_operator(&colored, 1, "rg"), Some([0.0, 1.0, 1.0]));
    }

    #[test]
    fn workflow_img2pdf_writes_parseable_pdf() {
        let dir = temp_dir("workflow_img2pdf_writes_parseable_pdf");
        let workflow = dir.join("workflow.yaml");
        let output = dir.join("image.pdf");
        fs::write(
            &workflow,
            format!(
                r#"
                version: 1
                inputs:
                  - id: source
                    path: {}
                tasks:
                  - id: convert
                    op:
                      pdf_edit:
                        image_to_pdf:
                          layout: original_size
                    inputs: [source]
                outputs:
                  - id: final
                    from: convert
                    path: {}
                "#,
                yaml_path(fixture_jpg()),
                yaml_path(&output)
            ),
        )
        .unwrap();
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let code = run_with_io(
            ["oxidepdf", "run", "--workflow", workflow.to_str().unwrap()],
            [],
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 0);
        assert_eq!(stdout, b"");
        assert_eq!(stderr, b"");
        assert_eq!(pdf_page_count(&output), 1);
    }

    #[test]
    fn workflow_extract_text_writes_plain_text() {
        let dir = temp_dir("workflow_extract_text_writes_plain_text");
        let workflow = dir.join("workflow.yaml");
        let output = dir.join("extracted.txt");
        fs::write(
            &workflow,
            format!(
                r#"
                version: 1
                inputs:
                  - id: source
                    path: {}
                tasks:
                  - id: extract
                    op:
                      pdf_inspect:
                        extract_text:
                          format: plain
                    inputs: [source]
                outputs:
                  - id: final
                    from: extract
                    path: {}
                "#,
                yaml_path(fixture_pdf()),
                yaml_path(&output)
            ),
        )
        .unwrap();
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let code = run_with_io(
            ["oxidepdf", "run", "--workflow", workflow.to_str().unwrap()],
            [],
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 0);
        assert_eq!(stdout, b"");
        assert_eq!(stderr, b"");
        assert!(!fs::read_to_string(output).unwrap().trim().is_empty());
    }

    #[test]
    fn workflow_signature_operator_writes_json_report() {
        let dir = temp_dir("workflow_signature_operator_writes_json_report");
        let input = write_signature_pdf(&dir);
        let workflow = dir.join("workflow.yaml");
        let output = dir.join("signature-report.json");
        let trust_anchors = write_test_trust_anchors(&dir);
        fs::write(
            &workflow,
            format!(
                r#"
                version: 1
                inputs:
                  - id: source
                    path: {}
                tasks:
                  - id: verify
                    op:
                      pdf_sign:
                        verify:
                          mode: verify
                          trust_anchors: {}
                    inputs: [source]
                outputs:
                  - id: final
                    from: verify
                    path: {}
                "#,
                yaml_path(&input),
                yaml_path(&trust_anchors),
                yaml_path(&output)
            ),
        )
        .unwrap();
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let code = run_with_io(
            ["oxidepdf", "run", "--workflow", workflow.to_str().unwrap()],
            [],
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 0);
        assert_eq!(stdout, b"");
        assert_eq!(stderr, b"");
        let report: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(output).unwrap()).unwrap();
        assert_eq!(report["verdict"], "unsupported");
        assert_eq!(report["trust_anchor_count"], 1);
    }

    #[test]
    fn workflow_signature_operator_without_trust_anchors_fails_with_invalid_input() {
        let dir = temp_dir("workflow_signature_operator_without_trust_anchors_fails");
        let workflow = dir.join("workflow.yaml");
        let output = dir.join("signature-report.json");
        fs::write(
            &workflow,
            format!(
                r#"
                version: 1
                inputs:
                  - id: source
                    path: {}
                tasks:
                  - id: verify
                    op:
                      pdf_sign:
                        verify:
                          mode: verify
                    inputs: [source]
                outputs:
                  - id: final
                    from: verify
                    path: {}
                "#,
                yaml_path(fixture_signature_pdf()),
                yaml_path(&output)
            ),
        )
        .unwrap();
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let code = run_with_io(
            ["oxidepdf", "run", "--workflow", workflow.to_str().unwrap()],
            [],
            &mut stdout,
            &mut stderr,
        );
        let stderr = String::from_utf8(stderr).unwrap();

        assert_eq!(code, 3);
        assert_eq!(stdout, b"");
        assert!(!output.exists());
        assert!(stderr.contains("invalid_input"));
        assert!(stderr.contains("signature verification requires explicit trust anchors"));
    }

    #[test]
    fn invalid_workflow_returns_usage_exit_code() {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let code = run_with_io(
            ["oxidepdf", "run", "--workflow", "-"],
            b"version: 1\ninputs: []\ntasks: []\n",
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 2);
        assert_eq!(stdout, b"");
        assert!(String::from_utf8(stderr).unwrap().contains("workflow"));
    }

    #[test]
    fn missing_input_file_returns_input_exit_code() {
        let dir = temp_dir("missing_input_file_returns_input_exit_code");
        let workflow = dir.join("workflow.yaml");
        fs::write(
            &workflow,
            format!(
                r#"
                version: 1
                inputs:
                  - id: source
                    path: {}
                tasks: []
                outputs:
                  - id: final
                    from: source
                    path: {}
                "#,
                yaml_path(dir.join("missing.bin")),
                yaml_path(dir.join("out.bin"))
            ),
        )
        .unwrap();
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let code = run_with_io(
            ["oxidepdf", "run", "--workflow", workflow.to_str().unwrap()],
            [],
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 3);
        assert_eq!(stdout, b"");
        assert!(String::from_utf8(stderr).unwrap().contains("input"));
    }

    #[test]
    fn output_file_is_not_overwritten_without_force() {
        let dir = temp_dir("output_file_is_not_overwritten_without_force");
        let workflow = dir.join("workflow.yaml");
        let output = dir.join("out.bin");
        fs::write(dir.join("input.bin"), b"input bytes").unwrap();
        fs::write(&output, b"existing").unwrap();
        fs::write(
            &workflow,
            format!(
                r#"
                version: 1
                inputs:
                  - id: source
                    path: {}
                tasks: []
                outputs:
                  - id: final
                    from: source
                    path: {}
                "#,
                yaml_path(dir.join("input.bin")),
                yaml_path(&output)
            ),
        )
        .unwrap();
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let code = run_with_io(
            ["oxidepdf", "run", "--workflow", workflow.to_str().unwrap()],
            [],
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 2);
        assert_eq!(fs::read(output).unwrap(), b"existing");
        assert!(String::from_utf8(stderr)
            .unwrap()
            .contains("already exists"));
    }

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("oxidepdf_cli_{}_{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn yaml_path(path: impl AsRef<std::path::Path>) -> String {
        path.as_ref().display().to_string()
    }

    fn fixture_pdf() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/test.pdf")
            .canonicalize()
            .unwrap()
    }

    fn fixture_jpg() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/test.jpg")
            .canonicalize()
            .unwrap()
    }

    fn fixture_signature_pdf() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/signature-placeholder.pdf")
            .canonicalize()
            .unwrap()
    }

    fn write_test_trust_anchors(dir: &Path) -> std::path::PathBuf {
        let path = dir.join("anchors.pem");
        fs::write(
            &path,
            include_bytes!("../../../tests/fixtures/test-trust-anchor.txt"),
        )
        .unwrap();
        path
    }

    fn write_signature_pdf(dir: &Path) -> std::path::PathBuf {
        let path = dir.join("signed.pdf");
        let mut document = lopdf::Document::with_version("1.7");
        let pages_id = document.new_object_id();
        let page_id = document.new_object_id();
        let sig_field_id = document.new_object_id();
        let sig_value_id = document.new_object_id();
        let acroform_id = document.new_object_id();
        let catalog_id = document.new_object_id();

        let sig_value = lopdf::dictionary! {
            "Type" => "Sig",
            "Filter" => "Adobe.PPKLite",
            "SubFilter" => "adbe.pkcs7.detached",
            "ByteRange" => lopdf::Object::Array(vec![0.into(), 64.into(), 192.into(), 64.into()]),
            "Contents" => lopdf::Object::String(vec![0x30, 0x82], lopdf::StringFormat::Hexadecimal),
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

        document.save(&path).unwrap();
        path
    }

    fn simple_svg() -> &'static [u8] {
        br##"<svg xmlns="http://www.w3.org/2000/svg" width="120" height="80">
            <rect x="10" y="10" width="100" height="60" fill="#2563eb"/>
        </svg>"##
    }

    fn image_only_pdf() -> Vec<u8> {
        oxidepdf_core::image_artifacts_to_pdf(
            &[Artifact::image(fixture_jpg_bytes())],
            &ImageToPdfOptions::default(),
            &Default::default(),
        )
        .unwrap()
        .bytes
    }

    fn fixture_jpg_bytes() -> Vec<u8> {
        fs::read(fixture_jpg()).unwrap()
    }

    fn fixture_pdf_bytes() -> Vec<u8> {
        fs::read(fixture_pdf()).unwrap()
    }

    fn pdf_page_count(path: &std::path::Path) -> usize {
        lopdf::Document::load(path).unwrap().get_pages().len()
    }

    fn pdf_page_rotation(path: &std::path::Path, page_number: u32) -> i64 {
        let document = lopdf::Document::load(path).unwrap();
        let page_id = document.get_pages().get(&page_number).copied().unwrap();
        let page = document.get_object(page_id).unwrap().as_dict().unwrap();
        page.get(b"Rotate")
            .and_then(lopdf::Object::as_i64)
            .unwrap_or(0)
    }

    fn pdf_page_box(path: &std::path::Path, page_number: u32, key: &[u8]) -> [f32; 4] {
        let document = lopdf::Document::load(path).unwrap();
        let page_id = document.get_pages().get(&page_number).copied().unwrap();
        let page = document.get_object(page_id).unwrap().as_dict().unwrap();
        let values = page.get(key).unwrap().as_array().unwrap();
        [
            pdf_object_to_f32(&values[0]),
            pdf_object_to_f32(&values[1]),
            pdf_object_to_f32(&values[2]),
            pdf_object_to_f32(&values[3]),
        ]
    }

    fn pdf_object_to_f32(object: &lopdf::Object) -> f32 {
        match object {
            lopdf::Object::Integer(value) => *value as f32,
            lopdf::Object::Real(value) => *value,
            other => panic!("unexpected page box value: {other:?}"),
        }
    }

    fn pdf_page_xobject_count(path: &std::path::Path, page_number: u32) -> usize {
        let document = lopdf::Document::load(path).unwrap();
        let page_id = document.get_pages().get(&page_number).copied().unwrap();
        let page = document.get_object(page_id).unwrap().as_dict().unwrap();
        let resources = page.get(b"Resources").unwrap().as_dict().unwrap();
        resources
            .get(b"XObject")
            .and_then(lopdf::Object::as_dict)
            .map(|dictionary| dictionary.len())
            .unwrap_or(0)
    }

    fn pdf_page_content_contains(path: &std::path::Path, page_number: u32, expected: &str) -> bool {
        let document = lopdf::Document::load(path).unwrap();
        let page_id = document.get_pages().get(&page_number).copied().unwrap();
        String::from_utf8_lossy(&document.get_page_content(page_id).unwrap()).contains(expected)
    }

    fn pdf_rgb_operator(
        path: &std::path::Path,
        page_number: u32,
        operator: &str,
    ) -> Option<[f32; 3]> {
        let document = lopdf::Document::load(path).unwrap();
        let page_id = document.get_pages().get(&page_number).copied().unwrap();
        let content = document.get_page_content(page_id).ok()?;
        let content = lopdf::content::Content::decode(&content).ok()?;
        content.operations.iter().find_map(|operation| {
            if operation.operator == operator && operation.operands.len() == 3 {
                Some([
                    pdf_object_to_f32(&operation.operands[0]),
                    pdf_object_to_f32(&operation.operands[1]),
                    pdf_object_to_f32(&operation.operands[2]),
                ])
            } else {
                None
            }
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
            lopdf::Object::Stream(lopdf::Stream::new(lopdf::Dictionary::new(), marked_content)),
        );
        document.objects.insert(
            blank_page_id,
            lopdf::Object::Dictionary(lopdf::dictionary! {
                "Type" => "Page",
                "Parent" => pages_id,
                "MediaBox" => lopdf::Object::Array(vec![0.into(), 0.into(), 100.into(), 100.into()]),
            }),
        );
        document.objects.insert(
            marked_page_id,
            lopdf::Object::Dictionary(lopdf::dictionary! {
                "Type" => "Page",
                "Parent" => pages_id,
                "MediaBox" => lopdf::Object::Array(vec![0.into(), 0.into(), 100.into(), 100.into()]),
                "Contents" => marked_content_id,
            }),
        );
        document.objects.insert(
            pages_id,
            lopdf::Object::Dictionary(lopdf::dictionary! {
                "Type" => "Pages",
                "Kids" => lopdf::Object::Array(vec![blank_page_id.into(), marked_page_id.into()]),
                "Count" => 2,
            }),
        );
        document.objects.insert(
            catalog_id,
            lopdf::Object::Dictionary(lopdf::dictionary! {
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
        let mut document = lopdf::Document::with_version("1.7");
        let pages_id = document.new_object_id();
        let page_id = document.new_object_id();
        let catalog_id = document.new_object_id();
        document.objects.insert(
            page_id,
            lopdf::Object::Dictionary(lopdf::dictionary! {
                "Type" => "Page",
                "Parent" => pages_id,
                "MediaBox" => lopdf::Object::Array(vec![0.into(), 0.into(), 595.into(), 842.into()]),
            }),
        );
        document.objects.insert(
            pages_id,
            lopdf::Object::Dictionary(lopdf::dictionary! {
                "Type" => "Pages",
                "Kids" => lopdf::Object::Array(vec![page_id.into()]),
                "Count" => 1,
            }),
        );
        document.objects.insert(
            catalog_id,
            lopdf::Object::Dictionary(lopdf::dictionary! {
                "Type" => "Catalog",
                "Pages" => pages_id,
            }),
        );
        document.trailer.set("Root", catalog_id);

        let mut bytes = Vec::new();
        document.save_to(&mut bytes).unwrap();
        bytes
    }

    fn form_pdf(readonly: bool) -> Vec<u8> {
        let mut document = lopdf::Document::with_version("1.7");
        let pages_id = document.new_object_id();
        let page_id = document.new_object_id();
        let field_id = document.new_object_id();
        let acroform_id = document.new_object_id();
        let catalog_id = document.new_object_id();
        let flags = if readonly { 1 } else { 0 };

        document.objects.insert(
            field_id,
            lopdf::Object::Dictionary(lopdf::dictionary! {
                "FT" => "Tx",
                "T" => lopdf::Object::string_literal("customer"),
                "V" => lopdf::Object::string_literal(""),
                "Ff" => flags,
                "Rect" => lopdf::Object::Array(vec![10.into(), 10.into(), 120.into(), 30.into()]),
                "P" => page_id,
            }),
        );
        document.objects.insert(
            page_id,
            lopdf::Object::Dictionary(lopdf::dictionary! {
                "Type" => "Page",
                "Parent" => pages_id,
                "MediaBox" => lopdf::Object::Array(vec![0.into(), 0.into(), 200.into(), 200.into()]),
                "Annots" => lopdf::Object::Array(vec![field_id.into()]),
            }),
        );
        document.objects.insert(
            pages_id,
            lopdf::Object::Dictionary(lopdf::dictionary! {
                "Type" => "Pages",
                "Kids" => lopdf::Object::Array(vec![page_id.into()]),
                "Count" => 1,
            }),
        );
        document.objects.insert(
            acroform_id,
            lopdf::Object::Dictionary(lopdf::dictionary! {
                "Fields" => lopdf::Object::Array(vec![field_id.into()]),
            }),
        );
        document.objects.insert(
            catalog_id,
            lopdf::Object::Dictionary(lopdf::dictionary! {
                "Type" => "Catalog",
                "Pages" => pages_id,
                "AcroForm" => acroform_id,
            }),
        );
        document.trailer.set("Root", catalog_id);

        let mut bytes = Vec::new();
        document.save_to(&mut bytes).unwrap();
        bytes
    }

    fn pdf_with_rgb_fill_content() -> Vec<u8> {
        let mut document = lopdf::Document::with_version("1.7");
        let pages_id = document.new_object_id();
        let page_id = document.new_object_id();
        let content_id = document.new_object_id();
        let catalog_id = document.new_object_id();
        let content = lopdf::content::Content {
            operations: vec![
                lopdf::content::Operation::new(
                    "rg",
                    vec![
                        lopdf::Object::Real(1.0),
                        lopdf::Object::Real(0.0),
                        lopdf::Object::Real(0.0),
                    ],
                ),
                lopdf::content::Operation::new(
                    "re",
                    vec![0.into(), 0.into(), 100.into(), 100.into()],
                ),
                lopdf::content::Operation::new("f", Vec::new()),
            ],
        }
        .encode()
        .unwrap();
        document.objects.insert(
            content_id,
            lopdf::Object::Stream(lopdf::Stream::new(lopdf::Dictionary::new(), content)),
        );
        document.objects.insert(
            page_id,
            lopdf::Object::Dictionary(lopdf::dictionary! {
                "Type" => "Page",
                "Parent" => pages_id,
                "MediaBox" => lopdf::Object::Array(vec![0.into(), 0.into(), 100.into(), 100.into()]),
                "Contents" => content_id,
            }),
        );
        document.objects.insert(
            pages_id,
            lopdf::Object::Dictionary(lopdf::dictionary! {
                "Type" => "Pages",
                "Kids" => lopdf::Object::Array(vec![page_id.into()]),
                "Count" => 1,
            }),
        );
        document.objects.insert(
            catalog_id,
            lopdf::Object::Dictionary(lopdf::dictionary! {
                "Type" => "Catalog",
                "Pages" => pages_id,
            }),
        );
        document.trailer.set("Root", catalog_id);

        let mut bytes = Vec::new();
        document.save_to(&mut bytes).unwrap();
        bytes
    }

    fn page_has_content_operator(path: &std::path::Path, page_number: u32, operator: &str) -> bool {
        let document = lopdf::Document::load(path).unwrap();
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
}
