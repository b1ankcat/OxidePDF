use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;

#[test]
fn run_workflow_file_succeeds() {
    let dir = temp_dir("run_workflow_file_succeeds");
    let input = dir.join("input.bin");
    let output = dir.join("output.bin");
    let workflow = dir.join("workflow.yaml");
    fs::write(&input, b"input").unwrap();
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
            input.display(),
            output.display()
        ),
    )
    .unwrap();

    Command::cargo_bin("oxidepdf")
        .unwrap()
        .args(["run", "--workflow", workflow.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::eq(""))
        .stderr(predicate::eq(""));

    assert_eq!(fs::read(output).unwrap(), b"input");
}

#[test]
fn invalid_workflow_exits_with_code_2() {
    let dir = temp_dir("invalid_workflow_exits_with_code_2");
    let workflow = dir.join("workflow.yaml");
    fs::write(&workflow, "version: 1\ninputs: []\ntasks: []\n").unwrap();

    Command::cargo_bin("oxidepdf")
        .unwrap()
        .args(["run", "--workflow", workflow.to_str().unwrap()])
        .assert()
        .code(2)
        .stdout(predicate::eq(""))
        .stderr(predicate::str::contains("invalid workflow"));
}

#[test]
fn unsupported_operator_exits_with_code_3() {
    let dir = temp_dir("unsupported_operator_exits_with_code_3");
    let input = dir.join("input.bin");
    let output = dir.join("output.bin");
    let workflow = dir.join("workflow.yaml");
    fs::write(&input, b"input").unwrap();
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
                  rotate:
                    pages: "1"
                    degrees: 90
                inputs: [source]
            outputs:
              - id: final
                from: rotate
                path: {}
            "#,
            input.display(),
            output.display()
        ),
    )
    .unwrap();

    Command::cargo_bin("oxidepdf")
        .unwrap()
        .args(["run", "--workflow", workflow.to_str().unwrap()])
        .assert()
        .code(3)
        .stdout(predicate::eq(""))
        .stderr(predicate::str::contains("unsupported_pdf_feature"));

    assert!(!output.exists());
}

fn temp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "oxidepdf_cli_integration_{}_{}",
        name,
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}
