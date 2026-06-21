use std::fs;
use std::path::PathBuf;

#[test]
fn release_script_builds_musl_targets_and_checks_linkage() {
    let script = read("scripts/release.sh");

    assert!(script.contains("x86_64-unknown-linux-musl"));
    assert!(script.contains("aarch64-unknown-linux-musl"));
    assert!(script.contains("cargo zigbuild --release --target"));
    assert!(script.contains("WEB_PACKAGE=\"${WEB_PACKAGE:-oxidepdf-web}\""));
    assert!(script.contains("WEB_BIN=\"${WEB_BIN:-oxidepdf-web}\""));
    assert!(script.contains("validate_component WEB_PACKAGE"));
    assert!(script.contains("validate_component WEB_BIN"));
    assert!(script.contains("-p \"$WEB_PACKAGE\""));
    assert!(script.contains("web_binary=\"target/$target/release/$WEB_BIN\""));
    assert!(script.contains("for release_binary in \"$binary\" \"$web_binary\""));
    assert!(script.contains("cp \"$web_binary\" \"$package_dir/$WEB_BIN\""));
    assert!(script.contains("ldd"));
    assert!(script.contains("ldd is required"));
    assert!(script.contains("not a dynamic executable"));
    assert!(script.contains("zip"));
    assert!(script.contains("$BIN.bash"));
    assert!(script.contains("README.md"));
    assert!(script.contains("sha256sum"));
    assert!(script.contains("seen_target=false"));
    assert!(script.contains("Invalid TARGETS"));
    assert!(script.contains("validate_component PACKAGE"));
    assert!(script.contains("validate_version"));
    assert!(script.contains("*[!A-Za-z0-9._+-]*"));
    assert!(script.contains("VERSION=\"${VERSION:-$(package_version)}\""));
    assert!(script.contains("BUILD_DOCKER_IMAGE=\"${BUILD_DOCKER_IMAGE:-false}\""));
    assert!(script.contains("DOCKER_IMAGE=\"${DOCKER_IMAGE:-oxidepdf:local}\""));
    assert!(script.contains("validate_docker_image"));
    assert!(script.contains("docker build -t \"$DOCKER_IMAGE\" ."));
}

#[test]
fn check_script_runs_release_build_for_primary_linux_target() {
    let script = read("scripts/check.sh");

    assert!(script.contains("cargo fmt --all -- --check"));
    assert!(
        script.contains("cargo clippy --workspace --all-targets --all-features -- -D warnings")
    );
    assert!(script.contains("cargo test --workspace --all-targets --all-features"));
    assert!(script.contains("cargo-audit is required"));
    assert!(script.contains("cargo audit"));
    assert!(script.contains("TARGETS=x86_64-unknown-linux-musl scripts/release.sh"));
}

#[test]
fn github_ci_runs_the_same_core_quality_gates_as_check_script() {
    let workflow = read(".github/workflows/ci.yml");

    assert!(workflow.contains("cargo fmt --all -- --check"));
    assert!(
        workflow.contains("cargo clippy --workspace --all-targets --all-features -- -D warnings")
    );
    assert!(workflow.contains("cargo test --workspace --all-targets --all-features"));
    assert!(workflow.contains("tool: cargo-audit"));
    assert!(workflow.contains("cargo audit"));
    assert!(workflow.contains("scripts/bench-smoke.sh"));
}

#[test]
fn dockerfile_uses_prebuilt_static_web_binary() {
    let dockerfile = read("Dockerfile");

    assert!(dockerfile.contains("FROM scratch"));
    assert!(dockerfile.contains("FROM alpine:3.20 AS fonts"));
    assert!(dockerfile.contains("apk add --no-cache fontconfig font-noto-cjk ttf-dejavu"));
    assert!(dockerfile.contains(
        "COPY target/x86_64-unknown-linux-musl/release/oxidepdf-web /var/lib/oxidepdf/oxidepdf-web"
    ));
    assert!(dockerfile.contains("COPY --from=certs /etc/ssl/certs/ca-certificates.crt"));
    assert!(dockerfile.contains("COPY --from=fonts /etc/fonts /etc/fonts"));
    assert!(dockerfile.contains("COPY --from=fonts /usr/share/fonts /usr/share/fonts"));
    assert!(dockerfile.contains("WORKDIR /var/lib/oxidepdf"));
    assert!(dockerfile.contains("VOLUME [\"/var/lib/oxidepdf/upload\"]"));
    assert!(dockerfile.contains("EXPOSE 19898"));
    assert!(dockerfile.contains("ENV OXIDEPDF_AUTH_USER=admin"));
    assert!(!dockerfile.contains("ENV OXIDEPDF_AUTH_PASS=admin"));
    assert!(dockerfile.contains("provide"));
    assert!(dockerfile.contains("OXIDEPDF_AUTH_PASS at runtime"));
    assert!(dockerfile.contains("ENTRYPOINT [\"/var/lib/oxidepdf/oxidepdf-web\"]"));
    assert!(dockerfile.contains("CMD [\"--addr\", \"0.0.0.0\", \"--port\", \"19898\"]"));
}

#[test]
fn readme_documents_open_source_distribution_and_milestones() {
    let readme = read("README.md");

    assert!(readme.contains("OxidePDF"));
    assert!(readme.contains("GPLv3"));
    assert!(readme.contains("oxidepdf completion bash"));
    assert!(readme.contains("Deployment and Distribution"));
    assert!(readme.contains("OXIDEPDF_MAX_UPLOAD=256M"));
    assert!(readme.contains("BUILD_DOCKER_IMAGE=true"));
    assert!(readme.contains("admin / change-me"));
    assert!(readme.contains("does not ship with a default password"));
    assert!(readme.contains("DejaVu and Noto CJK system fonts"));
    assert!(readme.contains("English/Chinese text watermarks and overlays"));
    assert!(readme.contains("/var/lib/oxidepdf/upload"));
    assert!(readme.contains("under `./upload`"));
    assert!(readme.contains("small multipart overhead"));
    assert!(readme.contains("--allow-unauth-network"));
    assert!(readme.contains("--max-upload"));
    assert!(readme.contains("Each zip contains"));
    assert!(readme.contains("oxidepdf-web"));
    assert!(readme.contains("GitHub tag releases publish the same combined archive layout."));
    assert!(readme.contains("English and Chinese"));
    assert!(readme.contains("drag-and-drop upload preview zone"));
    assert!(readme.contains("Apache-2.0"));
}

#[test]
fn github_release_workflow_builds_musl_zip_release() {
    let workflow = read(".github/workflows/release.yml");

    assert!(workflow.contains("workflow_dispatch"));
    assert!(workflow.contains("WEB_PACKAGE: oxidepdf-web"));
    assert!(workflow.contains("WEB_BIN: oxidepdf-web"));
    assert!(workflow.contains("cargo zigbuild --release --target \"$TARGET\" -p \"$PACKAGE\""));
    assert!(workflow.contains("cargo zigbuild --release --target \"$TARGET\" -p \"$WEB_PACKAGE\""));
    assert!(workflow.contains("x86_64-unknown-linux-musl"));
    assert!(workflow.contains("${BIN}.bash"));
    assert!(workflow.contains("\"$package_dir/${WEB_BIN}\""));
    assert!(workflow.contains("zip -qr"));
    assert!(workflow.contains("softprops/action-gh-release"));
    assert!(workflow.contains("cargo metadata --no-deps"));
    assert!(workflow.contains("value=v${version}"));
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
