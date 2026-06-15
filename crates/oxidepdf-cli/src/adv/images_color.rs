pub(crate) async fn run_image(
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
        )
        .await,
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
            .await
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
            .await
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
        )
        .await,
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
        )
        .await,
    }
}

pub(crate) async fn run_color(
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
        ).await,
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
        ).await,
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
        ).await,
    }
}

pub(crate) async fn run_color_edit(
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
    ).await
}

pub(crate) fn parse_rgb(value: &str) -> Result<[f32; 3], CliError> {
    let value = value.trim();
    if let Some(hex) = value.strip_prefix('#') {
        return parse_hex_rgb(hex);
    }
    let parts = value.split(',').collect::<Vec<_>>();
    if parts.len() != 3 {
        return Err(CliError::Workflow(
            "RGB color must be #RRGGBB or three comma-separated components".to_owned(),
        ));
    }
    let mut rgb = [0.0; 3];
    for (index, part) in parts.iter().enumerate() {
        let component = part
            .trim()
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

fn parse_hex_rgb(hex: &str) -> Result<[f32; 3], CliError> {
    if hex.len() != 6 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(CliError::Workflow(
            "hex RGB color must be #RRGGBB".to_owned(),
        ));
    }
    let mut rgb = [0.0; 3];
    for (index, slot) in rgb.iter_mut().enumerate() {
        let start = index * 2;
        let component = u8::from_str_radix(&hex[start..start + 2], 16)
            .map_err(|_| CliError::Workflow("hex RGB color must be #RRGGBB".to_owned()))?;
        *slot = f32::from(component) / 255.0;
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

#[cfg(test)]
mod tests {
    use super::parse_rgb;

    #[test]
    fn parse_rgb_accepts_hex() {
        assert_eq!(parse_rgb("#000000").unwrap(), [0.0, 0.0, 0.0]);
        assert_eq!(parse_rgb("#ffffff").unwrap(), [1.0, 1.0, 1.0]);
        let red = parse_rgb("#FF0000").unwrap();
        assert_eq!(red, [1.0, 0.0, 0.0]);
    }

    #[test]
    fn parse_rgb_accepts_comma_components() {
        assert_eq!(parse_rgb("0,0.5,1").unwrap(), [0.0, 0.5, 1.0]);
        assert_eq!(parse_rgb(" 1 , 0 , 0 ").unwrap(), [1.0, 0.0, 0.0]);
    }

    #[test]
    fn parse_rgb_rejects_malformed_values() {
        assert!(parse_rgb("#fff").is_err());
        assert!(parse_rgb("#gggggg").is_err());
        assert!(parse_rgb("1,2,3").is_err());
        assert!(parse_rgb("0,0").is_err());
    }
}
