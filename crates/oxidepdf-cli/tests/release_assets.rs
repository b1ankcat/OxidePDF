use std::fs;
use std::path::PathBuf;

#[test]
fn release_script_builds_musl_targets_and_checks_linkage() {
    let script = read("scripts/release.sh");

    assert!(script.contains("x86_64-unknown-linux-musl"));
    assert!(script.contains("aarch64-unknown-linux-musl"));
    assert!(script.contains("cargo zigbuild --release --target"));
    assert!(script.contains("ldd"));
    assert!(script.contains("not a dynamic executable"));
    assert!(script.contains("zip"));
    assert!(script.contains("$BIN.bash"));
    assert!(script.contains("README.md"));
    assert!(script.contains("sha256sum"));
}

#[test]
fn check_script_runs_release_build_for_primary_linux_target() {
    let script = read("scripts/check.sh");

    assert!(script.contains("cargo fmt --all -- --check"));
    assert!(script.contains("cargo clippy --workspace --all-targets"));
    assert!(script.contains("cargo test --workspace"));
    assert!(script.contains("TARGETS=x86_64-unknown-linux-musl scripts/release.sh"));
}

#[test]
fn dockerfile_uses_prebuilt_static_cli_binary() {
    let dockerfile = read("Dockerfile");

    assert!(dockerfile.contains("FROM scratch"));
    assert!(dockerfile.contains("COPY target/x86_64-unknown-linux-musl/release/oxidepdf"));
    assert!(dockerfile
        .contains("COPY target/x86_64-unknown-linux-musl/release/completions/oxidepdf.bash"));
    assert!(dockerfile.contains("COPY --from=certs /etc/ssl/certs/ca-certificates.crt"));
    assert!(dockerfile.contains("ENTRYPOINT [\"/oxidepdf\"]"));
}

#[test]
fn release_documentation_records_packaging_commands_and_checklist() {
    let docs = read("docs/release.md");

    assert!(docs.contains("cargo install cargo-zigbuild"));
    assert!(docs.contains("rustup target add x86_64-unknown-linux-musl"));
    assert!(docs.contains("rustup target add aarch64-unknown-linux-musl"));
    assert!(docs
        .contains("cargo zigbuild --release --target x86_64-unknown-linux-musl -p oxidepdf-cli"));
    assert!(docs
        .contains("cargo zigbuild --release --target aarch64-unknown-linux-musl -p oxidepdf-cli"));
    assert!(docs.contains("ldd target/x86_64-unknown-linux-musl/release/oxidepdf"));
    assert!(docs.contains("docker build"));
    assert!(docs.contains("docker run --rm oxidepdf:local --help"));
    assert!(docs.contains("oxidepdf.bash"));
    assert!(docs.contains(".github/workflows/release.yml"));
    assert!(docs.contains("Release Checklist"));
}

#[test]
fn readme_documents_open_source_distribution_and_milestones() {
    let readme = read("README.md");

    assert!(readme.contains("OxidePDF"));
    assert!(readme.contains("GPLv3"));
    assert!(readme.contains("oxidepdf completion bash"));
    assert!(readme.contains("Deployment and Distribution"));
    assert!(readme.contains("Milestones"));
    assert!(readme.contains("Native macOS release archives"));
    assert!(readme.contains("Online TSA requests"));
}

#[test]
fn github_release_workflow_builds_musl_zip_release() {
    let workflow = read(".github/workflows/release.yml");

    assert!(workflow.contains("workflow_dispatch"));
    assert!(workflow.contains("cargo zigbuild --release --target"));
    assert!(workflow.contains("x86_64-unknown-linux-musl"));
    assert!(workflow.contains("${BIN}.bash"));
    assert!(workflow.contains("zip -qr"));
    assert!(workflow.contains("softprops/action-gh-release"));
    assert!(workflow.contains("date -u +%Y%m%d"));
}

fn read(path: &str) -> String {
    fs::read_to_string(repo_root().join(path))
        .unwrap_or_else(|error| panic!("failed to read {path}: {error}"))
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}
