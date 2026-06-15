#!/usr/bin/env sh
set -eu

cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
TARGETS=x86_64-unknown-linux-musl scripts/release.sh
