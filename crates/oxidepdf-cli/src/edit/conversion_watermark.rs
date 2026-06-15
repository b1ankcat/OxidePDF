pub(crate) fn run_img2pdf(
    args: ImageToPdfArgs,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    let workflow = multi_input_workflow(
        args.inputs,
        args.output,
        "img2pdf",
        OperatorSpec::PdfEdit(PdfEditOptions::ImageToPdf(ImageToPdfOptions {
            layout: args.layout,
        })),
    );

    execute_and_write_workflow(workflow, stdin, args.force, stdout)
}

pub(crate) fn run_svg2pdf(
    args: SvgToPdfArgs,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    let workflow = one_input_workflow(
        args.input,
        args.output,
        "svg2pdf",
        OperatorSpec::PdfEdit(PdfEditOptions::SvgToPdf(SvgToPdfOptions {
            rasterize: args.rasterize,
        })),
    );

    execute_and_write_workflow(workflow, stdin, args.force, stdout)
}

pub(crate) fn run_watermark(
    args: WatermarkArgs,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    let kind = parse_watermark_kind(&args.kind)?;
    let mut input_specs = vec![oxidepdf_core::InputSpec {
        id: ArtifactRef::new("input"),
        path: args.input,
    }];
    let mut task_inputs = vec![ArtifactRef::new("input")];
    if matches!(kind, WatermarkKind::Image | WatermarkKind::Svg) {
        let watermark = args.watermark.ok_or_else(|| {
            CliError::Workflow("image and SVG watermarks require --watermark".to_owned())
        })?;
        input_specs.push(oxidepdf_core::InputSpec {
            id: ArtifactRef::new("watermark_input"),
            path: watermark,
        });
        task_inputs.push(ArtifactRef::new("watermark_input"));
    }

    let workflow = Workflow {
        version: WorkflowVersion::V1,
        inputs: input_specs,
        tasks: vec![TaskSpec {
            id: TaskId::new("watermark"),
            op: OperatorSpec::PdfEdit(PdfEditOptions::Watermark(WatermarkOptions {
                kind,
                text: args.text,
                font: args.font,
                font_path: args.font_path,
                font_size: args.font_size,
                opacity: args.opacity,
                rotation: args.rotation,
                position: args.position,
                pages: args.pages,
                scale: args.scale,
                rasterize: args.rasterize,
            })),
            inputs: task_inputs,
        }],
        outputs: vec![oxidepdf_core::OutputSpec {
            id: ArtifactRef::new("output"),
            from: ArtifactRef::new("watermark"),
            path: args.output,
        }],
        limits: Default::default(),
        metadata: WorkflowMetadata::default(),
    };

    execute_and_write_workflow(workflow, stdin, args.force, stdout)
}

pub(crate) fn parse_watermark_kind(value: &str) -> Result<WatermarkKind, CliError> {
    match value {
        "text" => Ok(WatermarkKind::Text),
        "image" => Ok(WatermarkKind::Image),
        "svg" => Ok(WatermarkKind::Svg),
        other => Err(CliError::Workflow(format!(
            "unsupported watermark kind '{other}'"
        ))),
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum PageCommand {
    Keep,
    Extract,
    Reorder,
}
