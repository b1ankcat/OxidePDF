use super::*;

pub(crate) async fn run_pdf_security(
    command: PdfSecurityCommand,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    match command {
        PdfSecurityCommand::Encrypt(args) => run_encrypt(args, stdin, stdout).await,
        PdfSecurityCommand::Decrypt(args) => run_decrypt(args, stdin, stdout).await,
        PdfSecurityCommand::Permissions(command) => run_permissions(command, stdin, stdout).await,
    }
}

pub(crate) async fn run_encrypt(
    args: SecurityEncryptArgs,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    let owner_password =
        resolve_required_password("owner", args.owner_password, args.owner_password_file)?;
    let user_password =
        resolve_required_password("user", args.user_password, args.user_password_file)?;
    let workflow = one_input_workflow(
        args.input,
        args.output,
        "encrypt",
        OperatorSpec::PdfSecurity(PdfSecurityOptions::Encrypt(SecurityEncryptOptions {
            owner_password,
            user_password,
            algorithm: Default::default(),
            permissions: permission_policy(&args.permissions),
        })),
    );

    execute_and_write_workflow(workflow, stdin, args.force, stdout).await
}

pub(crate) async fn run_decrypt(
    args: SecurityDecryptArgs,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    let password = resolve_required_password("", args.password, args.password_file)?;
    let workflow = one_input_workflow(
        args.input,
        args.output,
        "decrypt",
        OperatorSpec::PdfSecurity(PdfSecurityOptions::Decrypt(SecurityDecryptOptions {
            password: Some(password),
        })),
    );

    execute_and_write_workflow(workflow, stdin, args.force, stdout).await
}

pub(crate) async fn run_permissions(
    command: PermissionsCommand,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    match command {
        PermissionsCommand::Get(args) => {
            let password = resolve_optional_password(args.password, args.password_file)?;
            let workflow = one_input_workflow(
                args.input,
                args.output,
                "permissions_get",
                OperatorSpec::PdfSecurity(PdfSecurityOptions::PermissionsGet(
                    SecurityPermissionGetOptions { password },
                )),
            );
            execute_and_write_workflow(workflow, stdin, args.force, stdout).await
        }
        PermissionsCommand::Set(args) => {
            let owner_password =
                resolve_required_password("owner", args.owner_password, args.owner_password_file)?;
            let user_password =
                resolve_required_password("user", args.user_password, args.user_password_file)?;
            let workflow = one_input_workflow(
                args.input,
                args.output,
                "permissions_set",
                OperatorSpec::PdfSecurity(PdfSecurityOptions::PermissionsSet(
                    SecurityPermissionSetOptions {
                        owner_password,
                        user_password,
                        algorithm: Default::default(),
                        permissions: permission_policy(&args.permissions),
                    },
                )),
            );
            execute_and_write_workflow(workflow, stdin, args.force, stdout).await
        }
    }
}

/// Resolves a password from exactly one of an inline value or a file. Reading
/// from a file keeps secrets out of the process argument list (`ps`, shell
/// history). Supplying both or neither is an error.
fn resolve_required_password(
    label: &str,
    inline: Option<String>,
    file: Option<PathBuf>,
) -> Result<String, CliError> {
    match (inline, file) {
        (Some(_), Some(_)) => Err(CliError::Workflow(format!(
            "{label} password: pass only one of the inline value or the file"
        ))),
        (Some(value), None) => Ok(value),
        (None, Some(path)) => read_password_file(&path),
        (None, None) => Err(CliError::Workflow(format!(
            "{label} password is required (inline or via file)"
        ))),
    }
}

/// Resolves an optional password from at most one of an inline value or a file.
fn resolve_optional_password(
    inline: Option<String>,
    file: Option<PathBuf>,
) -> Result<Option<String>, CliError> {
    match (inline, file) {
        (Some(_), Some(_)) => Err(CliError::Workflow(
            "password: pass only one of the inline value or the file".to_owned(),
        )),
        (Some(value), None) => Ok(Some(value)),
        (None, Some(path)) => read_password_file(&path).map(Some),
        (None, None) => Ok(None),
    }
}

fn read_password_file(path: &Path) -> Result<String, CliError> {
    let bytes = fs::read(path).map_err(CliError::Input)?;
    let text = String::from_utf8(bytes)
        .map_err(|_| CliError::Workflow("password file is not valid UTF-8".to_owned()))?;
    let password = text.trim_end_matches(['\n', '\r']).to_owned();
    if password.is_empty() {
        return Err(CliError::Workflow("password file is empty".to_owned()));
    }
    Ok(password)
}

pub(crate) fn permission_policy(args: &PermissionArgs) -> PermissionPolicy {
    PermissionPolicy {
        print: !args.no_print,
        modify: !args.no_modify,
        copy: !args.no_copy,
        annotate: !args.no_annotate,
        fill_forms: !args.no_fill_forms,
        accessibility: !args.no_accessibility,
        assemble: !args.no_assemble,
        high_quality_print: !args.no_high_quality_print,
    }
}
