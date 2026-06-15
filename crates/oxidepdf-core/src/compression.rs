mod images;
mod pipeline;
mod resources;
mod types;

pub(crate) use pipeline::compress_on_document;
pub use pipeline::compress_pdf;
pub use types::{
    CompressionImageFormat, CompressionImageOptions, CompressionMode, CompressionOptions,
};
