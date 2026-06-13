//! Integration tests for the `cli` CLI surface.
//!
//! Split out of the former monolithic `src/lib.rs` test module. These drive the
//! CLI through its public `run_with_io`/`command` entry points only.

mod common;
#[allow(unused_imports)]
use common::*;
#[allow(unused_imports)]
use oxidepdf_cli::{command, run, run_with_io};
use std::fs;

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
    assert!(help.contains("edit"));
    assert!(help.contains("inspect"));
    assert!(help.contains("sign"));
}

#[test]
fn command_tree_has_useful_help_for_commands_and_arguments() {
    fn assert_help(command: &clap::Command, path: String) {
        if command.is_hide_set() {
            return;
        }

        if !path.is_empty() {
            assert!(
                command.get_about().is_some() || command.get_long_about().is_some(),
                "{path} should describe what it does"
            );
        }

        for argument in command.get_arguments() {
            if argument.is_hide_set() {
                continue;
            }
            assert!(
                argument.get_help().is_some() || argument.get_long_help().is_some(),
                "{} {} should describe how to use it",
                path,
                argument.get_id()
            );
        }

        for subcommand in command.get_subcommands() {
            let child_path = if path.is_empty() {
                subcommand.get_name().to_owned()
            } else {
                format!("{path} {}", subcommand.get_name())
            };
            assert_help(subcommand, child_path);
        }
    }

    let command = command();
    assert_help(&command, String::new());
}

#[test]
fn bash_completion_can_be_written_to_stdout() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let code = run_with_io(
        ["oxidepdf", "completion", "bash"],
        [],
        &mut stdout,
        &mut stderr,
    );
    let completion = String::from_utf8(stdout).unwrap();

    assert_eq!(code, 0);
    assert_eq!(stderr, b"");
    assert!(completion.contains("_oxidepdf()"));
    assert!(completion.contains("complete -F _oxidepdf"));
    assert!(completion.contains("oxidepdf__subcmd__pdf_sign"));
}

#[test]
fn bash_completion_can_be_written_to_file() {
    let dir = temp_dir("bash_completion_can_be_written_to_file");
    let output = dir.join("oxidepdf.bash");
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let code = run_with_io(
        [
            "oxidepdf",
            "completion",
            "bash",
            "-o",
            output.to_str().unwrap(),
        ],
        [],
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(code, 0);
    assert_eq!(stdout, b"");
    assert_eq!(stderr, b"");
    let completion = fs::read_to_string(output).unwrap();
    assert!(completion.contains("_oxidepdf()"));
}

#[test]
fn bash_completion_rejects_conflicting_destinations() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let code = run_with_io(
        [
            "oxidepdf",
            "completion",
            "bash",
            "--stdout",
            "-o",
            "oxidepdf.bash",
        ],
        [],
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(code, 2);
    assert_eq!(stdout, b"");
    assert!(String::from_utf8(stderr)
        .unwrap()
        .contains("cannot be used"));
}

#[test]
fn version_uses_package_version() {
    let command = command();
    let version = command.get_version().unwrap();

    assert_eq!(version, env!("CARGO_PKG_VERSION"));
}

#[test]
fn help_returns_success_exit_code() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let code = run_with_io(["oxidepdf", "--help"], [], &mut stdout, &mut stderr);

    assert_eq!(code, 0);
    assert_eq!(stdout, b"");
    assert!(String::from_utf8(stderr)
        .unwrap()
        .contains("Usage: oxidepdf"));
}

#[test]
fn invalid_arguments_return_usage_exit_code() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let code = run_with_io(["oxidepdf", "--missing"], [], &mut stdout, &mut stderr);

    assert_eq!(code, 2);
    assert_eq!(stdout, b"");
    assert!(String::from_utf8(stderr)
        .unwrap()
        .contains("unexpected argument"));
}

#[test]
fn removed_legacy_top_level_commands_are_not_aliases() {
    for legacy_command in [
        "merge",
        "split",
        "reorder",
        "rotate",
        "img2pdf",
        "svg2pdf",
        "render",
        "extract-text",
        "watermark",
        "verify-signatures",
        "pdf-compare",
        "pdf-sign",
        "compress",
        "pdf-edit",
        "pdf-inspect",
        "signature-appearance",
        "edit",
        "inspect",
        "compare",
        "sign",
        "timestamp",
        "metadata",
        "outline",
        "attach",
        "annot",
        "form",
        "interactive",
        "stamp",
        "overlay-pdf",
        "image",
        "color",
        "encrypt",
        "decrypt",
        "permissions",
    ] {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let code = run_with_io(
            ["oxidepdf", legacy_command, "--help"],
            [],
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 2, "{legacy_command} should not remain as an alias");
        assert_eq!(stdout, b"");
        assert!(
            String::from_utf8(stderr)
                .unwrap()
                .contains("unrecognized subcommand"),
            "{legacy_command} should fail at the CLI boundary"
        );
    }
}

