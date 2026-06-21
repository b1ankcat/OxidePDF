//! Integration tests for the `adv` CLI surface.
//!
//! Split out of the former monolithic `src/lib.rs` test module. These drive the
//! CLI through its public `run_with_io`/`command` entry points only.

mod common;
include!("adv/metadata_commands_set_and_get_json_report.rs");
include!("adv/form_commands_fill_inspect_unlock_and_remove.rs");
