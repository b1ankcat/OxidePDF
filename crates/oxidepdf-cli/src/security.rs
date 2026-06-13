use super::*;

pub(crate) fn run_pdf_security(
    command: PdfSecurityCommand,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    match command {
        PdfSecurityCommand::Encrypt(args) => run_encrypt(args, stdin, stdout),
        PdfSecurityCommand::Decrypt(args) => run_decrypt(args, stdin, stdout),
        PdfSecurityCommand::Permissions(command) => run_permissions(command, stdin, stdout),
    }
}

pub(crate) fn run_encrypt(
    args: SecurityEncryptArgs,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    let workflow = one_input_workflow(
        args.input,
        args.output,
        "encrypt",
        OperatorSpec::PdfSecurity(PdfSecurityOptions::Encrypt(SecurityEncryptOptions {
            owner_password: args.owner_password,
            user_password: args.user_password,
            algorithm: Default::default(),
            permissions: permission_policy(&args.permissions),
        })),
    );

    execute_and_write_workflow(workflow, stdin, args.force, stdout)
}

pub(crate) fn run_decrypt(
    args: SecurityDecryptArgs,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    let workflow = one_input_workflow(
        args.input,
        args.output,
        "decrypt",
        OperatorSpec::PdfSecurity(PdfSecurityOptions::Decrypt(SecurityDecryptOptions {
            password: Some(args.password),
        })),
    );

    execute_and_write_workflow(workflow, stdin, args.force, stdout)
}

pub(crate) fn run_permissions(
    command: PermissionsCommand,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    match command {
        PermissionsCommand::Get(args) => {
            let workflow = one_input_workflow(
                args.input,
                args.output,
                "permissions_get",
                OperatorSpec::PdfSecurity(PdfSecurityOptions::PermissionsGet(
                    SecurityPermissionGetOptions {
                        password: args.password,
                    },
                )),
            );
            execute_and_write_workflow(workflow, stdin, args.force, stdout)
        }
        PermissionsCommand::Set(args) => {
            let workflow = one_input_workflow(
                args.input,
                args.output,
                "permissions_set",
                OperatorSpec::PdfSecurity(PdfSecurityOptions::PermissionsSet(
                    SecurityPermissionSetOptions {
                        owner_password: args.owner_password,
                        user_password: args.user_password,
                        algorithm: Default::default(),
                        permissions: permission_policy(&args.permissions),
                    },
                )),
            );
            execute_and_write_workflow(workflow, stdin, args.force, stdout)
        }
    }
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
