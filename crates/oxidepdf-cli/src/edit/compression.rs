pub(crate) fn run_compress(
    args: CompressArgs,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    let images = compression_image_options(&args);
    let workflow = one_input_workflow(
        args.input,
        args.output,
        "compress",
        OperatorSpec::PdfEdit(PdfEditOptions::Compression(CompressionOptions {
            mode: args.mode.into(),
            images,
        })),
    );

    execute_and_write_workflow(workflow, stdin, args.force, stdout)
}

pub(crate) fn compression_image_options(args: &CompressArgs) -> Option<CompressionImageOptions> {
    if args.image_quality.is_none()
        && args.image_max_width.is_none()
        && args.image_max_height.is_none()
        && args.image_format.is_none()
    {
        return None;
    }

    Some(CompressionImageOptions {
        quality: args.image_quality,
        max_width: args.image_max_width,
        max_height: args.image_max_height,
        format: args.image_format.map(Into::into),
    })
}