#[test]
fn run_workflow_file_writes_input_artifact_to_output() {
    let dir = temp_dir("run_workflow_file_writes_input_artifact_to_output");
    let workflow = dir.join("workflow.yaml");
    let output = dir.join("out.bin");
    fs::write(
        &workflow,
        format!(
            r#"
            version: 1
            inputs:
              - id: source
                path: {}
            tasks: []
            outputs:
              - id: final
                from: source
                path: {}
            "#,
            yaml_path(dir.join("input.bin")),
            yaml_path(&output)
        ),
    )
    .unwrap();
    fs::write(dir.join("input.bin"), b"input bytes").unwrap();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let code = run_with_io(
        ["oxidepdf", "run", "--workflow", workflow.to_str().unwrap()],
        [],
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(code, 0);
    assert_eq!(stdout, b"");
    assert_eq!(stderr, b"");
    assert_eq!(fs::read(output).unwrap(), b"input bytes");
}

#[test]
fn run_workflow_input_stdin_and_output_stdout_keep_diagnostics_on_stderr() {
    let dir = temp_dir("run_workflow_input_stdin_and_output_stdout");
    let workflow_path = dir.join("workflow.yaml");
    let workflow = br#"
        version: 1
        inputs:
          - id: source
            path: "-"
        tasks: []
        outputs:
          - id: final
            from: source
            path: "-"
    "#;
    fs::write(&workflow_path, workflow).unwrap();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let code = run_with_io(
        [
            "oxidepdf",
            "run",
            "--workflow",
            workflow_path.to_str().unwrap(),
        ],
        b"stdin bytes",
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(code, 0);
    assert_eq!(stdout, b"stdin bytes");
    assert_eq!(stderr, b"");
}

#[test]
fn run_workflow_document_can_be_read_from_stdin() {
    let workflow = br#"
        version: 1
        inputs: []
        tasks: []
        outputs: []
    "#;
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let code = run_with_io(
        ["oxidepdf", "run", "--workflow", "-"],
        workflow,
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(code, 0);
    assert_eq!(stdout, b"");
    assert_eq!(stderr, b"");
}

#[test]
fn pdf_parse_error_returns_input_exit_code_without_output() {
    let dir = temp_dir("pdf_parse_error_returns_input_exit_code");
    let workflow = dir.join("workflow.yaml");
    let output = dir.join("out.bin");
    fs::write(dir.join("input.bin"), b"input bytes").unwrap();
    fs::write(
        &workflow,
        format!(
            r#"
            version: 1
            inputs:
              - id: source
                path: {}
            tasks:
              - id: rotate
                op:
                  pdf_edit:
                    rotate_pages:
                      pages: "1"
                      degrees: 90
                inputs: [source]
            outputs:
              - id: final
                from: rotate
                path: {}
            "#,
            yaml_path(dir.join("input.bin")),
            yaml_path(&output)
        ),
    )
    .unwrap();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let code = run_with_io(
        ["oxidepdf", "run", "--workflow", workflow.to_str().unwrap()],
        [],
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(code, 3);
    assert_eq!(stdout, b"");
    assert!(!output.exists());
    let stderr = String::from_utf8(stderr).unwrap();
    assert!(stderr.contains("invalid_input"));
    assert!(stderr.contains("expected PDF"));
}

#[test]
fn workflow_enforces_total_input_size_limit() {
    let dir = temp_dir("workflow_enforces_total_input_size_limit");
    let workflow = dir.join("workflow.yaml");
    let output = dir.join("out.bin");
    fs::write(dir.join("input_a.bin"), b"12345").unwrap();
    fs::write(dir.join("input_b.bin"), b"67890").unwrap();
    fs::write(
        &workflow,
        format!(
            r#"
            version: 1
            inputs:
              - id: first
                path: {}
              - id: second
                path: {}
            tasks: []
            outputs:
              - id: final
                from: first
                path: {}
            limits:
              max_total_input_bytes: 9
            "#,
            yaml_path(dir.join("input_a.bin")),
            yaml_path(dir.join("input_b.bin")),
            yaml_path(&output)
        ),
    )
    .unwrap();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let code = run_with_io(
        ["oxidepdf", "run", "--workflow", workflow.to_str().unwrap()],
        [],
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(code, 5);
    assert_eq!(stdout, b"");
    assert!(!output.exists());
    assert!(String::from_utf8(stderr)
        .unwrap()
        .contains("max_total_input_bytes"));
}

#[test]
fn workflow_enforces_output_size_limit() {
    let dir = temp_dir("workflow_enforces_output_size_limit");
    let workflow = dir.join("workflow.yaml");
    let output = dir.join("out.bin");
    fs::write(dir.join("input.bin"), b"larger than limit").unwrap();
    fs::write(
        &workflow,
        format!(
            r#"
            version: 1
            inputs:
              - id: source
                path: {}
            tasks: []
            outputs:
              - id: final
                from: source
                path: {}
            limits:
              max_output_bytes: 1
            "#,
            yaml_path(dir.join("input.bin")),
            yaml_path(&output)
        ),
    )
    .unwrap();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let code = run_with_io(
        ["oxidepdf", "run", "--workflow", workflow.to_str().unwrap()],
        [],
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(code, 5);
    assert_eq!(stdout, b"");
    assert!(!output.exists());
    assert!(String::from_utf8(stderr)
        .unwrap()
        .contains("max_output_bytes"));
}

#[test]
fn error_output_redacts_sensitive_material_and_paths() {
    let dir = temp_dir("error_output_redacts_sensitive_material_and_paths");
    let secret_dir = dir.join("secret-client-certificates");
    fs::create_dir_all(&secret_dir).unwrap();
    let missing = secret_dir.join("client-password-token.pem");
    let workflow = dir.join("workflow.yaml");
    fs::write(
        &workflow,
        format!(
            r#"
            version: 1
            inputs:
              - id: source
                path: {}
            tasks: []
            outputs:
              - id: final
                from: source
                path: {}
            "#,
            yaml_path(&missing),
            yaml_path(dir.join("out.bin"))
        ),
    )
    .unwrap();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let code = run_with_io(
        ["oxidepdf", "run", "--workflow", workflow.to_str().unwrap()],
        [],
        &mut stdout,
        &mut stderr,
    );
    let stderr = String::from_utf8(stderr).unwrap();

    assert_eq!(code, 3);
    assert_eq!(stdout, b"");
    assert!(!stderr.contains(dir.to_str().unwrap()));
    assert!(!stderr.to_ascii_lowercase().contains("password"));
    assert!(!stderr.to_ascii_lowercase().contains("token"));
    assert!(!stderr.to_ascii_lowercase().contains("certificate"));
    assert!(!stderr.contains(".pem"));
    assert!(!stderr.contains("stack backtrace"));
}

#[test]
fn invalid_workflow_returns_usage_exit_code() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let code = run_with_io(
        ["oxidepdf", "run", "--workflow", "-"],
        b"version: 1\ninputs: []\ntasks: []\n",
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(code, 2);
    assert_eq!(stdout, b"");
    assert!(String::from_utf8(stderr).unwrap().contains("workflow"));
}

#[test]
fn missing_input_file_returns_input_exit_code() {
    let dir = temp_dir("missing_input_file_returns_input_exit_code");
    let workflow = dir.join("workflow.yaml");
    fs::write(
        &workflow,
        format!(
            r#"
            version: 1
            inputs:
              - id: source
                path: {}
            tasks: []
            outputs:
              - id: final
                from: source
                path: {}
            "#,
            yaml_path(dir.join("missing.bin")),
            yaml_path(dir.join("out.bin"))
        ),
    )
    .unwrap();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let code = run_with_io(
        ["oxidepdf", "run", "--workflow", workflow.to_str().unwrap()],
        [],
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(code, 3);
    assert_eq!(stdout, b"");
    assert!(String::from_utf8(stderr).unwrap().contains("input"));
}

#[test]
fn output_file_is_not_overwritten_without_force() {
    let dir = temp_dir("output_file_is_not_overwritten_without_force");
    let workflow = dir.join("workflow.yaml");
    let output = dir.join("out.bin");
    fs::write(dir.join("input.bin"), b"input bytes").unwrap();
    fs::write(&output, b"existing").unwrap();
    fs::write(
        &workflow,
        format!(
            r#"
            version: 1
            inputs:
              - id: source
                path: {}
            tasks: []
            outputs:
              - id: final
                from: source
                path: {}
            "#,
            yaml_path(dir.join("input.bin")),
            yaml_path(&output)
        ),
    )
    .unwrap();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let code = run_with_io(
        ["oxidepdf", "run", "--workflow", workflow.to_str().unwrap()],
        [],
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(code, 2);
    assert_eq!(fs::read(output).unwrap(), b"existing");
    assert!(String::from_utf8(stderr)
        .unwrap()
        .contains("already exists"));
}
