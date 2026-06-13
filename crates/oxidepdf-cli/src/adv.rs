use super::*;

pub(crate) fn run_pdf_adv(
    command: PdfAdvCommand,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    match command {
        PdfAdvCommand::Metadata(command) => run_metadata(command, stdin, stdout),
        PdfAdvCommand::Outline(command) => run_outline(command, stdin, stdout),
        PdfAdvCommand::Attach(command) => run_attach(command, stdin, stdout),
        PdfAdvCommand::Annot(command) => run_annot(command, stdin, stdout),
        PdfAdvCommand::Form(command) => run_form(command, stdin, stdout),
        PdfAdvCommand::Image(command) => run_image(command, stdin, stdout),
    }
}

pub(crate) fn run_metadata(
    command: MetadataCommand,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    match command {
        MetadataCommand::Get(args) => execute_and_write_workflow(
            one_input_workflow(
                args.input,
                args.output,
                "metadata_get",
                OperatorSpec::PdfInspect(PdfInspectOptions::Metadata(
                    MetadataInspectOptions::default(),
                )),
            ),
            stdin,
            args.force,
            stdout,
        ),
        MetadataCommand::Set(args) => execute_and_write_workflow(
            one_input_workflow(
                args.input,
                args.output,
                "metadata_set",
                OperatorSpec::PdfEdit(PdfEditOptions::Metadata(MetadataEditOptions {
                    action: MetadataEditAction::Set,
                    entries: parse_metadata_entries(args.entries)?,
                    keys: Vec::new(),
                })),
            ),
            stdin,
            args.force,
            stdout,
        ),
        MetadataCommand::Delete(args) => execute_and_write_workflow(
            one_input_workflow(
                args.input,
                args.output,
                "metadata_delete",
                OperatorSpec::PdfEdit(PdfEditOptions::Metadata(MetadataEditOptions {
                    action: MetadataEditAction::Delete,
                    entries: Vec::new(),
                    keys: args.keys,
                })),
            ),
            stdin,
            args.force,
            stdout,
        ),
        MetadataCommand::Validate(args) => execute_and_write_workflow(
            one_input_workflow(
                args.input,
                args.output,
                "metadata_validate",
                OperatorSpec::PdfInspect(PdfInspectOptions::Metadata(
                    MetadataInspectOptions::default(),
                )),
            ),
            stdin,
            args.force,
            stdout,
        ),
    }
}

pub(crate) fn run_outline(
    command: OutlineCommand,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    match command {
        OutlineCommand::Get(args) => execute_and_write_workflow(
            one_input_workflow(
                args.input,
                args.output,
                "outline_get",
                OperatorSpec::PdfInspect(PdfInspectOptions::Outline(
                    OutlineInspectOptions::default(),
                )),
            ),
            stdin,
            args.force,
            stdout,
        ),
        OutlineCommand::Set(args) => {
            reject_shared_stdin_inputs(&args.input, &args.tree)?;
            let tree_bytes = read_path_or_stdin(&args.tree, stdin).map_err(CliError::Input)?;
            let tree: OutlineTree = serde_json::from_slice(&tree_bytes)
                .map_err(|error| CliError::Workflow(error.to_string()))?;
            execute_and_write_workflow(
                one_input_workflow(
                    args.input,
                    args.output,
                    "outline_set",
                    OperatorSpec::PdfEdit(PdfEditOptions::Outline(OutlineEditOptions {
                        action: OutlineEditAction::Set,
                        tree: Some(tree),
                    })),
                ),
                stdin,
                args.force,
                stdout,
            )
        }
        OutlineCommand::Delete(args) => execute_and_write_workflow(
            one_input_workflow(
                args.input,
                args.output,
                "outline_delete",
                OperatorSpec::PdfEdit(PdfEditOptions::Outline(OutlineEditOptions {
                    action: OutlineEditAction::Delete,
                    tree: None,
                })),
            ),
            stdin,
            args.force,
            stdout,
        ),
    }
}

