# OxidePDF 🦀📄

OxidePDF is a pure Rust PDF toolkit with a modular CLI and workflow engine. It focuses on practical document automation: editing pages, inspecting structure, extracting content, signing, comparing, compressing, and packaging PDFs from scripts or CI jobs.

## Highlights ✨

- 🧩 **Modular commands**: `edit`, `inspect`, `sign`, `metadata`, `outline`, `attach`, `annot`, `form`, `image`, `color`, `permissions`, and more.
- 🛠️ **PDF editing**: merge, page selection, reorder, rotate, delete, crop, scale, n-up, booklet, page numbers, image-to-PDF, SVG-to-PDF, watermarks, and compression.
- 🔍 **Inspection**: render pages to PNG and extract text.
- 🔐 **Security and signatures**: encrypt, decrypt, inspect/set permissions, add/list/verify/delete signature fields, add visual signature appearances, and attach explicit timestamp material.
- 📦 **Static Linux releases**: musl builds via `cargo zigbuild`.
- 🧭 **Bash completion**: generated at build time and available from the CLI.
- ⚙️ **Workflow orchestration**: YAML/JSON pipeline documents with DAG-based task scheduling, resource limits, and programmatic API.
- 🐳 **Container-friendly**: static binary copied into a `scratch` runtime image.

## Quick Start 🚀

Build locally:

```sh
cargo build -p oxidepdf-cli
```

Run help:

```sh
target/debug/oxidepdf --help
target/debug/oxidepdf edit --help
target/debug/oxidepdf sign verify --help
```

Every command and argument is documented in `-h` output.

## Common Commands

Merge PDFs:

```sh
oxidepdf edit merge a.pdf b.pdf -o merged.pdf
```

Extract text:

```sh
oxidepdf inspect extract-text input.pdf -o text.txt
```

Render a page:

```sh
oxidepdf inspect render input.pdf --page 1 -o page.png
```

Compress losslessly:

```sh
oxidepdf edit compress input.pdf -o compressed.pdf
```

Verify signatures:

```sh
oxidepdf sign verify signed.pdf -o signature-report.json
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

Create release zip archives:

```sh
VERSION=20260101 scripts/release.sh
```

Each zip contains:

```text
oxidepdf
oxidepdf.bash
LICENSE
README.md
```

Run from Docker:

```sh
cargo zigbuild --release --target x86_64-unknown-linux-musl -p oxidepdf-cli
docker build -t oxidepdf:local .
docker run --rm oxidepdf:local --help
```

## Advanced Workflow Orchestration

OxidePDF includes a YAML/JSON-based workflow engine for multi-step document automation. Instead of chaining CLI invocations together with shell scripts, you can declare an entire pipeline as a single workflow document — inputs, tasks, dependencies, and outputs — and let OxidePDF validate, plan, and execute it in one shot.

### Concepts

- **Workflow document**: a YAML or JSON file that declares inputs, tasks, outputs, and optional resource limits.
- **Tasks**: units of work, each referencing an operator (edit, inspect, sign, security, compare) and its input artifacts.
- **Artifacts**: named references to PDFs, images, text, or raw bytes that flow between tasks.
- **DAG execution**: tasks are topologically sorted by their artifact dependencies and run serially. Cycles are detected and rejected.
- **Resource limits**: enforce bounds on input bytes, total input bytes, page count, pixel count, output bytes, and execution time.

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
tasks:
  - id: compressed
    op:
      pdf_edit:
        compression:
          compress_images: true
          compress_streams: true
          lossless: true
    inputs: [source]
```

### Multi-Step Pipeline Example

This workflow merges two PDFs, rotates every page, and renders the result to PNG — all in one pass:

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
        merge:
          inputs: [cover, body]
    inputs: [cover, body]
  - id: rotated
    op:
      pdf_edit:
        rotate:
          rotation: 90
          page_selector: all
    inputs: [merged]
  - id: rendered
    op:
      pdf_inspect:
        render:
          page: 1
          dpi: 150
    inputs: [rotated]
```

### Operator Families

| Family | Examples |
|---|---|
| `pdf_edit` | merge, rotate, crop, scale, n-up, booklet, watermark, page-numbers, img2pdf, svg2pdf, compression |
| `pdf_inspect` | render, extract-text, metadata inspect, outline inspect, forms, images |
| `pdf_sign` | add, list, verify, delete-field, timestamp |
| `pdf_security` | encrypt, decrypt, permissions-get, permissions-set |
| `pdf_compare` | report, visual-diff |

Each task specifies exactly one operator. The engine validates references, detects cycles, and enforces limits before any work begins.

### Scripting and CI Integration

Workflows are designed for headless environments:

- **stdin/stdout**: use `-` as the workflow path or input path to read from stdin.
- **Exit codes**: 0 on success, 2 for invalid workflow, 3 for input error, 4 for auth error, 5 for resource-limit exceeded, 70 for internal errors.
- **Path redaction**: file paths are stripped from error output — safe for CI logs.
- **Static binary**: a single musl binary works in `scratch` containers and restricted CI runners.

### Programmatic API

The workflow engine is re-exported from `oxidepdf-core` for Rust embedders. Construct and execute workflows programmatically:

```rust
use oxidepdf_core::{Workflow, execute_workflow, PdfOperatorRunner, ArtifactStore};

let workflow: Workflow = serde_yaml::from_str(yaml_str)?;
let store = ArtifactStore::new();
let mut runner = PdfOperatorRunner::default();
let result = execute_workflow(&workflow, store, &mut runner)?;
```

Individual CLI commands (`edit merge`, `inspect render`, etc.) are implemented as single-task workflows internally, so the same validation and execution path serves both interactive use and workflow documents.

## Milestones 🗺️

These are not claimed as supported today:

- Native macOS release archives.
- Native Windows release archives.
- More shell completions, such as zsh, fish, and PowerShell.
- Package-manager distribution, such as Homebrew, APT/RPM, Nix, WinGet, and Scoop.
- Online TSA requests for timestamping.
- Full PAdES policy validation.
- Incremental PDF updates that preserve all original byte layout.
- OCR for scanned PDFs.
- PDF/A validation and conversion.
- Redaction workflows with visual and content-layer removal.
- More advanced table extraction.
- WebAssembly bindings.
- Library API stability guarantees for third-party embedders.

## License

OxidePDF is licensed under GPLv3. See [LICENSE](LICENSE).
