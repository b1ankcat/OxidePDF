
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
    /// Source RGB color as `#RRGGBB` or three comma-separated 0.0-1.0 components.
    #[arg(long)]
    from: String,
    /// Replacement RGB color as `#RRGGBB` or three comma-separated 0.0-1.0 components.
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
