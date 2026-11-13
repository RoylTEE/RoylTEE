#!/usr/bin/env bash
# Builds every contract to wasm and copies the files to contracts/res/.
# The vault is built first because the registry embeds its code.
# `--cfg near` makes near-sdk use the on-chain host functions (what
# cargo-near sets); plain cargo is enough, no cargo-near install needed.
set -euo pipefail
cd "$(dirname "$0")/.."

TARGET=wasm32-unknown-unknown
OUT=target/wasm/$TARGET/release
export RUSTFLAGS="--cfg near"
mkdir -p contracts/res

cargo build -p vault --target "$TARGET" --release --target-dir target/wasm
cp "$OUT/vault.wasm" contracts/res/vault.wasm

cargo build -p mock_usdc -p registry -p test_receiver --target "$TARGET" --release --target-dir target/wasm
for c in mock_usdc registry test_receiver; do
  cp "$OUT/$c.wasm" "contracts/res/$c.wasm"
done

for f in contracts/res/*.wasm; do
  printf '%-40s %8s bytes  sha256 %s\n' "$f" "$(wc -c < "$f")" "$(sha256sum "$f" | cut -d' ' -f1)"
done
