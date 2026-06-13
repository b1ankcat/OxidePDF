use super::*;

pub(crate) fn run_pdf_edit(
    command: PdfEditCommand,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    match command {
        PdfEditCommand::Merge(args) => run_merge(args, stdin, stdout),
        PdfEditCommand::KeepPages(args) => {
            run_page_selection(args, stdin, stdout, PageCommand::Keep)
        }
        PdfEditCommand::ExtractPages(args) => {
            run_page_selection(args, stdin, stdout, PageCommand::Extract)
        }
        PdfEditCommand::ReorderPages(args) => {
            run_page_selection(args, stdin, stdout, PageCommand::Reorder)
        }
        PdfEditCommand::RotatePages(args) => run_rotate(args, stdin, stdout),
        PdfEditCommand::DeletePages(args) => run_delete_pages(args, stdin, stdout),
        PdfEditCommand::DeleteBlankPages(args) => run_delete_blank_pages(args, stdin, stdout),
        PdfEditCommand::CropPages(args) => run_crop_pages(args, stdin, stdout),
        PdfEditCommand::ScalePages(args) => run_scale_pages(args, stdin, stdout),
        PdfEditCommand::SinglePage(args) => run_single_page(args, stdin, stdout),
        PdfEditCommand::NUp(args) => run_nup(args, stdin, stdout),
        PdfEditCommand::Booklet(args) => run_booklet(args, stdin, stdout),
        PdfEditCommand::PageNumbers(args) => run_page_numbers(args, stdin, stdout),
        PdfEditCommand::Img2pdf(args) => run_img2pdf(args, stdin, stdout),
        PdfEditCommand::Svg2pdf(args) => run_svg2pdf(args, stdin, stdout),
        PdfEditCommand::Watermark(args) => run_watermark(args, stdin, stdout),
        PdfEditCommand::Compress(args) => run_compress(args, stdin, stdout),
        PdfEditCommand::Stamp(args) => run_stamp(args, stdin, stdout),
        PdfEditCommand::OverlayPdf(args) => run_overlay_pdf(args, stdin, stdout),
        PdfEditCommand::Color(command) => run_color(command, stdin, stdout),
        PdfEditCommand::InteractiveRemove(args) => run_interactive_remove(args, stdin, stdout),
    }
}

pub(crate) fn run_merge(
    args: MergeArgs,
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
            id: TaskId::new("merge"),
            op: OperatorSpec::PdfEdit(PdfEditOptions::Merge(MergeOptions {})),
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

pub(crate) fn run_page_selection(
    args: PageSelectionArgs,
    stdin: &[u8],
    stdout: &mut impl Write,
    command: PageCommand,
) -> Result<(), CliError> {
    let task_id = match command {
        PageCommand::Keep => "keep_pages",
        PageCommand::Extract => "extract_pages",
        PageCommand::Reorder => "reorder_pages",
    };
    let op = match command {
        PageCommand::Keep => OperatorSpec::PdfEdit(PdfEditOptions::KeepPages(SplitOptions {
            pages: args.pages,
        })),
        PageCommand::Extract => {
            OperatorSpec::PdfEdit(PdfEditOptions::ExtractPages(PageSelectionOptions {
                pages: args.pages,
            }))
        }
        PageCommand::Reorder => {
            OperatorSpec::PdfEdit(PdfEditOptions::ReorderPages(ReorderOptions {
                pages: args.pages,
            }))
        }
    };
    let workflow = one_input_workflow(args.input, args.output, task_id, op);

    execute_and_write_workflow(workflow, stdin, args.force, stdout)
}

pub(crate) fn run_rotate(
    args: RotateArgs,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    let workflow = one_input_workflow(
        args.input,
        args.output,
        "rotate_pages",
        OperatorSpec::PdfEdit(PdfEditOptions::RotatePages(RotateOptions {
            pages: args.pages,
            degrees: args.degrees,
        })),
    );

    execute_and_write_workflow(workflow, stdin, args.force, stdout)
}

pub(crate) fn run_delete_pages(
    args: PageSelectionArgs,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    let workflow = one_input_workflow(
        args.input,
        args.output,
        "delete_pages",
        OperatorSpec::PdfEdit(PdfEditOptions::DeletePages(PageSelectionOptions {
            pages: args.pages,
        })),
    );

    execute_and_write_workflow(workflow, stdin, args.force, stdout)
}

pub(crate) fn run_delete_blank_pages(
    args: DeleteBlankPagesArgs,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    let workflow = one_input_workflow(
        args.input,
        args.output,
        "delete_blank_pages",
        OperatorSpec::PdfEdit(PdfEditOptions::DeleteBlankPages(
            DeleteBlankPagesOptions::default(),
        )),
    );

    execute_and_write_workflow(workflow, stdin, args.force, stdout)
}

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

pub(crate) fn run_crop_pages(
    args: CropPagesArgs,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    let workflow = one_input_workflow(
        args.input,
        args.output,
        "crop_pages",
        OperatorSpec::PdfEdit(PdfEditOptions::CropPages(CropPagesOptions {
            pages: args.pages,
            left: args.left,
            bottom: args.bottom,
            right: args.right,
            top: args.top,
        })),
    );

    execute_and_write_workflow(workflow, stdin, args.force, stdout)
}

pub(crate) fn run_scale_pages(
    args: ScalePagesArgs,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    let workflow = one_input_workflow(
        args.input,
        args.output,
        "scale_pages",
        OperatorSpec::PdfEdit(PdfEditOptions::ScalePages(ScalePagesOptions {
            pages: args.pages,
            factor: args.factor,
        })),
    );

    execute_and_write_workflow(workflow, stdin, args.force, stdout)
}

pub(crate) fn run_single_page(
    args: SinglePageArgs,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    let workflow = one_input_workflow(
        args.input,
        args.output,
        "single_page",
        OperatorSpec::PdfEdit(PdfEditOptions::SinglePage(SinglePageOptions::default())),
    );

    execute_and_write_workflow(workflow, stdin, args.force, stdout)
}

pub(crate) fn run_nup(
    args: NUpArgs,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    let workflow = one_input_workflow(
        args.input,
        args.output,
        "nup",
        OperatorSpec::PdfEdit(PdfEditOptions::NUp(NUpOptions {
            columns: args.columns,
            rows: args.rows,
        })),
    );

    execute_and_write_workflow(workflow, stdin, args.force, stdout)
}

pub(crate) fn run_booklet(
    args: BookletArgs,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    let workflow = one_input_workflow(
        args.input,
        args.output,
        "booklet",
        OperatorSpec::PdfEdit(PdfEditOptions::Booklet(BookletOptions::default())),
    );

    execute_and_write_workflow(workflow, stdin, args.force, stdout)
}

pub(crate) fn run_page_numbers(
    args: PageNumbersArgs,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    let workflow = one_input_workflow(
        args.input,
        args.output,
        "page_numbers",
        OperatorSpec::PdfEdit(PdfEditOptions::PageNumbers(PageNumbersOptions {
            pages: args.pages,
            start: args.start,
            prefix: args.prefix,
            suffix: args.suffix,
            font_size: args.font_size,
            position: args.position.into(),
        })),
    );

    execute_and_write_workflow(workflow, stdin, args.force, stdout)
}

pub(crate) fn run_img2pdf(
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
            op: OperatorSpec::PdfEdit(PdfEditOptions::ImageToPdf(ImageToPdfOptions {
                layout: args.layout,
            })),
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
