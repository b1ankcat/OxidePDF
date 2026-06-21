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
