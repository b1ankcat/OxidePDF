use assert_cmd::Command;
use common::{fixture_jpg, fixture_pdf};
use lopdf::dictionary;
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
fn run_workflow_non_force_does_not_overwrite_existing_output() {
    let dir = temp_dir("run_workflow_non_force_does_not_overwrite_existing_output");
    let input = dir.join("input.bin");
    let output = dir.join("output.bin");
    let workflow = dir.join("workflow.yaml");
    fs::write(&input, b"input").unwrap();
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
            input.display(),
            output.display()
        ),
    )
    .unwrap();

    Command::cargo_bin("oxidepdf")
        .unwrap()
        .args(["run", "--workflow", workflow.to_str().unwrap()])
        .assert()
        .code(2)
        .stdout(predicate::eq(""))
        .stderr(predicate::str::contains("output file already exists"));

    assert_eq!(fs::read(output).unwrap(), b"existing");
}

#[test]
fn run_workflow_writes_metrics_json_after_success() {
    let dir = temp_dir("run_workflow_writes_metrics_json_after_success");
    let input = dir.join("input.bin");
    let output = dir.join("output.bin");
    let metrics = dir.join("metrics.json");
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
        .args([
            "run",
            "--workflow",
            workflow.to_str().unwrap(),
            "--metrics-output",
            metrics.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::eq(""))
        .stderr(predicate::eq(""));

    assert_eq!(fs::read(output).unwrap(), b"input");
    let metrics: serde_json::Value = serde_json::from_slice(&fs::read(metrics).unwrap()).unwrap();
    assert_eq!(metrics["version"], 1);
    assert!(metrics["elapsed_ms"].as_u64().is_some());
    assert_eq!(metrics["input_bytes"], 5);
    assert_eq!(metrics["output_bytes"], 5);
    assert_eq!(metrics["task_count"], 0);
    assert!(metrics["peak_rss_bytes"].as_u64().unwrap() > 0);
    assert_eq!(metrics["peak_rss_source"], "/proc/self/status:VmHWM");
}

#[test]
fn run_workflow_rejects_metrics_stdout() {
    let dir = temp_dir("run_workflow_rejects_metrics_stdout");
    let workflow = dir.join("workflow.yaml");
    fs::write(
        &workflow,
        "version: 1\ninputs: []\ntasks: []\noutputs: []\n",
    )
    .unwrap();

    Command::cargo_bin("oxidepdf")
        .unwrap()
        .args([
            "run",
            "--workflow",
            workflow.to_str().unwrap(),
            "--metrics-output",
            "-",
        ])
        .assert()
        .code(2)
        .stdout(predicate::eq(""))
        .stderr(predicate::str::contains("metrics output cannot be '-'"));
}

#[test]
fn run_workflow_metrics_output_uses_force_for_overwrite() {
    let dir = temp_dir("run_workflow_metrics_output_uses_force_for_overwrite");
    let input = dir.join("input.bin");
    let output = dir.join("output.bin");
    let metrics = dir.join("metrics.json");
    let workflow = dir.join("workflow.yaml");
    fs::write(&input, b"input").unwrap();
    fs::write(&metrics, b"existing").unwrap();
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
        .args([
            "run",
            "--workflow",
            workflow.to_str().unwrap(),
            "--metrics-output",
            metrics.to_str().unwrap(),
        ])
        .assert()
        .code(2)
        .stdout(predicate::eq(""))
        .stderr(predicate::str::contains(
            "metrics output file already exists",
        ));
    assert!(!output.exists());

    Command::cargo_bin("oxidepdf")
        .unwrap()
        .args([
            "run",
            "--workflow",
            workflow.to_str().unwrap(),
            "--metrics-output",
            metrics.to_str().unwrap(),
            "--force",
        ])
        .assert()
        .success();

    let metrics: serde_json::Value = serde_json::from_slice(&fs::read(metrics).unwrap()).unwrap();
    assert_eq!(metrics["input_bytes"], 5);
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
    let dir = temp_dir("invalid_pdf_operator_exits_with_code_3");
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
        .stderr(predicate::str::contains("expected PDF"));

    assert!(!output.exists());
}

#[test]
fn workflow_rotate_updates_pdf_page_rotation() {
    let dir = temp_dir("workflow_rotate_updates_pdf_page_rotation");
    let output = dir.join("output.pdf");
    let workflow = dir.join("workflow.yaml");
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
            fixture_pdf().display(),
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

    assert_eq!(pdf_page_rotation(&output, 1), 90);
}
