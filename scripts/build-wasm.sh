#!/usr/bin/env bash
# Builds every contract to wasm and copies the files to contracts/res/.
# The vault is built first because the registry embeds its code.
# `--cfg near` makes near-sdk use the on-chain host functions (what
# cargo-near sets); plain cargo is enough, no cargo-near install needed.
set -euo pipefail
cd "$(dirname "$0")/.."

TARGET=wasm32-unknown-unknown
OUT=target/wasm/$TARGET/release
# --remap-path-prefix keeps machine paths out of the wasm: rustc writes source
# paths into panic messages, so the cargo home and the repo root are mapped to
# fixed neutral prefixes (/cargo and /src). Proven so far: two clones of the
# same commit at two different paths on the same machine gave byte-identical
# wasm (same sha256 for all four files). The same bytes on other machines with
# the same toolchain and lock file are UNVERIFIED. CARGO_ENCODED_RUSTFLAGS
# (flags separated by 0x1f) is used so paths with spaces still work.
CARGO_HOME_DIR="${CARGO_HOME:-${HOME:-}/.cargo}"
REPO_ROOT="$(pwd -P)"
SEP=$'\x1f'
export CARGO_ENCODED_RUSTFLAGS="--cfg${SEP}near${SEP}--remap-path-prefix=${CARGO_HOME_DIR}=/cargo${SEP}--remap-path-prefix=${REPO_ROOT}=/src"
unset RUSTFLAGS
mkdir -p contracts/res

cargo build -p vault --target "$TARGET" --release --target-dir target/wasm
cp "$OUT/vault.wasm" contracts/res/vault.wasm

cargo build -p mock_usdc -p registry -p test_receiver --target "$TARGET" --release --target-dir target/wasm
for c in mock_usdc registry test_receiver; do
  cp "$OUT/$c.wasm" "contracts/res/$c.wasm"
done

# Fail if a machine path is still in any built wasm. A pattern that is empty
# or just "/" (for example HOME=/ in a container) is skipped, since it would
# match any wasm.
patterns=()
for p in "$CARGO_HOME_DIR" "$REPO_ROOT" "${HOME:-}" /home/ /Users/; do
  case "$p" in ""|/|/.cargo) ;; *) patterns+=("$p") ;; esac
done
leak=0
for f in contracts/res/*.wasm; do
  for p in "${patterns[@]}"; do
    if LC_ALL=C grep -a -q -F -- "$p" "$f"; then
      echo "build-wasm: $f contains a local path" >&2
      leak=1
    fi
  done
done
[ "$leak" -eq 0 ] || exit 1

for f in contracts/res/*.wasm; do
  printf '%-40s %8s bytes  sha256 %s\n' "$f" "$(wc -c < "$f")" "$(sha256sum "$f" | cut -d' ' -f1)"
done
