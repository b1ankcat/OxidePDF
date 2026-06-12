#!/usr/bin/env sh
set -eu

cargo fmt --all -- --check
cargo clippy --workspace --all-targets
cargo test --workspace
cargo zigbuild --release --target x86_64-unknown-linux-musl -p oxidepdf-cli
