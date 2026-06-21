use super::*;
use tempfile::NamedTempFile;

pub(crate) fn run_completion(
    command: CompletionCommand,
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    match command {
        CompletionCommand::Bash(args) => run_bash_completion(args, stdout),
    }
}

pub(crate) fn run_bash_completion(
    args: CompletionBashArgs,
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    let mut bytes = Vec::new();
    write_bash_completion(&mut bytes);

    if let Some(path) = args.output {
        write_completion_file(&path, &bytes, args.force)?;
        return Ok(());
    }

    stdout.write_all(&bytes).map_err(CliError::Io)
}

pub(crate) fn write_bash_completion(output: &mut impl Write) {
    let mut command = command();
    generate(Bash, &mut command, "oxidepdf", output);
}

pub(crate) fn write_completion_file(
    path: &Path,
    bytes: &[u8],
    force: bool,
) -> Result<(), CliError> {
    // Use the same atomic, symlink-safe, clobber-checked write path as every
    // other CLI output instead of a racy exists()+write that follows symlinks.
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut file = NamedTempFile::new_in(parent).map_err(CliError::Io)?;
    file.write_all(bytes).map_err(CliError::Io)?;
    persist_output_file(file, path, force)
}
