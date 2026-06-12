#!/usr/bin/env sh
set -eu

TARGETS="${TARGETS:-x86_64-unknown-linux-musl aarch64-unknown-linux-musl}"
PACKAGE="${PACKAGE:-oxidepdf-cli}"
BIN="${BIN:-oxidepdf}"
DIST_DIR="${DIST_DIR:-dist}"

command -v cargo >/dev/null 2>&1 || {
  echo "cargo is required" >&2
  exit 127
}

if ! cargo zigbuild --help >/dev/null 2>&1; then
  echo "cargo-zigbuild is required; install with: cargo install cargo-zigbuild" >&2
  exit 127
fi

mkdir -p "$DIST_DIR"

for target in $TARGETS; do
  echo "Building $BIN for $target"
  if ! cargo zigbuild --release --target "$target" -p "$PACKAGE"; then
    echo "Failed to build $target. Ensure the Rust target is installed with:" >&2
    echo "  rustup target add $target" >&2
    echo "and Zig is available on PATH for cargo-zigbuild." >&2
    exit 1
  fi

  binary="target/$target/release/$BIN"
  if [ ! -x "$binary" ]; then
    echo "Expected release binary not found: $binary" >&2
    exit 1
  fi

  if command -v ldd >/dev/null 2>&1; then
    ldd_output="$(ldd "$binary" 2>&1 || true)"
    case "$ldd_output" in
      *"not a dynamic executable"* | *"statically linked"*) ;;
      *)
        echo "Expected $binary to be static; ldd reported:" >&2
        echo "$ldd_output" >&2
        exit 1
        ;;
    esac
  else
    echo "ldd not found; skipping static linkage check for $binary" >&2
  fi

  package_dir="$DIST_DIR/$BIN-$target"
  rm -rf "$package_dir"
  mkdir -p "$package_dir"
  cp "$binary" "$package_dir/$BIN"
  cp LICENSE "$package_dir/LICENSE"
  tar -C "$DIST_DIR" -czf "$DIST_DIR/$BIN-$target.tar.gz" "$BIN-$target"
  sha256sum "$DIST_DIR/$BIN-$target.tar.gz" > "$DIST_DIR/$BIN-$target.tar.gz.sha256"
done
