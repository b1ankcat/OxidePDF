#![forbid(unsafe_code)]

use clap::{CommandFactory, Parser};

/// OxidePDF command-line arguments.
#[derive(Debug, Parser)]
#[command(
    name = "oxidepdf",
    version,
    about = "Pure Rust PDF toolkit",
    long_about = "OxidePDF is a pure Rust PDF toolkit. Stage 1 exposes the CLI shell; PDF operators are added in later stages."
)]
pub struct Cli {}

/// Parses CLI arguments and runs the requested command.
pub fn run() -> i32 {
    let _cli = Cli::parse();
    0
}

/// Returns the clap command definition for tests and generated help.
pub fn command() -> clap::Command {
    Cli::command()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clap_definition_is_valid() {
        command().debug_assert();
    }

    #[test]
    fn help_mentions_project_name() {
        let mut help = Vec::new();
        command().write_long_help(&mut help).unwrap();
        let help = String::from_utf8(help).unwrap();

        assert!(help.contains("OxidePDF"));
        assert!(help.contains("pure Rust PDF toolkit"));
    }

    #[test]
    fn version_uses_package_version() {
        let command = command();
        let version = command.get_version().unwrap();

        assert_eq!(version, env!("CARGO_PKG_VERSION"));
    }
}
