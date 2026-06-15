use crate::{
    Artifact, OxideError, PdfArtifact, ResourceLimits, enforce_input_bytes, enforce_max_pages,
    enforce_output_bytes, load_pdf, merge_resource_dictionary, object_to_f32, page_size, pdf_bytes,
    rebuild_pages_tree, resource_limit, save_pdf,
};
use lopdf::{Dictionary, Object, Stream};

mod imposition;
mod options;
mod page_numbers;
mod selection;
pub use imposition::{
    booklet_pdf_pages, booklet_pdf_pages_with_limits, nup_pdf_pages, nup_pdf_pages_with_limits,
};
pub use options::{
    BookletOptions, CropPagesOptions, DeleteBlankPagesOptions, MergeOptions, NUpOptions,
    PageNumberPosition, PageNumbersOptions, PageSelectionOptions, ReorderOptions, RotateOptions,
    ScalePagesOptions, SinglePageOptions, SplitOptions,
};
pub(crate) use page_numbers::add_page_numbers_on_document;
pub use page_numbers::{add_pdf_page_numbers, add_pdf_page_numbers_with_limits};
pub(crate) use selection::parse_page_range;
use selection::selected_or_all_pages;
use std::collections::BTreeMap;

include!("merge.rs");
include!("selection_edit.rs");
include!("geometry_edit.rs");
include!("helpers.rs");
