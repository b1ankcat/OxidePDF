struct DecodedImage {
    width: u32,
    height: u32,
    rgb: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ImageLayout {
    Fit,
    OriginalSize,
}

impl ImageLayout {
    fn from_options(options: &ImageToPdfOptions) -> Result<Self, OxideError> {
        match options.layout.as_deref().unwrap_or("fit") {
            "fit" => Ok(Self::Fit),
            "original_size" => Ok(Self::OriginalSize),
            other => Err(OxideError::InvalidInput {
                reason: format!("unsupported image layout '{other}'"),
            }),
        }
    }
}

fn decode_image(input: &[u8]) -> Result<DecodedImage, OxideError> {
    let format = image::guess_format(input).map_err(|_| OxideError::ImageDecode)?;
    match format {
        image::ImageFormat::Jpeg | image::ImageFormat::Png | image::ImageFormat::WebP => {}
        _ => return Err(OxideError::ImageDecode),
    }
    let image = image::load_from_memory_with_format(input, format)
        .map_err(|_| OxideError::ImageDecode)?
        .to_rgb8();

    Ok(DecodedImage {
        width: image.width(),
        height: image.height(),
        rgb: image.into_raw(),
    })
}

fn decode_limited_image(input: &[u8], limits: &ResourceLimits) -> Result<DecodedImage, OxideError> {
    enforce_input_bytes(input.len(), limits)?;
    let decoded = decode_image(input)?;
    let pixels = u64::from(decoded.width) * u64::from(decoded.height);
    enforce_max_pixels(pixels, limits)?;
    Ok(decoded)
}

fn write_images_pdf(images: &[DecodedImage], layout: ImageLayout) -> Result<Vec<u8>, OxideError> {
    let mut next_ref = 1;
    let mut alloc_ref = || {
        let reference = pdf_writer::Ref::new(next_ref);
        next_ref += 1;
        reference
    };
    let catalog_id = alloc_ref();
    let pages_id = alloc_ref();
    let page_ids = (0..images.len()).map(|_| alloc_ref()).collect::<Vec<_>>();
    let image_ids = (0..images.len()).map(|_| alloc_ref()).collect::<Vec<_>>();
    let content_ids = (0..images.len()).map(|_| alloc_ref()).collect::<Vec<_>>();

    let mut pdf = pdf_writer::Pdf::new();
    pdf.catalog(catalog_id).pages(pages_id);
    pdf.pages(pages_id)
        .kids(page_ids.iter().copied())
        .count(images.len() as i32);

    for (((page_id, image_id), content_id), image) in page_ids
        .iter()
        .zip(image_ids.iter())
        .zip(content_ids.iter())
        .zip(images.iter())
    {
        let image_name = pdf_writer::Name(b"Im1");
        let (page_width, page_height, image_width, image_height, x, y) =
            image_placement(image, layout);

        let mut page = pdf.page(*page_id);
        page.media_box(pdf_writer::Rect::new(0.0, 0.0, page_width, page_height));
        page.parent(pages_id);
        page.contents(*content_id);
        page.resources().x_objects().pair(image_name, *image_id);
        page.finish();

        let mut image_object = pdf.image_xobject(*image_id, &image.rgb);
        image_object.width(image.width as i32);
        image_object.height(image.height as i32);
        image_object.color_space().device_rgb();
        image_object.bits_per_component(8);
        image_object.finish();

        let mut content = pdf_writer::Content::new();
        content.save_state();
        content.transform([image_width, 0.0, 0.0, image_height, x, y]);
        content.x_object(image_name);
        content.restore_state();
        pdf.stream(*content_id, &content.finish());
    }

    Ok(pdf.finish())
}

fn image_placement(image: &DecodedImage, layout: ImageLayout) -> (f32, f32, f32, f32, f32, f32) {
    let original_width = image.width as f32;
    let original_height = image.height as f32;
    match layout {
        ImageLayout::OriginalSize => (
            original_width,
            original_height,
            original_width,
            original_height,
            0.0,
            0.0,
        ),
        ImageLayout::Fit => {
            let scale = (A4_WIDTH / original_width)
                .min(A4_HEIGHT / original_height)
                .min(1.0);
            let image_width = original_width * scale;
            let image_height = original_height * scale;
            let x = (A4_WIDTH - image_width) / 2.0;
            let y = (A4_HEIGHT - image_height) / 2.0;
            (A4_WIDTH, A4_HEIGHT, image_width, image_height, x, y)
        }
    }
}

fn parse_svg(input: &[u8]) -> Result<svg2pdf::usvg::Tree, OxideError> {
    ensure_svg_magic(input)?;
    let options = svg2pdf::usvg::Options::default();
    svg2pdf::usvg::Tree::from_data(input, &options).map_err(|_| OxideError::SvgParse)
}

fn ensure_svg_magic(input: &[u8]) -> Result<(), OxideError> {
    let input = input
        .strip_prefix(&[0xEF, 0xBB, 0xBF])
        .unwrap_or(input)
        .trim_ascii_start();
    if input.starts_with(b"<svg") || input.starts_with(b"<?xml") {
        return Ok(());
    }

    Err(OxideError::SvgParse)
}

fn svg_pixel_count(tree: &svg2pdf::usvg::Tree) -> Result<u64, OxideError> {
    let size = tree.size().to_int_size();
    Ok(u64::from(size.width()) * u64::from(size.height()))
}

fn rasterize_svg(tree: &svg2pdf::usvg::Tree) -> Result<DecodedImage, OxideError> {
    let size = tree.size().to_int_size();
    let mut pixmap =
        resvg::tiny_skia::Pixmap::new(size.width(), size.height()).ok_or(OxideError::RenderPdf)?;
    resvg::render(
        tree,
        resvg::tiny_skia::Transform::identity(),
        &mut pixmap.as_mut(),
    );
    let mut rgb =
        Vec::with_capacity((u64::from(size.width()) * u64::from(size.height()) * 3) as usize);
    for pixel in pixmap.data().chunks_exact(4) {
        rgb.extend_from_slice(&[pixel[0], pixel[1], pixel[2]]);
    }

    Ok(DecodedImage {
        width: size.width(),
        height: size.height(),
        rgb,
    })
}
