#!/usr/bin/env sh
set -eu

TARGETS="${TARGETS:-x86_64-unknown-linux-musl aarch64-unknown-linux-musl}"
PACKAGE="${PACKAGE:-oxidepdf-cli}"
BIN="${BIN:-oxidepdf}"
DIST_DIR="${DIST_DIR:-dist}"
VERSION="${VERSION:-$(date +%Y%m%d)}"

command -v cargo >/dev/null 2>&1 || {
  echo "cargo is required" >&2
  exit 127
}

if ! cargo zigbuild --help >/dev/null 2>&1; then
  echo "cargo-zigbuild is required; install with: cargo install cargo-zigbuild" >&2
  exit 127
fi

command -v zip >/dev/null 2>&1 || {
  echo "zip is required" >&2
  exit 127
}

command -v sha256sum >/dev/null 2>&1 || {
  echo "sha256sum is required" >&2
  exit 127
}

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
  completion="target/$target/release/completions/$BIN.bash"
  if [ ! -x "$binary" ]; then
    echo "Expected release binary not found: $binary" >&2
    exit 1
  fi
  if [ ! -f "$completion" ]; then
    echo "Expected bash completion file not found: $completion" >&2
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

  package_dir="$DIST_DIR/$BIN-$VERSION-$target"
  rm -rf "$package_dir"
  mkdir -p "$package_dir"
  cp "$binary" "$package_dir/$BIN"
  cp "$completion" "$package_dir/$BIN.bash"
  cp LICENSE "$package_dir/LICENSE"
  cp README.md "$package_dir/README.md"
  rm -f "$DIST_DIR/$BIN-$VERSION-$target.zip" "$DIST_DIR/$BIN-$VERSION-$target.zip.sha256"
  (cd "$DIST_DIR" && zip -qr "$BIN-$VERSION-$target.zip" "$BIN-$VERSION-$target")
  sha256sum "$DIST_DIR/$BIN-$VERSION-$target.zip" > "$DIST_DIR/$BIN-$VERSION-$target.zip.sha256"
done
