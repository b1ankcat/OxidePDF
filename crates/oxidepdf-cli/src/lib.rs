#![forbid(unsafe_code)]

use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::{generate, shells::Bash};
use oxidepdf_core::{
    AnnotationEditAction, AnnotationEditOptions, AnnotationInspectOptions, ArtifactRef,
    AttachmentEditAction, AttachmentEditOptions, AttachmentExtractOptions,
    AttachmentInspectOptions, BookletOptions, ColorEditAction, ColorEditOptions, CompareOptions,
    CompressionImageFormat, CompressionImageOptions, CompressionMode, CompressionOptions,
    CropPagesOptions, DeleteBlankPagesOptions, ExtractTextOptions, FormFieldValue, FormFillOptions,
    FormInspectOptions, ImageEditAction, ImageEditOptions, ImageExtractOptions,
    ImageInspectOptions, ImageToPdfOptions, InteractiveRemovalOptions, MergeOptions,
    MetadataEditAction, MetadataEditOptions, MetadataEntry, MetadataInspectOptions, NUpOptions,
    OperatorSpec, OutlineEditAction, OutlineEditOptions, OutlineInspectOptions, OutlineTree,
    OverlayKind, OverlayOptions, OxideError, PageNumberPosition, PageNumbersOptions,
    PageSelectionOptions, PdfCompareOptions, PdfEditOptions, PdfInspectOptions, PdfOperatorRunner,
    PdfSecurityOptions, PdfSignOptions, PermissionPolicy, RenderOptions, ReorderOptions,
    ResourceLimits, RotateOptions, ScalePagesOptions, SecurityDecryptOptions,
    SecurityEncryptOptions,
    SecurityPermissionGetOptions, SecurityPermissionSetOptions, SignatureAddOptions,
    SignatureDeleteFieldOptions, SignatureOptions, SinglePageOptions, SplitOptions,
    SvgToPdfOptions, TaskId, TaskSpec, TimestampAddOptions, VisualDiffOptions, WatermarkKind,
    WatermarkOptions, Workflow, WorkflowMetadata, WorkflowVersion, execute_workflow,
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
mod runtime;
mod security;
mod sign;
mod stdin;
mod workflow;

use adv::*;
use compare::*;
use completion::*;
use edit::*;
use inspect::*;
use runtime::*;
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
pub async fn run() -> i32 {
    let args = std::env::args_os().collect::<Vec<_>>();
    let stdin_buffer = match stdin_for_args(args.clone()) {
        Ok(buffer) => buffer,
        Err(CliError::Arguments(error)) => {
            // clap surfaces --help/--version as "errors" whose output belongs on
            // stdout with exit code 0; its other parse errors belong on stderr.
            // Defer to clap's own printer so the text is unprefixed and routed
            // correctly instead of mangling it with an "oxidepdf:" prefix.
            let _ = error.print();
            return error.exit_code();
        }
        Err(error) => {
            let _ = writeln!(io::stderr().lock(), "oxidepdf: {error}");
            return error.exit_code();
        }
    };
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    let stderr = io::stderr();
    let mut stderr = stderr.lock();

    run_with_io(args, &stdin_buffer, &mut stdout, &mut stderr).await
}

/// Runs the CLI with injectable IO for tests.
pub async fn run_with_io<I, S>(
    args: I,
    stdin: impl AsRef<[u8]>,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> i32
where
    I: IntoIterator<Item = S>,
    S: Into<std::ffi::OsString> + Clone,
{
    match run_with_io_result(args, stdin.as_ref(), stdout).await {
        Ok(()) => 0,
        Err(CliError::Arguments(error)) if is_help_or_version(&error) => {
            // Help/version "errors" are normal output: route them to stdout with
            // clap's own formatting and a success-class exit code.
            let _ = write!(stdout, "{error}");
            error.exit_code()
        }
        Err(error) => {
            let _ = writeln!(stderr, "oxidepdf: {error}");
            error.exit_code()
        }
    }
}

fn is_help_or_version(error: &clap::Error) -> bool {
    matches!(
        error.kind(),
        clap::error::ErrorKind::DisplayHelp
            | clap::error::ErrorKind::DisplayVersion
            | clap::error::ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand
    )
}

/// Returns the clap command definition for tests and generated help.
pub fn command() -> clap::Command {
    Cli::command()
}

async fn run_with_io_result<I, S>(
    args: I,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError>
where
    I: IntoIterator<Item = S>,
    S: Into<std::ffi::OsString> + Clone,
{
    let cli = Cli::try_parse_from(args).map_err(CliError::Arguments)?;
    match cli.command {
        Some(Commands::Run(args)) => run_workflow(args, stdin, stdout).await,
        Some(Commands::PdfEdit(command)) => run_pdf_edit(command, stdin, stdout).await,
        Some(Commands::PdfInspect(command)) => run_pdf_inspect(command, stdin, stdout).await,
        Some(Commands::PdfSecurity(command)) => run_pdf_security(command, stdin, stdout).await,
        Some(Commands::PdfCompare(command)) => run_compare(command, stdin, stdout).await,
        Some(Commands::PdfSign(command)) => run_sign(command, stdin, stdout).await,
        Some(Commands::PdfAdv(command)) => run_pdf_adv(command, stdin, stdout).await,
        Some(Commands::Completion(command)) => run_completion(command, stdout),
        None => Ok(()),
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