pub(crate) fn run_attach(
    command: AttachCommand,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    match command {
        AttachCommand::Add(args) => {
            reject_shared_stdin_inputs(&args.input, &args.file)?;
            let name = match args.name {
                Some(name) => Some(name),
                None => Some(
                    args.file
                        .file_name()
                        .and_then(|name| name.to_str())
                        .ok_or_else(|| {
                            CliError::Workflow(
                                "attachment name must be explicit for this path".to_owned(),
                            )
                        })?
                        .to_owned(),
                ),
            };
            let workflow = two_input_workflow(
                args.input,
                args.file,
                args.output,
                "attach_add",
                OperatorSpec::PdfEdit(PdfEditOptions::Attachment(AttachmentEditOptions {
                    action: AttachmentEditAction::Add,
                    name,
                    description: args.description,
                })),
            );
            execute_and_write_workflow(workflow, stdin, args.force, stdout)
        }
        AttachCommand::List(args) => execute_and_write_workflow(
            one_input_workflow(
                args.input,
                args.output,
                "attach_list",
                OperatorSpec::PdfInspect(PdfInspectOptions::Attachments(
                    AttachmentInspectOptions::default(),
                )),
            ),
            stdin,
            args.force,
            stdout,
        ),
        AttachCommand::Extract(args) => execute_and_write_workflow(
            one_input_workflow(
                args.input,
                args.output,
                "attach_extract",
                OperatorSpec::PdfInspect(PdfInspectOptions::AttachmentExtract(
                    AttachmentExtractOptions { name: args.name },
                )),
            ),
            stdin,
            args.force,
            stdout,
        ),
        AttachCommand::Delete(args) => execute_and_write_workflow(
            one_input_workflow(
                args.input,
                args.output,
                "attach_delete",
                OperatorSpec::PdfEdit(PdfEditOptions::Attachment(AttachmentEditOptions {
                    action: AttachmentEditAction::Delete,
                    name: Some(args.name),
                    description: None,
                })),
            ),
            stdin,
            args.force,
            stdout,
        ),
    }
}

pub(crate) fn run_annot(
    command: AnnotCommand,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    match command {
        AnnotCommand::List(args) => execute_and_write_workflow(
            one_input_workflow(
                args.input,
                args.output,
                "annot_list",
                OperatorSpec::PdfInspect(PdfInspectOptions::Annotations(
                    AnnotationInspectOptions::default(),
                )),
            ),
            stdin,
            args.force,
            stdout,
        ),
        AnnotCommand::Add(args) => execute_and_write_workflow(
            one_input_workflow(
                args.input,
                args.output,
                "annot_add",
                OperatorSpec::PdfEdit(PdfEditOptions::Annotation(AnnotationEditOptions {
                    action: AnnotationEditAction::AddText,
                    page: Some(args.page),
                    id: Some(args.id),
                    text: Some(args.text),
                })),
            ),
            stdin,
            args.force,
            stdout,
        ),
        AnnotCommand::Delete(args) => execute_and_write_workflow(
            one_input_workflow(
                args.input,
                args.output,
                "annot_delete",
                OperatorSpec::PdfEdit(PdfEditOptions::Annotation(AnnotationEditOptions {
                    action: AnnotationEditAction::Delete,
                    page: None,
                    id: Some(args.id),
                    text: None,
                })),
            ),
            stdin,
            args.force,
            stdout,
        ),
    }
}

pub(crate) fn run_form(
    command: FormCommand,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    match command {
        FormCommand::Inspect(args) => execute_and_write_workflow(
            one_input_workflow(
                args.input,
                args.output,
                "form_inspect",
                OperatorSpec::PdfInspect(PdfInspectOptions::Forms(FormInspectOptions::default())),
            ),
            stdin,
            args.force,
            stdout,
        ),
        FormCommand::Fill(args) => execute_and_write_workflow(
            one_input_workflow(
                args.input,
                args.output,
                "form_fill",
                OperatorSpec::PdfEdit(PdfEditOptions::FormFill(FormFillOptions {
                    fields: parse_form_fields(args.fields)?,
                })),
            ),
            stdin,
            args.force,
            stdout,
        ),
        FormCommand::UnlockReadonly(args) => execute_and_write_workflow(
            one_input_workflow(
                args.input,
                args.output,
                "form_unlock_readonly",
                OperatorSpec::PdfEdit(PdfEditOptions::FormUnlockReadonly),
            ),
            stdin,
            args.force,
            stdout,
        ),
        FormCommand::Remove(args) => execute_and_write_workflow(
            one_input_workflow(
                args.input,
                args.output,
                "form_remove",
                OperatorSpec::PdfEdit(PdfEditOptions::FormRemove),
            ),
            stdin,
            args.force,
            stdout,
        ),
    }
}

pub(crate) fn run_interactive_remove(
    args: InteractiveRemoveArgs,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    execute_and_write_workflow(
        one_input_workflow(
            args.input,
            args.output,
            "interactive_remove",
            OperatorSpec::PdfEdit(PdfEditOptions::InteractiveRemove(
                InteractiveRemovalOptions {
                    annotations: args.annotations,
                    forms: args.forms,
                    actions: args.actions,
                    javascript: args.javascript,
                    embedded_files: args.embedded_files,
                },
            )),
        ),
        stdin,
        args.force,
        stdout,
    )
}

