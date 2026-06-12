#![forbid(unsafe_code)]

use clap::{CommandFactory, Parser, Subcommand};
use oxidepdf_core::{
    execute_workflow, Artifact, ArtifactRef, ArtifactStore, ImageToPdfOptions, MergeOptions,
    OperatorSpec, OxideError, PdfOperatorRunner, RenderOptions, ReorderOptions, RotateOptions,
    SplitOptions, SvgToPdfOptions, TaskId, TaskSpec, Workflow, WorkflowMetadata, WorkflowVersion,
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
    /// Merge multiple PDFs into one output.
    Merge(MergeArgs),
    /// Keep selected pages from a PDF.
    Split(PageSelectionArgs),
    /// Reorder pages in a PDF.
    Reorder(PageSelectionArgs),
    /// Rotate selected PDF pages.
    Rotate(RotateArgs),
    /// Convert one or more images into PDF pages.
    #[command(name = "img2pdf")]
    Img2pdf(ImageToPdfArgs),
    /// Convert an SVG document into a PDF.
    #[command(name = "svg2pdf")]
    Svg2pdf(SvgToPdfArgs),
    /// Render a PDF page into a PNG image.
    Render(RenderArgs),
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
        Some(Commands::Merge(args)) => args.inputs.iter().any(|input| is_stdio(input)),
        Some(Commands::Split(args)) | Some(Commands::Reorder(args)) => is_stdio(&args.input),
        Some(Commands::Rotate(args)) => is_stdio(&args.input),
        Some(Commands::Img2pdf(args)) => args.inputs.iter().any(|input| is_stdio(input)),
        Some(Commands::Svg2pdf(args)) => is_stdio(&args.input),
        Some(Commands::Render(args)) => is_stdio(&args.input),
        None => false,
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
        Some(Commands::Merge(args)) => run_merge(args, stdin, stdout),
        Some(Commands::Split(args)) => run_page_selection(args, stdin, stdout, PageCommand::Split),
        Some(Commands::Reorder(args)) => {
            run_page_selection(args, stdin, stdout, PageCommand::Reorder)
        }
        Some(Commands::Rotate(args)) => run_rotate(args, stdin, stdout),
        Some(Commands::Img2pdf(args)) => run_img2pdf(args, stdin, stdout),
        Some(Commands::Svg2pdf(args)) => run_svg2pdf(args, stdin, stdout),
        Some(Commands::Render(args)) => run_render(args, stdin, stdout),
        None => Ok(()),
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
            op: OperatorSpec::Merge(MergeOptions {}),
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
        PageCommand::Split => "split",
        PageCommand::Reorder => "reorder",
    };
    let op = match command {
        PageCommand::Split => OperatorSpec::Split(SplitOptions { pages: args.pages }),
        PageCommand::Reorder => OperatorSpec::Reorder(ReorderOptions { pages: args.pages }),
    };
    let workflow = one_input_workflow(args.input, args.output, task_id, op);

    execute_and_write_workflow(workflow, stdin, args.force, stdout)
}

fn run_rotate(args: RotateArgs, stdin: &[u8], stdout: &mut impl Write) -> Result<(), CliError> {
    let workflow = one_input_workflow(
        args.input,
        args.output,
        "rotate",
        OperatorSpec::Rotate(RotateOptions {
            pages: args.pages,
            degrees: args.degrees,
        }),
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
            op: OperatorSpec::ImageToPdf(ImageToPdfOptions {
                layout: args.layout,
            }),
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
        OperatorSpec::SvgToPdf(SvgToPdfOptions {
            rasterize: args.rasterize,
        }),
    );

    execute_and_write_workflow(workflow, stdin, args.force, stdout)
}

fn run_render(args: RenderArgs, stdin: &[u8], stdout: &mut impl Write) -> Result<(), CliError> {
    let workflow = one_input_workflow(
        args.input,
        args.output,
        "render",
        OperatorSpec::Render(RenderOptions {
            page: args.page,
            format: Some("png".to_owned()),
            scale: args.scale,
        }),
    );

    execute_and_write_workflow(workflow, stdin, args.force, stdout)
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

#[derive(Debug, Clone, Copy)]
enum PageCommand {
    Split,
    Reorder,
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
    for input in &workflow.inputs {
        let bytes = read_path_or_stdin(&input.path, stdin).map_err(CliError::Input)?;
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
            Self::Arguments(_) | Self::Workflow(_) => 2,
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
            Self::Input(error) => write!(formatter, "input error: {error}"),
            Self::Io(error) => write!(formatter, "I/O error: {error}"),
            Self::Core(error) => write!(formatter, "{}: {error}", error.code()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
    }

    #[test]
    fn version_uses_package_version() {
        let command = command();
        let version = command.get_version().unwrap();

        assert_eq!(version, env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn render_file_input_does_not_require_stdin() {
        let stdin = stdin_for_args([
            "oxidepdf",
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
        let cli =
            Cli::try_parse_from(["oxidepdf", "render", "-", "--page", "1", "-o", "output.png"])
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
                      rotate:
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
        assert!(String::from_utf8(stderr).unwrap().contains("parse_pdf"));
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
                "split",
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
                "rotate",
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
    fn img2pdf_command_writes_parseable_pdf() {
        let dir = temp_dir("img2pdf_command_writes_parseable_pdf");
        let output = dir.join("image.pdf");
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let code = run_with_io(
            [
                "oxidepdf",
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

    fn simple_svg() -> &'static [u8] {
        br##"<svg xmlns="http://www.w3.org/2000/svg" width="120" height="80">
            <rect x="10" y="10" width="100" height="60" fill="#2563eb"/>
        </svg>"##
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
}
