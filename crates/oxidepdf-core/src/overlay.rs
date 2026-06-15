use crate::page_ops::parse_page_range;
use crate::{
    Artifact, BytesArtifact, ImageArtifact, OxideError, PdfArtifact, ResourceLimits, TextArtifact,
    TextExtractionDiagnostic, TextExtractionDiagnosticCode, add_resource_dict_entry,
    enforce_input_bytes, enforce_max_pages, enforce_max_pixels, enforce_output_bytes,
    ensure_pdf_magic, load_pdf, map_pdf_extract_error, merge_resource_dictionary, object_to_f32,
    page_size, pdf_bytes, remap_imported_references, resource_limit, save_pdf,
};
use lopdf::{Dictionary, Object, Stream, dictionary};
use pdf_writer::Finish;
use read_fonts::TableProvider;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

const A4_WIDTH: f32 = 595.0;
const A4_HEIGHT: f32 = 842.0;

include!("overlay/options.rs");
include!("overlay/entry.rs");
include!("overlay/inputs.rs");
include!("overlay/images_svg.rs");
include!("overlay/watermark.rs");
include!("overlay/resources_color.rs");