pub(crate) fn run_image(
    command: ImageCommand,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    match command {
        ImageCommand::List(args) => execute_and_write_workflow(
            one_input_workflow(
                args.input,
                args.output,
                "image_list",
                OperatorSpec::PdfInspect(PdfInspectOptions::Images(ImageInspectOptions::default())),
            ),
            stdin,
            args.force,
            stdout,
        ),
        ImageCommand::Add(args) => {
            reject_shared_stdin_inputs(&args.input, &args.image)?;
            execute_and_write_workflow(
                two_input_workflow(
                    args.input,
                    args.image,
                    args.output,
                    "image_add",
                    OperatorSpec::PdfEdit(PdfEditOptions::ImageEdit(ImageEditOptions {
                        action: ImageEditAction::Add,
                        name: Some(args.name),
                        page: Some(args.page),
                    })),
                ),
                stdin,
                args.force,
                stdout,
            )
        }
        ImageCommand::Replace(args) => {
            reject_shared_stdin_inputs(&args.input, &args.image)?;
            execute_and_write_workflow(
                two_input_workflow(
                    args.input,
                    args.image,
                    args.output,
                    "image_replace",
                    OperatorSpec::PdfEdit(PdfEditOptions::ImageEdit(ImageEditOptions {
                        action: ImageEditAction::Replace,
                        name: Some(args.name),
                        page: None,
                    })),
                ),
                stdin,
                args.force,
                stdout,
            )
        }
        ImageCommand::Delete(args) => execute_and_write_workflow(
            one_input_workflow(
                args.input,
                args.output,
                "image_delete",
                OperatorSpec::PdfEdit(PdfEditOptions::ImageEdit(ImageEditOptions {
                    action: ImageEditAction::Delete,
                    name: Some(args.name),
                    page: None,
                })),
            ),
            stdin,
            args.force,
            stdout,
        ),
        ImageCommand::Extract(args) => execute_and_write_workflow(
            one_input_workflow(
                args.input,
                args.output,
                "image_extract",
                OperatorSpec::PdfInspect(PdfInspectOptions::ImageExtract(ImageExtractOptions {
                    name: args.name,
                })),
            ),
            stdin,
            args.force,
            stdout,
        ),
    }
}

pub(crate) fn run_color(
    command: ColorCommand,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    match command {
        ColorCommand::Contrast(args) => run_color_edit(
            args.input,
            args.output,
            args.force,
            ColorEditOptions {
                action: ColorEditAction::Contrast,
                pages: args.pages,
                from: None,
                to: None,
                factor: Some(args.factor),
                rasterize_pages: false,
            },
            stdin,
            stdout,
        ),
        ColorCommand::Invert(args) => run_color_edit(
            args.input,
            args.output,
            args.force,
            ColorEditOptions {
                action: ColorEditAction::Invert,
                pages: args.pages,
                from: None,
                to: None,
                factor: None,
                rasterize_pages: false,
            },
            stdin,
            stdout,
        ),
        ColorCommand::Replace(args) => run_color_edit(
            args.input,
            args.output,
            args.force,
            ColorEditOptions {
                action: ColorEditAction::Replace,
                pages: args.pages,
                from: Some(parse_rgb(&args.from)?),
                to: Some(parse_rgb(&args.to)?),
                factor: None,
                rasterize_pages: false,
            },
            stdin,
            stdout,
        ),
    }
}

pub(crate) fn run_color_edit(
    input: PathBuf,
    output: PathBuf,
    force: bool,
    options: ColorEditOptions,
    stdin: &[u8],
    stdout: &mut impl Write,
) -> Result<(), CliError> {
    execute_and_write_workflow(
        one_input_workflow(
            input,
            output,
            "color",
            OperatorSpec::PdfEdit(PdfEditOptions::Color(options)),
        ),
        stdin,
        force,
        stdout,
    )
}

pub(crate) fn parse_rgb(value: &str) -> Result<[f32; 3], CliError> {
    let parts = value.split(',').collect::<Vec<_>>();
    if parts.len() != 3 {
        return Err(CliError::Workflow(
            "RGB color must use r,g,b components".to_owned(),
        ));
    }
    let mut rgb = [0.0; 3];
    for (index, part) in parts.iter().enumerate() {
        let component = part
            .parse::<f32>()
            .map_err(|_| CliError::Workflow("RGB color components must be numbers".to_owned()))?;
        if !(0.0..=1.0).contains(&component) {
            return Err(CliError::Workflow(
                "RGB color components must be between 0.0 and 1.0".to_owned(),
            ));
        }
        rgb[index] = component;
    }
    Ok(rgb)
}

pub(crate) fn parse_metadata_entries(entries: Vec<String>) -> Result<Vec<MetadataEntry>, CliError> {
    entries
        .into_iter()
        .map(|entry| {
            let (key, value) = parse_key_value(&entry, "metadata entry")?;
            Ok(MetadataEntry { key, value })
        })
        .collect()
}

pub(crate) fn parse_form_fields(fields: Vec<String>) -> Result<Vec<FormFieldValue>, CliError> {
    fields
        .into_iter()
        .map(|field| {
            let (name, value) = parse_key_value(&field, "form field")?;
            Ok(FormFieldValue { name, value })
        })
        .collect()
}
