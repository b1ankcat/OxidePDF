use serde::{Deserialize, Serialize};

/// Options for merge.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MergeOptions {}

/// Options for split.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SplitOptions {
    /// Page range, for example `1,3-5`.
    pub pages: String,
}

/// Options for reorder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReorderOptions {
    /// Explicit page sequence, for example `3,1,2`.
    pub pages: String,
}

/// Options for rotate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RotateOptions {
    /// Page range, for example `1,3-5`.
    pub pages: String,
    /// Rotation in degrees. Validation happens in the workflow validator.
    pub degrees: i16,
}

/// Options for page-selection edits.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageSelectionOptions {
    /// Page range, for example `1,3-5`.
    pub pages: String,
}

/// Options for deleting structurally blank pages.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DeleteBlankPagesOptions {}

/// Options for cropping pages.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CropPagesOptions {
    /// Page range, for example `1,3-5`.
    pub pages: Option<String>,
    /// Left coordinate of the new CropBox.
    pub left: f32,
    /// Bottom coordinate of the new CropBox.
    pub bottom: f32,
    /// Right coordinate of the new CropBox.
    pub right: f32,
    /// Top coordinate of the new CropBox.
    pub top: f32,
}

/// Options for scaling pages.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScalePagesOptions {
    /// Page range, for example `1,3-5`.
    pub pages: Option<String>,
    /// Scale factor applied to page boxes and page contents.
    pub factor: f32,
}

/// Options for combining pages into one tall page.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SinglePageOptions {}

/// Options for N-up page layout.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NUpOptions {
    /// Number of columns on each output page.
    pub columns: u32,
    /// Number of rows on each output page.
    pub rows: u32,
}

/// Options for booklet imposition.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct BookletOptions {}

/// Options for adding page numbers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PageNumbersOptions {
    /// Page range, for example `1,3-5`. Defaults to all pages.
    pub pages: Option<String>,
    /// First number written on the first selected page.
    pub start: u32,
    /// Text before the number.
    pub prefix: String,
    /// Text after the number.
    pub suffix: String,
    /// Font size in PDF points.
    pub font_size: f32,
    /// Page number placement.
    pub position: PageNumberPosition,
}

impl Default for PageNumbersOptions {
    fn default() -> Self {
        Self {
            pages: None,
            start: 1,
            prefix: String::new(),
            suffix: String::new(),
            font_size: 12.0,
            position: PageNumberPosition::BottomCenter,
        }
    }
}

/// Page number placement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PageNumberPosition {
    /// Top-left corner.
    TopLeft,
    /// Top-center edge.
    TopCenter,
    /// Top-right corner.
    TopRight,
    /// Bottom-left corner.
    BottomLeft,
    /// Bottom-center edge.
    BottomCenter,
    /// Bottom-right corner.
    BottomRight,
}
