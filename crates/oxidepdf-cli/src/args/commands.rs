/// OxidePDF command-line arguments.
#[derive(Debug, Parser)]
#[command(
    name = "oxidepdf",
    version,
    about = "Pure Rust PDF toolkit",
    long_about = "OxidePDF is a pure Rust PDF toolkit.",
    after_help = "Conventions:
  Use '-' anywhere an input or output path says stdin/stdout.
  Page ranges use one-based pages, for example '1', '1,3-5', or '2-'.
  Pass --force to overwrite an existing output file.

Common examples:
  oxidepdf pdf_edit merge a.pdf b.pdf -o merged.pdf
  oxidepdf pdf_inspect extract-text input.pdf -o text.txt
  oxidepdf pdf_sign verify signed.pdf -o report.json
  source <(oxidepdf completion bash)
  oxidepdf completion bash -o oxidepdf.bash --force"
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
    #[command(name = "pdf_edit")]
    #[command(subcommand)]
    PdfEdit(PdfEditCommand),
    /// Inspect or render PDF files.
    #[command(name = "pdf_inspect")]
    #[command(subcommand)]
    PdfInspect(PdfInspectCommand),
    /// Encrypt, decrypt, or manage PDF password permissions.
    #[command(name = "pdf_security")]
    #[command(subcommand)]
    PdfSecurity(PdfSecurityCommand),
    /// Compare PDF files.
    #[command(name = "pdf_compare")]
    #[command(subcommand)]
    PdfCompare(PdfCompareCommand),
    /// Sign, list, verify, or timestamp PDF signatures.
    #[command(name = "pdf_sign")]
    #[command(subcommand)]
    PdfSign(PdfSignCommand),
    /// Advanced metadata, outline, attachment, annotation, form, and image operations.
    #[command(name = "pdf_adv")]
    #[command(subcommand)]
    PdfAdv(PdfAdvCommand),
    /// Generate shell completion scripts.
    #[command(subcommand)]
    Completion(CompletionCommand),
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
    /// Add a text stamp to a PDF.
    Stamp(StampArgs),
    /// Overlay one PDF page onto another PDF.
    #[command(name = "overlay-pdf")]
    OverlayPdf(OverlayPdfArgs),
    /// Edit simple page colors.
    #[command(subcommand)]
    Color(ColorCommand),
    /// Remove selected interactive document elements.
    #[command(name = "interactive-remove")]
    InteractiveRemove(InteractiveRemoveArgs),
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
enum PdfSecurityCommand {
    /// Encrypt a PDF with owner and user passwords.
    Encrypt(SecurityEncryptArgs),
    /// Decrypt a password-protected PDF.
    Decrypt(SecurityDecryptArgs),
    /// Inspect or change password permission policy.
    #[command(subcommand)]
    Permissions(PermissionsCommand),
}

#[derive(Debug, Subcommand)]
enum PdfCompareCommand {
    /// Compare two PDFs and write a JSON difference report.
    Report(CompareReportArgs),
    /// Render a visual diff PNG between two PDFs.
    #[command(name = "visual-diff")]
    VisualDiff(CompareVisualDiffArgs),
}

#[derive(Debug, Subcommand)]
enum PdfSignCommand {
    /// Add a PDF digital signature.
    Add(SignAddArgs),
    /// List PDF signatures.
    List(ListSignaturesArgs),
    /// Verify PDF signatures and certificates.
    Verify(VerifySignaturesArgs),
    /// Delete a PDF signature field.
    #[command(name = "delete-field")]
    DeleteField(SignDeleteFieldArgs),
    /// Add visual signature appearance only.
    Appearance(SignatureAppearanceArgs),
    /// Add or inspect explicit timestamp material.
    Timestamp(TimestampAddArgs),
}

#[derive(Debug, Subcommand)]
enum PdfAdvCommand {
    /// Inspect, set, delete, or validate document metadata.
    #[command(subcommand)]
    Metadata(MetadataCommand),
    /// Inspect, set, or delete document outlines.
    #[command(subcommand)]
    Outline(OutlineCommand),
    /// Add, list, extract, or delete embedded file attachments.
    #[command(subcommand)]
    Attach(AttachCommand),
    /// List, add, or delete annotations.
    #[command(subcommand)]
    Annot(AnnotCommand),
    /// Fill, unlock, inspect, or remove interactive forms.
    #[command(subcommand)]
    Form(FormCommand),
    /// Inspect or edit image XObject resources.
    #[command(subcommand)]
    Image(ImageCommand),
}

