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

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum CliCompressionImageFormat {
    Jpeg,
    Png,
    Webp,
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
struct SignAddArgs {
    /// Input PDF file, or `-` to read from stdin.
    input: PathBuf,

    /// Signature field name to create or fill.
    #[arg(long)]
    field_name: String,

    /// PEM file containing the signer certificate.
    #[arg(long)]
    certificate: PathBuf,

    /// PEM file containing the signer private key.
    #[arg(long)]
    private_key: PathBuf,

    /// Reserved signature Contents bytes.
    #[arg(long)]
    contents_reserved_bytes: Option<usize>,

    /// Optional visual signature appearance field to bind.
    #[arg(long)]
    appearance_field: Option<String>,

    /// Output signed PDF file, or `-` to write to stdout.
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
    trust_anchors: Option<PathBuf>,

    /// Output report file, or `-` to write to stdout.
    #[arg(short, long)]
    output: PathBuf,

    /// Overwrite output files when they already exist.
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Parser)]
struct ListSignaturesArgs {
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
struct SignDeleteFieldArgs {
    /// Input PDF file, or `-` to read from stdin.
    input: PathBuf,

    /// Signature field name to delete.
    #[arg(long)]
    field_name: String,

    /// Allow deleting a field that contains signature value material.
    #[arg(long)]
    destructive: bool,

    /// Output PDF file, or `-` to write to stdout.
    #[arg(short, long)]
    output: PathBuf,

    /// Overwrite output files when they already exist.
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Parser)]
struct TimestampAddArgs {
    /// Input PDF file, or `-` to read from stdin.
    input: PathBuf,

    /// Explicit TSA endpoint. Live TSA requests are not performed by this offline build.
    #[arg(long)]
    tsa_url: Option<String>,

    /// Explicit RFC 3161 timestamp token DER file.
    #[arg(long)]
    token: Option<PathBuf>,

    /// Output report file, or `-` to write to stdout.
    #[arg(short, long)]
    output: PathBuf,

    /// Overwrite output files when they already exist.
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Parser)]
struct SecurityEncryptArgs {
    /// Input PDF file, or `-` to read from stdin.
    input: PathBuf,

    /// Output encrypted PDF file, or `-` to write to stdout.
    #[arg(short, long)]
    output: PathBuf,

    /// Owner password used to control future permission changes.
    #[arg(long)]
    owner_password: String,

    /// User password required to open the document.
    #[arg(long)]
    user_password: String,

    #[command(flatten)]
    permissions: PermissionArgs,

    /// Overwrite output files when they already exist.
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Parser)]
struct SecurityDecryptArgs {
    /// Input PDF file, or `-` to read from stdin.
    input: PathBuf,

    /// Output decrypted PDF file, or `-` to write to stdout.
    #[arg(short, long)]
    output: PathBuf,

    /// Owner or user password.
    #[arg(long)]
    password: String,

    /// Overwrite output files when they already exist.
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Parser)]
struct PermissionsGetArgs {
    /// Input PDF file, or `-` to read from stdin.
    input: PathBuf,

    /// Output JSON report file, or `-` to write to stdout.
    #[arg(short, long)]
    output: PathBuf,

    /// Owner or user password for encrypted PDFs.
    #[arg(long)]
    password: Option<String>,

    /// Overwrite output files when they already exist.
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Parser)]
struct PermissionsSetArgs {
    /// Input PDF file, or `-` to read from stdin.
    input: PathBuf,

    /// Output encrypted PDF file, or `-` to write to stdout.
    #[arg(short, long)]
    output: PathBuf,

    /// Existing owner password for encrypted PDFs and owner password for the output PDF.
    #[arg(long)]
    owner_password: String,

    /// User password required to open the output document.
    #[arg(long)]
    user_password: String,

    #[command(flatten)]
    permissions: PermissionArgs,

    /// Overwrite output files when they already exist.
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Clone, Parser)]
struct PermissionArgs {
    /// Disallow printing.
    #[arg(long)]
    no_print: bool,

    /// Disallow document modifications.
    #[arg(long)]
    no_modify: bool,

    /// Disallow copying text and graphics.
    #[arg(long)]
    no_copy: bool,

    /// Disallow annotations.
    #[arg(long)]
    no_annotate: bool,

    /// Disallow filling form fields.
    #[arg(long)]
    no_fill_forms: bool,

    /// Disallow accessibility extraction.
    #[arg(long)]
    no_accessibility: bool,

    /// Disallow page assembly.
    #[arg(long)]
    no_assemble: bool,

    /// Disallow high quality printing.
    #[arg(long)]
    no_high_quality_print: bool,
}

#[derive(Debug, Parser)]
struct CompareReportArgs {
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
struct CompareVisualDiffArgs {
    /// Left input PDF file.
    left: PathBuf,

    /// Right input PDF file.
    right: PathBuf,

    /// Output PNG file, or `-` to write to stdout.
    #[arg(short, long)]
    output: PathBuf,

    /// One-based page for visual diff output.
    #[arg(long, default_value_t = 1)]
    page: u32,

    /// Render scale for visual diff output.
    #[arg(long)]
    scale: Option<f32>,

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

