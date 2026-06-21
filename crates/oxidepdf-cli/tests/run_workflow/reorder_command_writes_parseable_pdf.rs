
#[test]
fn reorder_command_writes_parseable_pdf() {
    let dir = temp_dir("reorder_command_writes_parseable_pdf");
    let output = dir.join("reordered.pdf");

    Command::cargo_bin("oxidepdf")
        .unwrap()
        .args([
            "pdf_edit",
            "reorder-pages",
            fixture_pdf().to_str().unwrap(),
            "--pages",
            "3,1,2",
            "-o",
            output.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::eq(""))
        .stderr(predicate::eq(""));

    assert_eq!(pdf_page_count(&output), 3);
}

#[test]
fn img2pdf_command_writes_parseable_pdf() {
    let dir = temp_dir("img2pdf_command_writes_parseable_pdf");
    let output = dir.join("image.pdf");

    Command::cargo_bin("oxidepdf")
        .unwrap()
        .args([
            "pdf_edit",
            "img2pdf",
            fixture_jpg().to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::eq(""))
        .stderr(predicate::eq(""));

    assert_eq!(pdf_page_count(&output), 1);
}

#[test]
fn svg2pdf_command_writes_parseable_pdf() {
    let dir = temp_dir("svg2pdf_command_writes_parseable_pdf");
    let input = dir.join("input.svg");
    let output = dir.join("svg.pdf");
    fs::write(
        &input,
        br##"<svg xmlns="http://www.w3.org/2000/svg" width="120" height="80">
            <rect x="10" y="10" width="100" height="60" fill="#16a34a"/>
        </svg>"##,
    )
    .unwrap();

    Command::cargo_bin("oxidepdf")
        .unwrap()
        .args([
            "pdf_edit",
            "svg2pdf",
            input.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::eq(""))
        .stderr(predicate::eq(""));

    assert_eq!(pdf_page_count(&output), 1);
}

#[test]
fn compress_command_writes_parseable_pdf() {
    let dir = temp_dir("compress_command_writes_parseable_pdf");
    let output = dir.join("compressed.pdf");

    Command::cargo_bin("oxidepdf")
        .unwrap()
        .args([
            "pdf_edit",
            "compress",
            fixture_pdf().to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::eq(""))
        .stderr(predicate::eq(""));

    assert_eq!(pdf_page_count(&output), 3);
}

#[test]
fn compress_command_accepts_explicit_lossy_options() {
    let dir = temp_dir("compress_command_accepts_explicit_lossy_options");
    let input = dir.join("image.pdf");
    let output = dir.join("compressed.pdf");
    fs::write(&input, pdf_with_rgb_image()).unwrap();

    Command::cargo_bin("oxidepdf")
        .unwrap()
        .args([
            "pdf_edit",
            "compress",
            input.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
            "--mode",
            "lossy",
            "--image-quality",
            "80",
        ])
        .assert()
        .success()
        .stdout(predicate::eq(""))
        .stderr(predicate::eq(""));

    assert_eq!(pdf_page_count(&output), 1);
}

#[test]
fn encrypt_and_decrypt_commands_round_trip_pdf() {
    let dir = temp_dir("encrypt_and_decrypt_commands_round_trip_pdf");
    let encrypted = dir.join("encrypted.pdf");
    let decrypted = dir.join("decrypted.pdf");

    Command::cargo_bin("oxidepdf")
        .unwrap()
        .args([
            "pdf_security",
            "encrypt",
            fixture_pdf().to_str().unwrap(),
            "-o",
            encrypted.to_str().unwrap(),
            "--owner-password",
            "owner-pass",
            "--user-password",
            "user-pass",
            "--no-copy",
        ])
        .assert()
        .success()
        .stdout(predicate::eq(""))
        .stderr(predicate::eq(""));

    let encrypted_document = lopdf::Document::load(&encrypted).unwrap();
    assert!(encrypted_document.is_encrypted());

    Command::cargo_bin("oxidepdf")
        .unwrap()
        .args([
            "pdf_security",
            "decrypt",
            encrypted.to_str().unwrap(),
            "-o",
            decrypted.to_str().unwrap(),
            "--password",
            "user-pass",
        ])
        .assert()
        .success()
        .stdout(predicate::eq(""))
        .stderr(predicate::eq(""));

    assert_eq!(pdf_page_count(&decrypted), 3);
}

#[test]
fn decrypt_command_rejects_wrong_password_without_output_or_secret_leak() {
    let dir = temp_dir("decrypt_command_rejects_wrong_password_without_output_or_secret_leak");
    let encrypted = dir.join("encrypted.pdf");
    let output = dir.join("decrypted.pdf");

    Command::cargo_bin("oxidepdf")
        .unwrap()
        .args([
            "pdf_security",
            "encrypt",
            fixture_pdf().to_str().unwrap(),
            "-o",
            encrypted.to_str().unwrap(),
            "--owner-password",
            "owner-pass",
            "--user-password",
            "user-pass",
        ])
        .assert()
        .success();

    Command::cargo_bin("oxidepdf")
        .unwrap()
        .args([
            "pdf_security",
            "decrypt",
            encrypted.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
            "--password",
            "wrong-pass",
        ])
        .assert()
        .code(4)
        .stdout(predicate::eq(""))
        .stderr(
            predicate::str::contains("incorrect_password")
                .and(predicate::str::contains("wrong-pass").not()),
        );

    assert!(!output.exists());
}

#[test]
fn permissions_get_and_set_commands_write_expected_policy() {
    let dir = temp_dir("permissions_get_and_set_commands_write_expected_policy");
    let encrypted = dir.join("encrypted.pdf");
    let report = dir.join("permissions.json");
    let updated = dir.join("updated.pdf");

    Command::cargo_bin("oxidepdf")
        .unwrap()
        .args([
            "pdf_security",
            "encrypt",
            fixture_pdf().to_str().unwrap(),
            "-o",
            encrypted.to_str().unwrap(),
            "--owner-password",
            "owner-pass",
            "--user-password",
            "user-pass",
            "--no-copy",
            "--no-modify",
        ])
        .assert()
        .success();

    Command::cargo_bin("oxidepdf")
        .unwrap()
        .args([
            "pdf_security",
            "permissions",
            "get",
            encrypted.to_str().unwrap(),
            "-o",
            report.to_str().unwrap(),
            "--password",
            "user-pass",
        ])
        .assert()
        .success()
        .stdout(predicate::eq(""))
        .stderr(predicate::eq(""));

    let report_json: serde_json::Value =
        serde_json::from_slice(&fs::read(&report).unwrap()).unwrap();
    assert_eq!(report_json["encrypted"], true);
    assert_eq!(report_json["permissions"]["copy"], false);
    assert_eq!(report_json["permissions"]["modify"], false);

    Command::cargo_bin("oxidepdf")
        .unwrap()
        .args([
            "pdf_security",
            "permissions",
            "set",
            encrypted.to_str().unwrap(),
            "-o",
            updated.to_str().unwrap(),
            "--owner-password",
            "owner-pass",
            "--user-password",
            "new-user-pass",
            "--no-print",
        ])
        .assert()
        .success();

    let updated_document = lopdf::Document::load(&updated).unwrap();
    assert!(updated_document.is_encrypted());
}

#[test]
fn render_command_writes_png() {
    let dir = temp_dir("render_command_writes_png");
    let output = dir.join("page.png");

    Command::cargo_bin("oxidepdf")
        .unwrap()
        .args([
            "pdf_inspect",
            "render",
            fixture_pdf().to_str().unwrap(),
            "--page",
            "1",
            "-o",
            output.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::eq(""))
        .stderr(predicate::eq(""));

    let image = image::load_from_memory(&fs::read(output).unwrap()).unwrap();
    assert!(image.width() > 0);
    assert!(image.height() > 0);
}

#[test]
fn extract_text_command_writes_plain_text() {
    let dir = temp_dir("extract_text_command_writes_plain_text");
    let output = dir.join("extracted.txt");

    Command::cargo_bin("oxidepdf")
        .unwrap()
        .args([
            "pdf_inspect",
            "extract-text",
            fixture_pdf().to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::eq(""))
        .stderr(predicate::eq(""));

    assert!(!fs::read_to_string(output).unwrap().trim().is_empty());
}
