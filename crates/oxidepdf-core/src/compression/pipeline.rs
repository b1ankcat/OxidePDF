use super::images::recompress_images_lossy;
use super::resources::{merge_duplicate_resource_streams, stream_subtype};
use super::types::{CompressionMode, CompressionOptions};
use crate::{
    OxideError, PdfArtifact, ResourceLimits, enforce_input_bytes, enforce_max_pages,
    enforce_output_bytes, load_pdf, save_pdf,
};
use lopdf::{Object, Stream};

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
        bytes: crate::ArtifactBytes::from_vec(bytes)?,
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

pub(super) fn stream_filter_names(stream: &Stream) -> Result<Option<Vec<Vec<u8>>>, OxideError> {
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

pub(super) fn unsupported_stream_filter_error(stream: &Stream) -> OxideError {
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