    /// Remove page annotations.
    #[arg(long)]
    annotations: bool,
    /// Remove interactive form fields.
    #[arg(long)]
    forms: bool,
    /// Remove document and annotation actions.
    #[arg(long)]
    actions: bool,
    /// Remove JavaScript actions and name-tree entries.
    #[arg(long)]
    javascript: bool,
    /// Remove embedded file attachments.
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
    /// Stamp text to draw on selected pages.
    #[arg(long)]
    text: String,
    /// Font family for stamp text. Defaults to Helvetica.
    #[arg(long)]
    font: Option<String>,
    /// Explicit font file for stamp text.
    #[arg(long)]
    font_path: Option<PathBuf>,
    /// Font size in PDF points.
    #[arg(long)]
    font_size: Option<f32>,
    /// Page range, for example `1,3-5`. Defaults to all pages.
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
    /// Output PDF file, or `-` to write to stdout.
    #[arg(short, long)]
    output: PathBuf,
    /// Overwrite output files when they already exist.
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Parser)]
struct SignatureAppearanceArgs {
    /// Input PDF file, or `-` to read from stdin.
    input: PathBuf,
    /// Appearance text to draw.
    #[arg(long)]
    text: String,
    /// Font family for appearance text. Defaults to Helvetica.
    #[arg(long)]
    font: Option<String>,
    /// Explicit font file for appearance text.
    #[arg(long)]
    font_path: Option<PathBuf>,
    /// Font size in PDF points.
    #[arg(long)]
    font_size: Option<f32>,
    /// Page range, for example `1,3-5`. Defaults to all pages.
    #[arg(long)]
    pages: Option<String>,
    /// Position: `center`, `top_left`, `top_right`, `bottom_left`, or `bottom_right`.
    #[arg(long)]
    position: Option<String>,
    /// Output PDF file, or `-` to write to stdout.
    #[arg(short, long)]
    output: PathBuf,
    /// Overwrite output files when they already exist.
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Parser)]
struct OverlayPdfArgs {
    /// Input PDF file, or `-` to read from stdin.
    input: PathBuf,
    /// Overlay PDF file, or `-` to read from stdin.
    overlay: PathBuf,
    /// One-based page from the overlay PDF. Defaults to 1.
    #[arg(long)]
    source_page: Option<u32>,
    /// Target page range, for example `1,3-5`. Defaults to all pages.
    #[arg(long)]
    pages: Option<String>,
    /// Opacity from 0.0 to 1.0.
    #[arg(long)]
    opacity: Option<f32>,
    /// Overlay scale factor.
    #[arg(long)]
    scale: Option<f32>,
    /// Position: `center`, `top_left`, `top_right`, `bottom_left`, or `bottom_right`.
    #[arg(long)]
    position: Option<String>,
    /// Output PDF file, or `-` to write to stdout.
    #[arg(short, long)]
    output: PathBuf,
    /// Overwrite output files when they already exist.
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Parser)]
struct ImageAddArgs {
    /// Input PDF file, or `-` to read from stdin.
    input: PathBuf,
    /// Image file to add.
    image: PathBuf,
    /// Resource name for the image XObject.
    #[arg(long)]
    name: String,
    /// One-based page number where the image resource is added.
    #[arg(long)]
    page: u32,
    /// Output PDF file, or `-` to write to stdout.
    #[arg(short, long)]
    output: PathBuf,
    /// Overwrite output files when they already exist.
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Parser)]
struct ImageReplaceArgs {
    /// Input PDF file, or `-` to read from stdin.
    input: PathBuf,
    /// Replacement image file.
    image: PathBuf,
    /// Existing image XObject resource name.
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
struct ImageDeleteArgs {
    /// Input PDF file, or `-` to read from stdin.
    input: PathBuf,
    /// Image XObject resource name to delete.
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
struct ImageExtractArgs {
    /// Input PDF file, or `-` to read from stdin.
    input: PathBuf,
    /// Image XObject resource name to extract.
    #[arg(long)]
    name: String,
    /// Output image bytes file, or `-` to write to stdout.
    #[arg(short, long)]
    output: PathBuf,
    /// Overwrite output files when they already exist.
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Parser)]
struct ColorEditArgs {
    /// Input PDF file, or `-` to read from stdin.
    input: PathBuf,
    /// Page range, for example `1,3-5`. Defaults to all pages.
    #[arg(long)]
    pages: Option<String>,
    /// Output PDF file, or `-` to write to stdout.
    #[arg(short, long)]
    output: PathBuf,
    /// Overwrite output files when they already exist.
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Parser)]
struct ColorContrastArgs {
    /// Input PDF file, or `-` to read from stdin.
    input: PathBuf,
    /// Contrast multiplier.
    #[arg(long)]
    factor: f32,
    /// Page range, for example `1,3-5`. Defaults to all pages.
    #[arg(long)]
    pages: Option<String>,
    /// Output PDF file, or `-` to write to stdout.
    #[arg(short, long)]
    output: PathBuf,
    /// Overwrite output files when they already exist.
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Parser)]
struct ColorReplaceArgs {
    /// Input PDF file, or `-` to read from stdin.
    input: PathBuf,
    /// Source RGB color as `#RRGGBB` or `r,g,b`.
    #[arg(long)]
    from: String,
    /// Replacement RGB color as `#RRGGBB` or `r,g,b`.
    #[arg(long)]
    to: String,
    /// Page range, for example `1,3-5`. Defaults to all pages.
    #[arg(long)]
    pages: Option<String>,
    /// Output PDF file, or `-` to write to stdout.
    #[arg(short, long)]
    output: PathBuf,
    /// Overwrite output files when they already exist.
    #[arg(long)]
    force: bool,
}
