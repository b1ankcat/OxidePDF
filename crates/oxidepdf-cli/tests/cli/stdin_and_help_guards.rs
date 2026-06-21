
#[tokio::test]
async fn merge_rejects_multiple_stdin_inputs() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let code = run_with_io(
        ["oxidepdf", "pdf_edit", "merge", "-", "-", "-o", "out.pdf"],
        b"%PDF-1.7\n",
        &mut stdout,
        &mut stderr,
    )
    .await;

    assert_eq!(code, 2);
    assert_eq!(stdout, b"");
    assert!(
        String::from_utf8(stderr)
            .unwrap()
            .contains("more than one input from stdin")
    );
}

#[tokio::test]
async fn help_flag_writes_to_stdout_with_success_exit_code() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let code = run_with_io(["oxidepdf", "--help"], [], &mut stdout, &mut stderr).await;

    assert_eq!(code, 0);
    assert!(stderr.is_empty());
    let stdout = String::from_utf8(stdout).unwrap();
    assert!(stdout.contains("Usage"));
    assert!(!stdout.contains("oxidepdf:"));
}
