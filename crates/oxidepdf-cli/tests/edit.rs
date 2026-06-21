//! Integration tests for the `edit` CLI surface.
//!
//! Split out of the former monolithic `src/lib.rs` test module. These drive the
//! CLI through its public `run_with_io`/`command` entry points only.

mod common;
include!("edit/merge_command_writes_combined_pdf.rs");
include!("edit/page_numbers_command_writes_selected_page_labels.rs");
