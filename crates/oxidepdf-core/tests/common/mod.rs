//! Shared fixtures and helpers for the `oxidepdf-core` integration tests.
//!
//! Everything here is built strictly on the crate's public API, so it can be
//! reused from the per-module integration test files under `tests/`.

#![allow(dead_code)]

include!("mod/object_to_f32.rs");
include!("mod/page_optional_box.rs");
include!("mod/three_page_text_pdf.rs");
include!("mod/pdf_with_text_form_field.rs");
include!("mod/pdf_with_rgb_fill_content.rs");
