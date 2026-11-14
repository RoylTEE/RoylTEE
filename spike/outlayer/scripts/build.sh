#!/usr/bin/env bash
# Builds both wasm files into out/ with machine paths remapped, then fails if
# a local path is left in either file. Uses only the installed
# wasm32-unknown-unknown target (the echo module imports WASI Preview 1
# directly, see echo-wasi/src/lib.rs).
set -euo pipefail
cd "$(dirname "$0")/.."
TARGET=wasm32-unknown-unknown
CARGO_HOME_DIR="${CARGO_HOME:-${HOME:-}/.cargo}"
ROOT="$(pwd -P)"
SEP=$'\x1f'
export CARGO_ENCODED_RUSTFLAGS="--cfg${SEP}near${SEP}--remap-path-prefix=${CARGO_HOME_DIR}=/cargo${SEP}--remap-path-prefix=${ROOT}=/src"
unset RUSTFLAGS
cargo build -p echo_wasi -p tee_probe --target "$TARGET" --release --target-dir target/wasm --locked
mkdir -p out
cp target/wasm/$TARGET/release/echo_wasi.wasm out/echo_wasi.wasm
cp target/wasm/$TARGET/release/tee_probe.wasm out/tee_probe.wasm
for f in out/*.wasm; do
  for p in "$CARGO_HOME_DIR" "$ROOT" "${HOME:-/nonexistent}" /home/ /Users/; do
    if LC_ALL=C grep -a -q -F -- "$p" "$f"; then echo "build: $f contains a local path" >&2; exit 1; fi
  done
done
sha256sum out/*.wasm
stat -c '%s %n' out/*.wasm
