#![forbid(unsafe_code)]

use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::{generate, shells::Bash};
use oxidepdf_core::{
    execute_workflow, AnnotationEditAction, AnnotationEditOptions, AnnotationInspectOptions,
    Artifact, ArtifactRef, ArtifactStore, AttachmentEditAction, AttachmentEditOptions,
    AttachmentExtractOptions, AttachmentInspectOptions, BookletOptions, ColorEditAction,
    ColorEditOptions, CompareOptions, CompressionImageFormat, CompressionImageOptions,
    CompressionMode, CompressionOptions, CropPagesOptions, DeleteBlankPagesOptions,
    ExtractTextOptions, FormFieldValue, FormFillOptions, FormInspectOptions, ImageEditAction,
    ImageEditOptions, ImageExtractOptions, ImageInspectOptions, ImageToPdfOptions,
    InteractiveRemovalOptions, MergeOptions, MetadataEditAction, MetadataEditOptions,
    MetadataEntry, MetadataInspectOptions, NUpOptions, OperatorSpec, OutlineEditAction,
    OutlineEditOptions, OutlineInspectOptions, OutlineTree, OverlayKind, OverlayOptions,
    OxideError, PageNumberPosition, PageNumbersOptions, PageSelectionOptions, PdfCompareOptions,
    PdfEditOptions, PdfInspectOptions, PdfOperatorRunner, PdfSecurityOptions, PdfSignOptions,
    PermissionPolicy, RenderOptions, ReorderOptions, RotateOptions, ScalePagesOptions,
    SecurityDecryptOptions, SecurityEncryptOptions, SecurityPermissionGetOptions,
    SecurityPermissionSetOptions, SignatureAddOptions, SignatureDeleteFieldOptions,
    SignatureOptions, SinglePageOptions, SplitOptions, SvgToPdfOptions, TaskId, TaskSpec,
    TimestampAddOptions, VisualDiffOptions, WatermarkKind, WatermarkOptions, Workflow,
    WorkflowMetadata, WorkflowVersion,
};
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

include!("args.rs");

mod adv;
mod compare;
mod completion;
mod edit;
mod inspect;
mod security;
mod sign;
mod stdin;
mod workflow;

use adv::*;
use compare::*;
use completion::*;
use edit::*;
use inspect::*;
use security::*;
use sign::*;
use stdin::*;
use workflow::*;

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

impl From<CliCompressionMode> for CompressionMode {
    fn from(value: CliCompressionMode) -> Self {
        match value {
            CliCompressionMode::Lossless => Self::Lossless,
            CliCompressionMode::Lossy => Self::Lossy,
        }
    }
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
        Some(Commands::PdfSecurity(command)) => run_pdf_security(command, stdin, stdout),
        Some(Commands::PdfCompare(command)) => run_compare(command, stdin, stdout),
        Some(Commands::PdfSign(command)) => run_sign(command, stdin, stdout),
        Some(Commands::PdfAdv(command)) => run_pdf_adv(command, stdin, stdout),
        Some(Commands::Completion(command)) => run_completion(command, stdout),
        None => Ok(()),
    }
}

fn execute_and_write_workflow(
    workflow: Workflow,
    stdin: &[u8],
    force: bool,
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    let store = load_inputs(&workflow, stdin)?;
    let runner = PdfOperatorRunner::with_limits(workflow.limits.clone());
    let result = execute_workflow(&workflow, store, &runner).map_err(CliError::Core)?;
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
