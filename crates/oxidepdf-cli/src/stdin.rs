use super::*;

pub(crate) fn stdin_for_args<I, S>(args: I) -> Result<Vec<u8>, CliError>
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

pub(crate) fn cli_reads_stdin(cli: &Cli) -> bool {
    match &cli.command {
        Some(Commands::Run(args)) => run_reads_stdin(args),
        Some(Commands::PdfEdit(command)) => pdf_edit_reads_stdin(command),
        Some(Commands::PdfInspect(command)) => pdf_inspect_reads_stdin(command),
        Some(Commands::PdfSecurity(command)) => pdf_security_reads_stdin(command),
        Some(Commands::PdfCompare(command)) => pdf_compare_reads_stdin(command),
        Some(Commands::PdfSign(command)) => sign_reads_stdin(command),
        Some(Commands::PdfAdv(command)) => pdf_adv_reads_stdin(command),
        Some(Commands::Completion(_)) => false,
        None => false,
    }
}

/// Decides whether `run` needs stdin. The workflow file itself may be `-`, or
/// the workflow (when read from disk) may declare an input with `path: "-"`.
/// Stdin can feed at most one consumer, so the file taking precedence here
/// matches `run_workflow`, which reads the workflow from stdin first.
fn run_reads_stdin(args: &RunArgs) -> bool {
    if is_stdio(&args.workflow) {
        return true;
    }
    let Ok(bytes) = std::fs::read(&args.workflow) else {
        // The file is unreadable; defer the error to run_workflow rather than
        // speculatively consuming stdin here.
        return false;
    };
    let Ok(workflow) = parse_workflow(&bytes, &args.workflow) else {
        return false;
    };
    workflow.inputs.iter().any(|input| is_stdio(&input.path))
}

pub(crate) fn pdf_edit_reads_stdin(command: &PdfEditCommand) -> bool {
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
        PdfEditCommand::Stamp(args) => is_stdio(&args.input),
        PdfEditCommand::OverlayPdf(args) => is_stdio(&args.input) || is_stdio(&args.overlay),
        PdfEditCommand::Color(command) => color_reads_stdin(command),
        PdfEditCommand::InteractiveRemove(args) => is_stdio(&args.input),
    }
}

pub(crate) fn pdf_inspect_reads_stdin(command: &PdfInspectCommand) -> bool {
    match command {
        PdfInspectCommand::Render(args) => is_stdio(&args.input),
        PdfInspectCommand::ExtractText(args) => is_stdio(&args.input),
    }
}

pub(crate) fn pdf_security_reads_stdin(command: &PdfSecurityCommand) -> bool {
    match command {
        PdfSecurityCommand::Encrypt(args) => is_stdio(&args.input),
        PdfSecurityCommand::Decrypt(args) => is_stdio(&args.input),
        PdfSecurityCommand::Permissions(command) => permissions_reads_stdin(command),
    }
}

pub(crate) fn pdf_compare_reads_stdin(command: &PdfCompareCommand) -> bool {
    match command {
        PdfCompareCommand::Report(args) => is_stdio(&args.left) || is_stdio(&args.right),
        PdfCompareCommand::VisualDiff(args) => is_stdio(&args.left) || is_stdio(&args.right),
    }
}

pub(crate) fn pdf_adv_reads_stdin(command: &PdfAdvCommand) -> bool {
    match command {
        PdfAdvCommand::Metadata(command) => metadata_reads_stdin(command),
        PdfAdvCommand::Outline(command) => outline_reads_stdin(command),
        PdfAdvCommand::Attach(command) => attach_reads_stdin(command),
        PdfAdvCommand::Annot(command) => annot_reads_stdin(command),
        PdfAdvCommand::Form(command) => form_reads_stdin(command),
        PdfAdvCommand::Image(command) => image_reads_stdin(command),
    }
}

