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
    assert!(script.contains("tar"));
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
    assert!(docs.contains("macOS"));
    assert!(docs.contains("Windows"));
    assert!(docs.contains("Release Checklist"));
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
