#!/usr/bin/env sh
set -eu

cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets --all-features
if ! cargo audit --version >/dev/null 2>&1; then
  echo "cargo-audit is required; install with: cargo install cargo-audit" >&2
  exit 127
fi
cargo audit
TARGETS=x86_64-unknown-linux-musl scripts/release.sh
