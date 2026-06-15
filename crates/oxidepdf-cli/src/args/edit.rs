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

