mod report;
mod types;
mod visual;

pub use report::compare_pdf_report;
pub use types::{
    CompareDifference, CompareDifferenceCode, CompareOptions, CompareReport,
    ObjectStructureSummary, PageSizeSummary, PdfCompareOptions, PdfCompareSummary, TextSummary,
    VisualDiffOptions,
};
pub use visual::compare_pdf_visual_diff;
