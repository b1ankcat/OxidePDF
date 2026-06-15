use super::report::enforce_compare_inputs;
use super::types::VisualDiffOptions;
use crate::{
    ImageArtifact, OxideError, RenderOptions, ResourceLimits, enforce_max_pixels,
    enforce_output_bytes, render_pdf_page,
};
use image::{ImageBuffer, ImageFormat, Rgba, RgbaImage};
use std::io::Cursor;

pub fn compare_pdf_visual_diff(
    left: &[u8],
    right: &[u8],
    options: &VisualDiffOptions,
    limits: &ResourceLimits,
) -> Result<ImageArtifact, OxideError> {
    enforce_compare_inputs(left, right, limits)?;
    if options.page == 0 {
        return Err(OxideError::InvalidInput {
            reason: "page number must be one or greater".to_owned(),
        });
    }

    let render_options = RenderOptions {
        page: options.page,
        format: Some("png".to_owned()),
        scale: options.scale,
    };
    let left_render = render_pdf_page(left, &render_options, limits)?;
    let right_render = render_pdf_page(right, &render_options, limits)?;
    let left_image = image::load_from_memory(&left_render.bytes)
        .map_err(|_| OxideError::RenderPdf)?
        .to_rgba8();
    let right_image = image::load_from_memory(&right_render.bytes)
        .map_err(|_| OxideError::RenderPdf)?
        .to_rgba8();

    let width = left_image.width().max(right_image.width());
    let height = left_image.height().max(right_image.height());
    let pixels = u64::from(width) * u64::from(height);
    enforce_max_pixels(pixels, limits)?;

    let mut diff: RgbaImage = ImageBuffer::from_pixel(width, height, Rgba([255, 255, 255, 255]));
    for y in 0..height {
        for x in 0..width {
            let left_pixel = pixel_at(&left_image, x, y);
            let right_pixel = pixel_at(&right_image, x, y);
            if left_pixel != right_pixel {
                diff.put_pixel(x, y, Rgba([220, 38, 38, 255]));
            }
        }
    }

    let mut bytes = Vec::new();
    diff.write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
        .map_err(|_| OxideError::RenderPdf)?;
    if bytes.is_empty() {
        return Err(OxideError::RenderPdf);
    }
    enforce_output_bytes(bytes.len(), limits)?;
    Ok(ImageArtifact {
        bytes: crate::ArtifactBytes::from_vec(bytes)?,
    })
}

fn pixel_at(image: &RgbaImage, x: u32, y: u32) -> Option<Rgba<u8>> {
    if x < image.width() && y < image.height() {
        Some(*image.get_pixel(x, y))
    } else {
        None
    }
}
