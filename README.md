# OxidePDF 🦀📄

OxidePDF 0.4.1 is a Rust PDF toolkit for editing, inspecting, signing, comparing, and automating document workflows. It ships as a CLI, a workflow engine, and a web UI.

## Highlights ✨

- 🧩 **CLI families**: `pdf_edit`, `pdf_inspect`, `pdf_security`, `pdf_compare`, `pdf_sign`, and `pdf_adv`
- ⚙️ **Workflow orchestration**: YAML/JSON DAG execution with retries, limits, and timeouts
- 🌐 **Web UI**: static `oxidepdf-web` with schema-generated forms, bilingual UI, drag-and-drop upload, and file previews
- 📦 **Releases**: CLI + web musl binaries, bash completion, and `cargo zigbuild`
- 🐳 **Container-ready**: static binary in a `scratch` image

## Quick Start 🚀

Build the CLI:

```sh
cargo build -p oxidepdf-cli
```

Run help:

```sh
target/debug/oxidepdf --help
target/debug/oxidepdf pdf_edit --help
target/debug/oxidepdf pdf_sign verify --help
```

Every command and argument is documented in `-h` output.

## Common Commands

Merge PDFs:

```sh
oxidepdf pdf_edit merge a.pdf b.pdf -o merged.pdf
```

Extract text:

```sh
oxidepdf pdf_inspect extract-text input.pdf -o text.txt
```

Render a page:

```sh
oxidepdf pdf_inspect render input.pdf --page 1 -o page.png
```

Compress losslessly:

```sh
oxidepdf pdf_edit compress input.pdf -o compressed.pdf
```

Verify signatures:

```sh
oxidepdf pdf_sign verify signed.pdf -o signature-report.json
```

Generate bash completion:

```sh
source <(oxidepdf completion bash)
```

## Bash Completion

`cargo build` generates:

```text
target/debug/completions/oxidepdf.bash
```

`cargo zigbuild --release --target x86_64-unknown-linux-musl -p oxidepdf-cli` generates:

```text
target/x86_64-unknown-linux-musl/release/completions/oxidepdf.bash
```

Users deploy completion themselves. For the current shell:

```sh
source ./oxidepdf.bash
```

For a user-level bash-completion install:

```sh
mkdir -p "${XDG_DATA_HOME:-$HOME/.local/share}/bash-completion/completions"
cp oxidepdf.bash "${XDG_DATA_HOME:-$HOME/.local/share}/bash-completion/completions/oxidepdf"
```

## Deployment and Distribution 📦

Install release tooling:

```sh
cargo install cargo-zigbuild
rustup target add x86_64-unknown-linux-musl
rustup target add aarch64-unknown-linux-musl
```

Build a static Linux binary:

```sh
cargo zigbuild --release --target x86_64-unknown-linux-musl -p oxidepdf-cli
```

Create release zip archives. By default, archive names use the package version
from `Cargo.toml`; pass `VERSION=...` only when you need an explicit override:

```sh
scripts/release.sh
```

The release script rejects empty or whitespace-only `TARGETS` and accepts
standard Cargo version strings, including build metadata. Set
`BUILD_DOCKER_IMAGE=true` to also build the local Docker image after the
`x86_64-unknown-linux-musl` web binary is available; override the image tag with
`DOCKER_IMAGE=...`.

Each zip contains:

```text
oxidepdf
oxidepdf-web
oxidepdf.bash
LICENSE
README.md
```

GitHub tag releases publish the same combined archive layout.

Run from Docker:

The image bundles the web front end, DejaVu and Noto CJK system fonts for
English/Chinese text watermarks and overlays, and serves it on port 19898:

```sh
cargo zigbuild --release --target x86_64-unknown-linux-musl -p oxidepdf-web
docker build -t oxidepdf:local .
docker run --rm -p 19898:19898 \
  -v "$PWD/oxidepdf-upload:/var/lib/oxidepdf/upload" \
  oxidepdf:local
# then open http://localhost:19898 and sign in with admin / admin
```

The container binds `0.0.0.0` and enables HTTP Basic auth by default with
`admin` / `admin`. Override both credentials before exposing it outside a trusted
development environment. Every flag also has an environment variable, so the same
options work via `docker run -e`:

```sh
docker run --rm -p 19898:19898 \
  -e OXIDEPDF_AUTH_USER=admin \
  -e OXIDEPDF_AUTH_PASS=change-me \
  -e OXIDEPDF_MAX_STORAGE=1G \
  -e OXIDEPDF_MAX_UPLOAD=256M \
  -v "$PWD/oxidepdf-upload:/var/lib/oxidepdf/upload" \
  oxidepdf:local
```

`OXIDEPDF_MAX_UPLOAD` limits each uploaded file and matching workflow resource
limits. The HTTP request body allows a small multipart overhead above that
value. The web server stores uploaded and produced artifacts under `upload/`
relative to its current working directory; the Docker image runs from
`/var/lib/oxidepdf` and declares `/var/lib/oxidepdf/upload` as a volume.

## Workflows ⚙️

Declare multi-step PDF automation as YAML or JSON: inputs, tasks, dependencies,
outputs, and optional limits. OxidePDF validates the graph, detects cycles, and
runs ready tasks as soon as their dependencies finish.

### Concepts

- **Tasks** reference one operator and one or more artifacts.
- **Artifacts** are named PDFs, images, text, or raw byte payloads.
- **Limits** bound input bytes, pages, pixels, output bytes, retries, rate, and timeouts.

### Running a Workflow

```sh
oxidepdf run --workflow pipeline.yaml
# or from stdin:
cat pipeline.yaml | oxidepdf run --workflow -
```

The `--force` flag allows overwriting existing output files.

