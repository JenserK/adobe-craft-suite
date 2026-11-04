#!/usr/bin/env bash
# Build the browser version of FilmCraft into ./web-dist (used by Netlify and Capacitor).
#
# Usage: scripts/build-web.sh [--ci]
#   --ci   also install what a bare CI image lacks: rustup + stable toolchain, the
#          wasm32-unknown-unknown target and the wasm-bindgen CLI (prebuilt binary, exact version).
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
ROOT="$PWD"

if [ "${1:-}" = "--ci" ]; then
  if ! command -v cargo >/dev/null 2>&1; then
    export PATH="$HOME/.cargo/bin:$PATH"
  fi
  if ! command -v cargo >/dev/null 2>&1; then
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain stable
    export PATH="$HOME/.cargo/bin:$PATH"
  fi
  # rust-version in Cargo.toml is 1.95; make sure the toolchain is current enough.
  rustup update stable --no-self-update
  rustup target add wasm32-unknown-unknown

  # The CLI version must equal the wasm-bindgen crate version (single source: xtask).
  WBG="$(sed -n 's/^const WASM_BINDGEN: &str = "\(.*\)";/\1/p' xtask/src/main.rs)"
  if ! command -v wasm-bindgen >/dev/null 2>&1 || ! wasm-bindgen --version | grep -q "$WBG"; then
    BIN="$HOME/.cargo/bin"
    URL="https://github.com/rustwasm/wasm-bindgen/releases/download/$WBG/wasm-bindgen-$WBG-x86_64-unknown-linux-musl.tar.gz"
    if curl -fsSL "$URL" | tar xz -C "$BIN" --strip-components=1 "wasm-bindgen-$WBG-x86_64-unknown-linux-musl/wasm-bindgen"; then
      :
    else
      cargo install wasm-bindgen-cli --version "$WBG" --locked
    fi
  fi

  # wasm-opt (binaryen) makes the wasm smaller and faster; `cargo xtask web` uses it when found.
  # Best effort: a failed download or a binary that does not run just skips it.
  # Set FILMCRAFT_NO_WASM_OPT=1 to build without it.
  if [ -z "${FILMCRAFT_NO_WASM_OPT:-}" ] && ! command -v wasm-opt >/dev/null 2>&1; then
    BV="version_119"
    if curl -fsSL "https://github.com/WebAssembly/binaryen/releases/download/$BV/binaryen-$BV-x86_64-linux.tar.gz" \
        | tar xz -C "$HOME/.cargo/bin" --strip-components=2 "binaryen-$BV/bin/wasm-opt" 2>/dev/null \
      && wasm-opt --version >/dev/null 2>&1; then
      echo "wasm-opt: $(wasm-opt --version)"
    else
      echo "wasm-opt: not available, building without it" >&2
      rm -f "$HOME/.cargo/bin/wasm-opt"
    fi
  fi
fi

cargo xtask web

CARGO_TARGET="${CARGO_TARGET_DIR:-$ROOT/target}"
rm -rf "$ROOT/web-dist"
mkdir -p "$ROOT/web-dist"
cp -R "$CARGO_TARGET/web/dist/." "$ROOT/web-dist/"
# Netlify / Cloudflare Pages header rules (MIME type for .wasm, caching).
cp "$ROOT/packaging/web/_headers" "$ROOT/web-dist/_headers"
echo "web-dist ready: $(du -sh "$ROOT/web-dist" | cut -f1)"
