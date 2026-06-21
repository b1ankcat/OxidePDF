//! Integration tests for the `sign` CLI surface.
//!
//! Split out of the former monolithic `src/lib.rs` test module. These drive the
//! CLI through its public `run_with_io`/`command` entry points only.

mod common;
include!("sign/verify_signatures_command_writes_json_report.rs");
include!("sign/workflow_signature_operator_writes_json_report.rs");