### Document Structure

```yaml
version: 1
inputs:
  - id: source
    path: input.pdf
outputs:
  - id: result
    from: compressed
    path: output.pdf
limits:
  max_input_bytes: 104857600     # 100 MB per input
  max_total_input_bytes: 209715200
  max_pages: 5000
  max_pixels: 200000000
  max_output_bytes: 524288000
  timeout_ms: 300000             # 5 min
  retry_attempts: 2
  rate_limit_per_second: 10
tasks:
  - id: compressed
    op:
      pdf_edit:
        compression:
          mode: lossless
    inputs: [source]
```

### Multi-Step Example

Merge two PDFs, rotate the first two pages, and render a preview:

```yaml
version: 1
inputs:
  - id: cover
    path: cover.pdf
  - id: body
    path: body.pdf
outputs:
  - id: preview
    from: rendered
    path: page1.png
tasks:
  - id: merged
    op:
      pdf_edit:
        merge: {}
    inputs: [cover, body]
  - id: rotated
    op:
      pdf_edit:
        rotate_pages:
          pages: "1-2"
          degrees: 90
    inputs: [merged]
  - id: rendered
    op:
      pdf_inspect:
        render:
          page: 1
          scale: 2.0
    inputs: [rotated]
```

### Operator Families

| Family | Examples |
|---|---|
| `pdf_edit` | merge, rotate, crop, scale, n-up, booklet, watermark, page-numbers, img2pdf, svg2pdf, compression |
| `pdf_inspect` | render, extract-text |
| `pdf_sign` | add, list, verify, delete-field, timestamp |
| `pdf_security` | encrypt, decrypt, permissions |
| `pdf_compare` | report, visual-diff |

The CLI uses the same operator families: `pdf_edit`, `pdf_inspect`,
`pdf_security`, `pdf_compare`, `pdf_sign`, plus `pdf_adv` for metadata, outline,
attachment, annotation, form, and image operations.

### Scripting and CI Integration

- **stdin/stdout**: use `-` as the workflow path or input path to read from stdin.
- **Exit codes**: 0 on success, 2 for invalid workflow, 3 for input error, 4 for auth error, 5 for resource-limit exceeded, 70 for internal errors.
- **Path redaction**: file paths are stripped from error output — safe for CI logs.
- **Static binary**: a single musl binary works in `scratch` containers and restricted CI runners.

### Programmatic API

The workflow engine is re-exported from `oxidepdf-core` for Rust embedders. Construct and execute workflows programmatically from async code:

```rust
use oxidepdf_core::{Workflow, execute_workflow, PdfOperatorRunner, ArtifactStore};

let workflow: Workflow = serde_saphyr::from_str(yaml_str)?;
let store = ArtifactStore::new();
let runner = PdfOperatorRunner::default();
let result = execute_workflow(&workflow, store, runner).await?;
```

Individual CLI commands (`pdf_edit merge`, `pdf_inspect render`, etc.) use the
same validation and execution path as workflow documents.

## Web UI 🌐

`oxidepdf-web` is a self-contained web front end for uploads, single operations,
visual workflow building, previews, and downloads. The interface supports
English and Chinese, includes a drag-and-drop upload preview zone, and binds to
`127.0.0.1:19898` by default when run directly.

```sh
cargo run -p oxidepdf-web
# then open http://localhost:19898
```

Configuration (every flag has a matching environment variable):

| Flag | Env | Default | Purpose |
|------|-----|---------|---------|
| `--addr` | `OXIDEPDF_ADDR` | `127.0.0.1` | Bind address |
| `--port` | `OXIDEPDF_PORT` | `19898` | Port |
| `--max-storage` | `OXIDEPDF_MAX_STORAGE` | `2G` | Total artifact cap (`2G`, `1024M`, `100K`, binary units) before oldest-first eviction |
| `--max-upload` | `OXIDEPDF_MAX_UPLOAD` | `128M` | Per-file upload cap and matching web workflow input/output cap; the HTTP body allows a small multipart overhead above this |
| `--auth-user` | `OXIDEPDF_AUTH_USER` | — | HTTP Basic username (enables auth with `--auth-pass`) |
| `--auth-pass` | `OXIDEPDF_AUTH_PASS` | — | HTTP Basic password |
| `--allow-unauth-network` | `OXIDEPDF_ALLOW_UNAUTH_NETWORK` | `false` | Explicitly allow unauthenticated non-loopback binds |

> ⚠️ Auth is **off** unless both `--auth-user` and `--auth-pass` are set; then
> every request requires HTTP Basic credentials. The server binds to loopback by
> default. Binding to a non-loopback address without auth is refused unless
> `--allow-unauth-network` is set.
> The Docker image differs intentionally: it binds `0.0.0.0` and sets default
> credentials `admin` / `admin` so published ports work immediately while still
> requiring authentication.
> Uploaded and produced files are held under `./upload` and evicted
> automatically (oldest-first past 256 files / the storage cap, and after 30
> minutes idle).

The web API exposes schema, upload, single-op execution, workflow execution, and
file download/delete endpoints. Server-local path options are not exposed through
the web API; uploads stream to files in the server working directory's `upload`
subdirectory and browser-built workflows enforce bounded upload, task-count,
per-task-input, output-size, and timeout limits.

## License

OxidePDF uses a split license:

- `oxidepdf-core` (the library) is licensed under **Apache-2.0**. See
  [crates/oxidepdf-core/LICENSE-APACHE](crates/oxidepdf-core/LICENSE-APACHE).
- `oxidepdf-cli` and `oxidepdf-web` (the end-user applications) are licensed
  under **GPLv3 or later** (`GPL-3.0-or-later`). See [LICENSE](LICENSE).
