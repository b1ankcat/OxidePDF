//! Integration tests for the `cli` CLI surface.
//!
//! Split out of the former monolithic `src/lib.rs` test module. These drive the
//! CLI through its public `run_with_io`/`command` entry points only.

mod common;
include!("cli/clap_definition_is_valid.rs");
include!("cli/pdf_parse_error_returns_input_exit_code_without_output.rs");
