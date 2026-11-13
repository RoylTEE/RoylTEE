#!/usr/bin/env bash
# Read only. Shows the probe contract config, last result, and (given a tx hash)
# the transaction logs and the OutLayer attestation record. No keys.
set -euo pipefail
RPC=${RPC:-https://rpc.testnet.near.org}
C=${C:-probe.royltee-dev.testnet}
view() {
  curl -s -m 20 "$RPC" -H 'content-type: application/json' -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"query\",\"params\":{\"request_type\":\"call_function\",\"finality\":\"final\",\"account_id\":\"$C\",\"method_name\":\"$1\",\"args_base64\":\"e30=\"}}" \
  | python3 -c 'import sys,json; r=json.load(sys.stdin); x=r.get("result",{}); print(bytes(x["result"]).decode() if "result" in x else r.get("error"))'
}
echo "get_config: $(view get_config)"
echo "last_result: $(view last_result)"
if [ -n "${1:-}" ]; then
  TX=$1; SIGNER=${2:-royltee-dev.testnet}
  echo "tx logs:"
  curl -s -m 30 "$RPC" -H 'content-type: application/json' -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"EXPERIMENTAL_tx_status\",\"params\":{\"tx_hash\":\"$TX\",\"sender_account_id\":\"$SIGNER\",\"wait_until\":\"NONE\"}}" \
  | python3 -c 'import sys,json; r=json.load(sys.stdin)["result"]; [print(" ",o["outcome"]["executor_id"],"|"," || ".join(o["outcome"]["logs"])[:400],"|",list(o["outcome"]["status"].keys())[0]) for o in r["receipts_outcome"]]'
  echo "attestation (testnet API):"
  curl -s -m 30 -w '\nhttp=%{http_code}\n' "https://testnet-api.outlayer.ai/attestations/by-tx/$TX" | head -c 3000; echo
fi
