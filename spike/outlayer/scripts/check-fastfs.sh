#!/usr/bin/env bash
# Read only: fetches the FastFS URL and compares its sha256 with out/echo_wasi.wasm.
set -euo pipefail
cd "$(dirname "$0")/.."
SENDER=${1:?sender account}
H=$(sha256sum out/echo_wasi.wasm | cut -d' ' -f1)
URL="https://test.fastfs.io/$SENDER/outlayer.testnet/$H.wasm"
tmp=out/fastfs-check.wasm
trap 'rm -f "$tmp"' EXIT
code=$(curl -s -o "$tmp" -w '%{http_code}' -m 30 "$URL")
echo "url=$URL http=$code"
[ "$code" = 200 ] || { echo "not served yet (indexer delay?)"; exit 2; }
G=$(sha256sum "$tmp" | cut -d' ' -f1)
echo "served_sha256=$G expected=$H"
[ "$G" = "$H" ] && echo MATCH || { echo MISMATCH; exit 1; }
