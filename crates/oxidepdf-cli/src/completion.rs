use super::*;

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
        write_completion_file(&path, &bytes, args.force).map_err(CliError::Io)?;
        return Ok(());
    }

    stdout.write_all(&bytes).map_err(CliError::Io)
}

pub(crate) fn write_bash_completion(output: &mut impl Write) {
    let mut command = command();
    generate(Bash, &mut command, "oxidepdf", output);
}

pub(crate) fn write_completion_file(path: &Path, bytes: &[u8], force: bool) -> io::Result<()> {
    if path.exists() && !force {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "output file already exists; pass --force to overwrite it",
        ));
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, bytes)
}
