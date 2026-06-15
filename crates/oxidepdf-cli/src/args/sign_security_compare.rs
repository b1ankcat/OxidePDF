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

