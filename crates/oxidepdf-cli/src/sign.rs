use super::*;

pub(crate) fn run_sign(
    command: PdfSignCommand,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    match command {
        PdfSignCommand::Add(args) => run_add_signature(args, stdin, stdout),
        PdfSignCommand::List(args) => run_list_signatures(args, stdin, stdout),
        PdfSignCommand::Verify(args) => run_verify_signatures(args, stdin, stdout),
        PdfSignCommand::DeleteField(args) => run_delete_signature_field(args, stdin, stdout),
        PdfSignCommand::Appearance(args) => run_signature_appearance(args, stdin, stdout),
        PdfSignCommand::Timestamp(args) => run_add_timestamp(args, stdin, stdout),
    }
}

pub(crate) fn run_verify_signatures(
    args: VerifySignaturesArgs,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    let workflow = one_input_workflow(
        args.input,
        args.output,
        "verify_signature",
        OperatorSpec::PdfSign(PdfSignOptions::Verify(SignatureOptions {
            mode: Default::default(),
            trust_anchors: args.trust_anchors,
        })),
    );

    execute_and_write_workflow(workflow, stdin, args.force, stdout)
}

pub(crate) fn run_add_signature(
    args: SignAddArgs,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    let workflow = one_input_workflow(
        args.input,
        args.output,
        "add_signature",
        OperatorSpec::PdfSign(PdfSignOptions::Add(SignatureAddOptions {
            field_name: args.field_name,
            certificate: args.certificate,
            private_key: args.private_key,
            contents_reserved_bytes: args.contents_reserved_bytes,
            appearance_field: args.appearance_field,
        })),
    );

    execute_and_write_workflow(workflow, stdin, args.force, stdout)
}

pub(crate) fn run_list_signatures(
    args: ListSignaturesArgs,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    let workflow = one_input_workflow(
        args.input,
        args.output,
        "list_signatures",
        OperatorSpec::PdfSign(PdfSignOptions::List(SignatureOptions {
            mode: oxidepdf_core::SignatureMode::List,
            trust_anchors: None,
        })),
    );

    execute_and_write_workflow(workflow, stdin, args.force, stdout)
}

pub(crate) fn run_delete_signature_field(
    args: SignDeleteFieldArgs,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    let workflow = one_input_workflow(
        args.input,
        args.output,
        "delete_signature_field",
        OperatorSpec::PdfSign(PdfSignOptions::DeleteField(SignatureDeleteFieldOptions {
            field_name: args.field_name,
            destructive: args.destructive,
        })),
    );

    execute_and_write_workflow(workflow, stdin, args.force, stdout)
}

pub(crate) fn run_add_timestamp(
    args: TimestampAddArgs,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    let workflow = one_input_workflow(
        args.input,
        args.output,
        "add_timestamp",
        OperatorSpec::PdfSign(PdfSignOptions::Timestamp(TimestampAddOptions {
            tsa_url: args.tsa_url,
            token: args.token,
        })),
    );

    execute_and_write_workflow(workflow, stdin, args.force, stdout)
}

pub(crate) fn run_stamp(
    args: StampArgs,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    execute_and_write_workflow(
        one_input_workflow(
            args.input,
            args.output,
            "stamp",
            OperatorSpec::PdfEdit(PdfEditOptions::Overlay(OverlayOptions {
                kind: OverlayKind::Stamp,
                text: Some(args.text),
                font: Some(args.font.unwrap_or_else(|| "Helvetica".to_owned())),
                font_path: args.font_path,
                font_size: args.font_size,
                opacity: args.opacity,
                rotation: args.rotation,
                position: args.position,
                pages: args.pages,
                scale: None,
                rasterize: false,
                source_page: None,
            })),
        ),
        stdin,
        args.force,
        stdout,
    )
}

pub(crate) fn run_signature_appearance(
    args: SignatureAppearanceArgs,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    execute_and_write_workflow(
        one_input_workflow(
            args.input,
            args.output,
            "signature_appearance",
            OperatorSpec::PdfEdit(PdfEditOptions::Overlay(OverlayOptions {
                kind: OverlayKind::SignatureAppearance,
                text: Some(args.text),
                font: Some(args.font.unwrap_or_else(|| "Helvetica".to_owned())),
                font_path: args.font_path,
                font_size: args.font_size,
                opacity: Some(1.0),
                rotation: None,
                position: args.position,
                pages: args.pages,
                scale: None,
                rasterize: false,
                source_page: None,
            })),
        ),
        stdin,
        args.force,
        stdout,
    )
}

pub(crate) fn run_overlay_pdf(
    args: OverlayPdfArgs,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    reject_shared_stdin_inputs(&args.input, &args.overlay)?;
    execute_and_write_workflow(
        two_input_workflow(
            args.input,
            args.overlay,
            args.output,
            "overlay_pdf",
            OperatorSpec::PdfEdit(PdfEditOptions::Overlay(OverlayOptions {
                kind: OverlayKind::PdfPage,
                text: None,
                font: None,
                font_path: None,
                font_size: None,
                opacity: args.opacity,
                rotation: None,
                position: args.position,
                pages: args.pages,
                scale: args.scale,
                rasterize: false,
                source_page: args.source_page,
            })),
        ),
        stdin,
        args.force,
        stdout,
    )
}
