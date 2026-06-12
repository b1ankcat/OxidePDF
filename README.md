# OxidePDF 🦀📄

OxidePDF is a pure Rust PDF toolkit with a modular CLI and workflow engine. It focuses on practical document automation: editing pages, inspecting structure, extracting content, signing, comparing, compressing, and packaging PDFs from scripts or CI jobs.

## Highlights ✨

- 🧩 **Modular commands**: `edit`, `inspect`, `sign`, `metadata`, `outline`, `attach`, `annot`, `form`, `image`, `color`, `permissions`, and more.
- 🛠️ **PDF editing**: merge, page selection, reorder, rotate, delete, crop, scale, n-up, booklet, page numbers, image-to-PDF, SVG-to-PDF, watermarks, and compression.
- 🔍 **Inspection**: render pages to PNG and extract text.
- 🔐 **Security and signatures**: encrypt, decrypt, inspect/set permissions, add/list/verify/delete signature fields, add visual signature appearances, and attach explicit timestamp material.
- 📦 **Static Linux releases**: musl builds via `cargo zigbuild`.
- 🧭 **Bash completion**: generated at build time and available from the CLI.
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

## GitHub Releases

The release workflow builds `x86_64-unknown-linux-musl` on push or manual dispatch, packages the binary and bash completion file into a zip archive, and publishes it to a date-style GitHub Release such as `20260101`.

Manual release:

```text
Actions -> Release -> Run workflow
```

## Project Layout

```text
crates/oxidepdf-core  Core PDF operators and workflow engine
crates/oxidepdf-cli   CLI, help text, completion generation, integration tests
docs/release.md       Release and packaging guide
scripts/release.sh    Local release packaging script
Dockerfile            Static binary runtime image
```

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