pub(crate) fn sign_reads_stdin(command: &PdfSignCommand) -> bool {
    match command {
        PdfSignCommand::Add(args) => is_stdio(&args.input),
        PdfSignCommand::List(args) => is_stdio(&args.input),
        PdfSignCommand::Verify(args) => is_stdio(&args.input),
        PdfSignCommand::DeleteField(args) => is_stdio(&args.input),
        PdfSignCommand::Appearance(args) => is_stdio(&args.input),
        PdfSignCommand::Timestamp(args) => is_stdio(&args.input),
    }
}

pub(crate) fn metadata_reads_stdin(command: &MetadataCommand) -> bool {
    match command {
        MetadataCommand::Get(args) | MetadataCommand::Validate(args) => is_stdio(&args.input),
        MetadataCommand::Set(args) => is_stdio(&args.input),
        MetadataCommand::Delete(args) => is_stdio(&args.input),
    }
}

pub(crate) fn outline_reads_stdin(command: &OutlineCommand) -> bool {
    match command {
        OutlineCommand::Get(args) => is_stdio(&args.input),
        OutlineCommand::Set(args) => is_stdio(&args.input) || is_stdio(&args.tree),
        OutlineCommand::Delete(args) => is_stdio(&args.input),
    }
}

pub(crate) fn attach_reads_stdin(command: &AttachCommand) -> bool {
    match command {
        AttachCommand::Add(args) => is_stdio(&args.input) || is_stdio(&args.file),
        AttachCommand::List(args) => is_stdio(&args.input),
        AttachCommand::Extract(args) => is_stdio(&args.input),
        AttachCommand::Delete(args) => is_stdio(&args.input),
    }
}

pub(crate) fn annot_reads_stdin(command: &AnnotCommand) -> bool {
    match command {
        AnnotCommand::List(args) => is_stdio(&args.input),
        AnnotCommand::Add(args) => is_stdio(&args.input),
        AnnotCommand::Delete(args) => is_stdio(&args.input),
    }
}

pub(crate) fn form_reads_stdin(command: &FormCommand) -> bool {
    match command {
        FormCommand::Inspect(args) => is_stdio(&args.input),
        FormCommand::Fill(args) => is_stdio(&args.input),
        FormCommand::UnlockReadonly(args) | FormCommand::Remove(args) => is_stdio(&args.input),
    }
}

pub(crate) fn image_reads_stdin(command: &ImageCommand) -> bool {
    match command {
        ImageCommand::List(args) => is_stdio(&args.input),
        ImageCommand::Add(args) => is_stdio(&args.input) || is_stdio(&args.image),
        ImageCommand::Replace(args) => is_stdio(&args.input) || is_stdio(&args.image),
        ImageCommand::Delete(args) => is_stdio(&args.input),
        ImageCommand::Extract(args) => is_stdio(&args.input),
    }
}

pub(crate) fn color_reads_stdin(command: &ColorCommand) -> bool {
    match command {
        ColorCommand::Contrast(args) => is_stdio(&args.input),
        ColorCommand::Invert(args) => is_stdio(&args.input),
        ColorCommand::Replace(args) => is_stdio(&args.input),
    }
}

pub(crate) fn permissions_reads_stdin(command: &PermissionsCommand) -> bool {
    match command {
        PermissionsCommand::Get(args) => is_stdio(&args.input),
        PermissionsCommand::Set(args) => is_stdio(&args.input),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_file_input_does_not_require_stdin() {
        let stdin = stdin_for_args([
            "oxidepdf",
            "pdf_inspect",
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
            "pdf_inspect",
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
            "pdf_inspect",
            "extract-text",
            "-",
            "-o",
            "output.txt",
        ])
        .unwrap();

        assert!(cli_reads_stdin(&cli));
    }

    #[test]
    fn compare_stdio_input_requires_stdin() {
        let cli = Cli::try_parse_from([
            "oxidepdf",
            "pdf_compare",
            "report",
            "-",
            "right.pdf",
            "-o",
            "report.json",
        ])
        .unwrap();

        assert!(cli_reads_stdin(&cli));
    }
}
