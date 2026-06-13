use crate::{
    enforce_input_bytes, enforce_max_pages, enforce_max_pixels, enforce_output_bytes, load_pdf,
    save_pdf, OxideError, PdfArtifact, ResourceLimits,
};
use lopdf::{Dictionary, Object, Stream};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Cursor;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CompressionOptions {
    pub mode: CompressionMode,
    pub images: Option<CompressionImageOptions>,
}

impl Default for CompressionOptions {
    fn default() -> Self {
        Self {
            mode: CompressionMode::Lossless,
            images: None,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompressionMode {
    #[default]
    Lossless,
    Lossy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompressionImageOptions {
    pub quality: Option<u8>,
    pub max_width: Option<u32>,
    pub max_height: Option<u32>,
    pub format: Option<CompressionImageFormat>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompressionImageFormat {
    Jpeg,
    Png,
    Webp,
}

pub fn compress_pdf(
    input: &[u8],
    options: &CompressionOptions,
    limits: &ResourceLimits,
) -> Result<PdfArtifact, OxideError> {
    enforce_input_bytes(input.len(), limits)?;
    let mut document = load_pdf(input)?;
    compress_on_document(&mut document, options, limits)?;
    let bytes = save_pdf(document)?;
    enforce_output_bytes(bytes.len(), limits)?;
    Ok(PdfArtifact {
        bytes: bytes.into(),
    })
}

/// Compresses an already-parsed document in place.
pub(crate) fn compress_on_document(
    document: &mut lopdf::Document,
    options: &CompressionOptions,
    limits: &ResourceLimits,
) -> Result<(), OxideError> {
    validate_compression_options(options)?;
    enforce_max_pages(document.get_pages().len(), limits)?;

    reject_unsupported_recompressed_stream_filters(document)?;
    merge_duplicate_resource_streams(document)?;
    if let CompressionMode::Lossy = options.mode {
        recompress_images_lossy(
            document,
            options.images.as_ref().ok_or(OxideError::Internal)?,
            limits,
        )?;
    }
    recompress_streams(document)
}

fn validate_compression_options(options: &CompressionOptions) -> Result<(), OxideError> {
    match options.mode {
        CompressionMode::Lossless => {
            if options.images.is_some() {
                return Err(OxideError::InvalidInput {
                    reason: "lossless compression does not accept image resampling or reencoding options"
                        .to_owned(),
                });
            }
        }
        CompressionMode::Lossy => {
            let images = options
                .images
                .as_ref()
                .ok_or_else(|| OxideError::InvalidInput {
                    reason: "lossy compression requires explicit image options".to_owned(),
                })?;
            let has_lossy_parameter = images.quality.is_some()
                || images.max_width.is_some()
                || images.max_height.is_some()
                || images.format.is_some();
            if !has_lossy_parameter {
                return Err(OxideError::InvalidInput {
                    reason: "lossy compression requires quality, max dimensions, or target format"
                        .to_owned(),
                });
            }
            if let Some(quality) = images.quality {
                if !(1..=100).contains(&quality) {
                    return Err(OxideError::InvalidInput {
                        reason: "image quality must be between 1 and 100".to_owned(),
                    });
                }
            }
        }
    }

    Ok(())
}

fn reject_unsupported_recompressed_stream_filters(
    document: &lopdf::Document,
) -> Result<(), OxideError> {
    for object in document.objects.values() {
        let Object::Stream(stream) = object else {
            continue;
        };
        if stream_subtype(stream) == Some(b"Image") {
            continue;
        }
        let Some(filters) = stream_filter_names(stream)? else {
            continue;
        };
        for filter in filters {
            match filter.as_slice() {
                b"FlateDecode" | b"LZWDecode" | b"ASCII85Decode" => {}
                _ => {
                    return Err(OxideError::UnsupportedPdfFeature {
                        feature: format!("stream filter '{}'", String::from_utf8_lossy(&filter)),
                    });
                }
            }
        }
    }
    Ok(())
}

fn stream_filter_names(stream: &Stream) -> Result<Option<Vec<Vec<u8>>>, OxideError> {
    let Ok(filter) = stream.dict.get(b"Filter") else {
        return Ok(None);
    };

    if let Ok(name) = filter.as_name() {
        return Ok(Some(vec![name.to_vec()]));
    }
    if let Ok(names) = filter.as_array() {
        let names = names
            .iter()
            .map(|name| name.as_name().map(|name| name.to_vec()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| OxideError::ParsePdf)?;
        return Ok(Some(names));
    }

    Err(OxideError::ParsePdf)
}

fn merge_duplicate_resource_streams(document: &mut lopdf::Document) -> Result<(), OxideError> {
    let mut duplicate_map = BTreeMap::new();
    let mut seen_streams = BTreeMap::new();
    for (id, object) in &document.objects {
        let Object::Stream(stream) = object else {
            continue;
        };
        let Some(subtype) = stream_subtype(stream) else {
            continue;
        };
        if subtype != b"Image" && subtype != b"Form" {
            continue;
        }
        let key = ResourceStreamKey::new(subtype.to_vec(), stream);
        if let Some(canonical_id) = seen_streams.get(&key) {
            duplicate_map.insert(*id, *canonical_id);
        } else {
            seen_streams.insert(key, *id);
        }
    }

    if duplicate_map.is_empty() {
        return Ok(());
    }

    for object in document.objects.values_mut() {
        remap_duplicate_references(object, &duplicate_map);
    }
    for duplicate_id in duplicate_map.keys() {
        document.objects.remove(duplicate_id);
    }

    Ok(())
}

fn stream_subtype(stream: &Stream) -> Option<&[u8]> {
    stream.dict.get(b"Subtype").and_then(Object::as_name).ok()
}

fn remap_duplicate_references(
    object: &mut Object,
    duplicate_map: &BTreeMap<lopdf::ObjectId, lopdf::ObjectId>,
) {
    match object {
        Object::Reference(id) => {
            if let Some(canonical_id) = duplicate_map.get(id) {
                *id = *canonical_id;
            }
        }
        Object::Array(items) => {
            for item in items {
                remap_duplicate_references(item, duplicate_map);
            }
        }
        Object::Dictionary(dictionary) => {
            for (_, value) in dictionary.iter_mut() {
                remap_duplicate_references(value, duplicate_map);
            }
        }
        Object::Stream(stream) => {
            for (_, value) in stream.dict.iter_mut() {
                remap_duplicate_references(value, duplicate_map);
            }
        }
        _ => {}
    }
}

fn recompress_images_lossy(
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

fn recompress_streams(document: &mut lopdf::Document) -> Result<(), OxideError> {
    for object in document.objects.values_mut() {
        let Object::Stream(stream) = object else {
            continue;
        };
        if !stream.allows_compression {
            continue;
        }
        if stream_subtype(stream) == Some(b"Image") {
            continue;
        }
        let plain = stream
            .get_plain_content()
            .map_err(|_| unsupported_stream_filter_error(stream))?;
        stream.set_plain_content(plain);
        stream.compress().map_err(|_| OxideError::WritePdf)?;
    }
    Ok(())
}

fn unsupported_stream_filter_error(stream: &Stream) -> OxideError {
    match stream_filter_names(stream) {
        Ok(Some(filters)) => OxideError::UnsupportedPdfFeature {
            feature: format!(
                "stream filter '{}'",
                filters
                    .iter()
                    .map(|filter| String::from_utf8_lossy(filter).into_owned())
                    .collect::<Vec<_>>()
                    .join(",")
            ),
        },
        _ => OxideError::ParsePdf,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct ResourceStreamKey {
    subtype: Vec<u8>,
    dictionary_entries: Vec<(Vec<u8>, ComparableObject)>,
    content: Vec<u8>,
}

impl ResourceStreamKey {
    fn new(subtype: Vec<u8>, stream: &Stream) -> Self {
        let mut dictionary_entries = comparable_dictionary(&stream.dict);
        dictionary_entries.retain(|(key, _)| key.as_slice() != b"Length");
        Self {
            subtype,
            dictionary_entries,
            content: stream.content.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum ComparableObject {
    Null,
    Boolean(bool),
    Integer(i64),
    Real(u32),
    Name(Vec<u8>),
    String(Vec<u8>),
    Array(Vec<ComparableObject>),
    Dictionary(Vec<(Vec<u8>, ComparableObject)>),
    Reference(lopdf::ObjectId),
}

fn comparable_dictionary(dictionary: &Dictionary) -> Vec<(Vec<u8>, ComparableObject)> {
    dictionary
        .iter()
        .map(|(key, value)| (key.clone(), comparable_object(value)))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn comparable_object(object: &Object) -> ComparableObject {
    match object {
        Object::Null => ComparableObject::Null,
        Object::Boolean(value) => ComparableObject::Boolean(*value),
        Object::Integer(value) => ComparableObject::Integer(*value),
        Object::Real(value) => ComparableObject::Real(value.to_bits()),
        Object::Name(value) => ComparableObject::Name(value.clone()),
        Object::String(value, _) => ComparableObject::String(value.clone()),
        Object::Array(items) => {
            ComparableObject::Array(items.iter().map(comparable_object).collect())
        }
        Object::Dictionary(dictionary) => {
            ComparableObject::Dictionary(comparable_dictionary(dictionary))
        }
        Object::Stream(stream) => ComparableObject::Dictionary(comparable_dictionary(&stream.dict)),
        Object::Reference(id) => ComparableObject::Reference(*id),
    }
}