#[derive(Debug, Subcommand)]
enum MetadataCommand {
    /// Write document metadata as JSON.
    Get(InspectOutputArgs),
    /// Set one or more document metadata entries.
    Set(MetadataSetArgs),
    /// Delete one or more document metadata keys.
    Delete(MetadataDeleteArgs),
    /// Validate metadata and write a JSON report.
    Validate(InspectOutputArgs),
}

#[derive(Debug, Subcommand)]
enum OutlineCommand {
    /// Write the document outline tree as JSON.
    Get(InspectOutputArgs),
    /// Replace the document outline tree from JSON.
    Set(OutlineSetArgs),
    /// Remove all document outlines.
    Delete(EditOutputArgs),
}

#[derive(Debug, Subcommand)]
enum AttachCommand {
    /// Embed a file attachment into a PDF.
    Add(AttachAddArgs),
    /// List embedded file attachments as JSON.
    List(InspectOutputArgs),
    /// Extract one embedded file attachment.
    Extract(AttachExtractArgs),
    /// Delete one embedded file attachment.
    Delete(AttachDeleteArgs),
}

#[derive(Debug, Subcommand)]
enum AnnotCommand {
    /// List annotations as JSON.
    List(InspectOutputArgs),
    /// Add a text annotation.
    Add(AnnotAddArgs),
    /// Delete an annotation by id.
    Delete(AnnotDeleteArgs),
}

#[derive(Debug, Subcommand)]
enum FormCommand {
    /// List form fields as JSON.
    Inspect(InspectOutputArgs),
    /// Fill form fields from name=value arguments.
    Fill(FormFillArgs),
    /// Clear readonly flags from form fields.
    #[command(name = "unlock-readonly")]
    UnlockReadonly(EditOutputArgs),
    /// Remove interactive form fields.
    Remove(EditOutputArgs),
}

#[derive(Debug, Subcommand)]
enum ImageCommand {
    /// List image XObject resources as JSON.
    List(InspectOutputArgs),
    /// Add an image XObject resource to a page.
    Add(ImageAddArgs),
    /// Replace an existing image XObject resource.
    Replace(ImageReplaceArgs),
    /// Delete an image XObject resource.
    Delete(ImageDeleteArgs),
    /// Extract raw image XObject bytes.
    Extract(ImageExtractArgs),
}

#[derive(Debug, Subcommand)]
enum ColorCommand {
    /// Adjust contrast on selected pages.
    Contrast(ColorContrastArgs),
    /// Invert simple page colors.
    Invert(ColorEditArgs),
    /// Replace one RGB color with another.
    Replace(ColorReplaceArgs),
}

#[derive(Debug, Subcommand)]
enum PermissionsCommand {
    /// Inspect password permission policy as JSON.
    Get(PermissionsGetArgs),
    /// Encrypt a PDF with a new permission policy.
    Set(PermissionsSetArgs),
}

#[derive(Debug, Subcommand)]
enum CompletionCommand {
    /// Generate a Bash completion script.
    Bash(CompletionBashArgs),
}

#[derive(Debug, Parser)]
struct CompletionBashArgs {
    /// Write to stdout so it can be loaded with `source <(oxidepdf completion bash)`.
    #[arg(long, conflicts_with = "output")]
    stdout: bool,

    /// Output completion script path. Defaults to stdout.
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Overwrite output files when they already exist.
    #[arg(long)]
    force: bool,

}

#[derive(Debug, Parser)]
struct RunArgs {
    /// Workflow YAML or JSON file, or `-` to read from stdin.
    #[arg(long)]
    workflow: PathBuf,

    /// Overwrite output files when they already exist.
    #[arg(long)]
    force: bool,

    /// Write workflow execution metrics as JSON to this file. '-' is not allowed.
    #[arg(long)]
    metrics_output: Option<PathBuf>,
}

