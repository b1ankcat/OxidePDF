//! Shared fixtures and assertions for the `oxidepdf-cli` integration tests.
//!
//! Built on the crate's public API plus the same external crates the original
//! in-crate tests used. Split out of the former `src/lib.rs` test module.

#![allow(dead_code)]

include!("mod/temp_dir.rs");
include!("mod/generated_fixture_pdf.rs");
include!("mod/pdf_with_blank_and_marked_page.rs");
