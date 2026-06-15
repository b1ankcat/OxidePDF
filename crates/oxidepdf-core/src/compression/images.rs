use super::pipeline::{stream_filter_names, unsupported_stream_filter_error};
use super::resources::stream_subtype;
use super::types::{CompressionImageFormat, CompressionImageOptions};
use crate::{OxideError, ResourceLimits, enforce_max_pixels};
use lopdf::{Object, Stream};
use std::io::Cursor;

pub(super) fn recompress_images_lossy(
    document: &mut lopdf::Document,
    options: &CompressionImageOptions,
    limits: &ResourceLimits,
) -> Result<(), OxideError> {
    if options
        .format
        .is_some_and(|format| format != CompressionImageFormat::Jpeg)
    {
        return Err(OxideError::UnsupportedPdfFeature {
            feature: "lossy image target formats other than jpeg".to_owned(),
        });
    }

    for object in document.objects.values_mut() {
        let Object::Stream(stream) = object else {
            continue;
        };
        if stream_subtype(stream) != Some(b"Image") {
            continue;
        }
        recompress_image_stream_to_jpeg(stream, options, limits)?;
    }

    Ok(())
}

fn recompress_image_stream_to_jpeg(
    stream: &mut Stream,
    options: &CompressionImageOptions,
    limits: &ResourceLimits,
) -> Result<(), OxideError> {
    ensure_supported_image_dictionary(stream)?;
    let width = required_u32(stream, b"Width")?;
    let height = required_u32(stream, b"Height")?;
    // Bound the decoded pixel count before allocating/decoding the payload, so a
    // small stream declaring huge dimensions cannot force a multi-GB allocation.
    enforce_max_pixels(u64::from(width) * u64::from(height), limits)?;
    let mut image = image::RgbImage::from_raw(width, height, image_rgb_bytes(stream)?).ok_or(
        OxideError::UnsupportedPdfFeature {
            feature: "image stream dimensions do not match RGB payload length".to_owned(),
        },
    )?;

    let (target_width, target_height) = target_image_size(width, height, options)?;
    if target_width != width || target_height != height {
        image = image::imageops::resize(
            &image,
            target_width,
            target_height,
            image::imageops::FilterType::Lanczos3,
        );
    }

    let mut encoded = Vec::new();
    let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(
        Cursor::new(&mut encoded),
        options.quality.unwrap_or(85),
    );
    encoder
        .encode(
            image.as_raw(),
            target_width,
            target_height,
            image::ExtendedColorType::Rgb8,
        )
        .map_err(|_| OxideError::WritePdf)?;

    stream.dict.set("Width", target_width);
    stream.dict.set("Height", target_height);
    stream.dict.set("ColorSpace", "DeviceRGB");
    stream.dict.set("BitsPerComponent", 8);
    stream.dict.remove(b"DecodeParms");
    stream.dict.set("Filter", "DCTDecode");
    stream.set_content(encoded);
    Ok(())
}

fn ensure_supported_image_dictionary(stream: &Stream) -> Result<(), OxideError> {
    if stream.dict.has(b"ImageMask") {
        return Err(OxideError::UnsupportedPdfFeature {
            feature: "image masks are not supported for lossy compression".to_owned(),
        });
    }
    if stream.dict.has(b"SMask") || stream.dict.has(b"Mask") {
        return Err(OxideError::UnsupportedPdfFeature {
            feature: "masked images are not supported for lossy compression".to_owned(),
        });
    }
    if stream
        .dict
        .get(b"ColorSpace")
        .and_then(Object::as_name)
        .map_err(|_| OxideError::UnsupportedPdfFeature {
            feature: "image color space is required for lossy compression".to_owned(),
        })?
        != b"DeviceRGB"
    {
        return Err(OxideError::UnsupportedPdfFeature {
            feature: "image color spaces other than DeviceRGB".to_owned(),
        });
    }
    if stream
        .dict
        .get(b"BitsPerComponent")
        .and_then(Object::as_i64)
        .map_err(|_| OxideError::UnsupportedPdfFeature {
            feature: "image bits per component is required for lossy compression".to_owned(),
        })?
        != 8
    {
        return Err(OxideError::UnsupportedPdfFeature {
            feature: "image bit depths other than 8 bits per component".to_owned(),
        });
    }
    Ok(())
}

fn image_rgb_bytes(stream: &Stream) -> Result<Vec<u8>, OxideError> {
    match stream_filter_names(stream)? {
        None => Ok(stream.content.clone()),
        Some(filters) if filters.len() == 1 && filters[0] == b"FlateDecode" => stream
            .get_plain_content()
            .map_err(|_| unsupported_stream_filter_error(stream)),
        Some(filters) if filters.len() == 1 && filters[0] == b"DCTDecode" => {
            let image =
                image::load_from_memory_with_format(&stream.content, image::ImageFormat::Jpeg)
                    .map_err(|_| OxideError::ImageDecode)?;
            Ok(image.into_rgb8().into_raw())
        }
        Some(filters) => Err(OxideError::UnsupportedPdfFeature {
            feature: format!(
                "image stream filter '{}'",
                filters
                    .iter()
                    .map(|filter| String::from_utf8_lossy(filter).into_owned())
                    .collect::<Vec<_>>()
                    .join(",")
            ),
        }),
    }
}

fn required_u32(stream: &Stream, key: &[u8]) -> Result<u32, OxideError> {
    let value = stream
        .dict
        .get(key)
        .and_then(Object::as_i64)
        .map_err(|_| OxideError::ParsePdf)?;
    u32::try_from(value).map_err(|_| OxideError::ParsePdf)
}

fn target_image_size(
    width: u32,
    height: u32,
    options: &CompressionImageOptions,
) -> Result<(u32, u32), OxideError> {
    let max_width = options.max_width.unwrap_or(width);
    let max_height = options.max_height.unwrap_or(height);
    if max_width == 0 || max_height == 0 {
        return Err(OxideError::InvalidInput {
            reason: "image maximum dimensions must be greater than zero".to_owned(),
        });
    }
    let scale = (max_width as f64 / width as f64)
        .min(max_height as f64 / height as f64)
        .min(1.0);
    let target_width = ((width as f64 * scale).round() as u32).max(1);
    let target_height = ((height as f64 * scale).round() as u32).max(1);
    Ok((target_width, target_height))
}
