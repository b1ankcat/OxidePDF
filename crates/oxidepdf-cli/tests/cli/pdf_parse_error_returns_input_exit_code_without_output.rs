
#[tokio::test]
async fn pdf_parse_error_returns_input_exit_code_without_output() {
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
    )
    .await;

    assert_eq!(code, 3);
    assert_eq!(stdout, b"");
    assert!(!output.exists());
    let stderr = String::from_utf8(stderr).unwrap();
    assert!(stderr.contains("invalid_input"));
    assert!(stderr.contains("expected PDF"));
}

#[tokio::test]
async fn workflow_enforces_total_input_size_limit() {
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
    )
    .await;

    assert_eq!(code, 5);
    assert_eq!(stdout, b"");
    assert!(!output.exists());
    assert!(
        String::from_utf8(stderr)
            .unwrap()
            .contains("max_total_input_bytes")
    );
}

#[tokio::test]
async fn workflow_enforces_output_size_limit() {
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
    )
    .await;

    assert_eq!(code, 5);
    assert_eq!(stdout, b"");
    assert!(!output.exists());
    assert!(
        String::from_utf8(stderr)
            .unwrap()
            .contains("max_output_bytes")
    );
}

#[tokio::test]
async fn error_output_redacts_sensitive_material_and_paths() {
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
    )
    .await;
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

#[tokio::test]
async fn invalid_workflow_returns_usage_exit_code() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let code = run_with_io(
        ["oxidepdf", "run", "--workflow", "-"],
        b"version: 1\ninputs: []\ntasks: []\n",
        &mut stdout,
        &mut stderr,
    )
    .await;

    assert_eq!(code, 2);
    assert_eq!(stdout, b"");
    assert!(String::from_utf8(stderr).unwrap().contains("workflow"));
}

#[tokio::test]
async fn missing_input_file_returns_input_exit_code() {
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
    )
    .await;

    assert_eq!(code, 3);
    assert_eq!(stdout, b"");
    assert!(String::from_utf8(stderr).unwrap().contains("input"));
}

#[tokio::test]
async fn output_file_is_not_overwritten_without_force() {
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
    )
    .await;

    assert_eq!(code, 2);
    assert_eq!(fs::read(output).unwrap(), b"existing");
    assert!(
        String::from_utf8(stderr)
            .unwrap()
            .contains("already exists")
    );
}
