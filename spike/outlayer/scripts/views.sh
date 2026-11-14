#!/usr/bin/env bash
# Read-only RPC views on outlayer.testnet. No keys, no writes.
set -euo pipefail
RPC=${RPC:-https://rpc.testnet.near.org}
view() { # method json-args
  local a; a=$(printf '%s' "$2" | base64 -w0)
  curl -s -m 20 "$RPC" -H 'content-type: application/json' -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"query\",\"params\":{\"request_type\":\"call_function\",\"finality\":\"final\",\"account_id\":\"outlayer.testnet\",\"method_name\":\"$1\",\"args_base64\":\"$a\"}}" \
  | python3 -c 'import sys,json; r=json.load(sys.stdin); x=r.get("result",{}); print(bytes(x["result"]).decode() if "result" in x else r)'
}
echo "code_hash: $(curl -s -m 20 "$RPC" -H 'content-type: application/json' -d '{"jsonrpc":"2.0","id":1,"method":"query","params":{"request_type":"view_account","finality":"final","account_id":"outlayer.testnet"}}' | python3 -c 'import sys,json; print(json.load(sys.stdin)["result"]["code_hash"])')"
echo "is_paused: $(view is_paused '{}')"
echo "get_pricing: $(view get_pricing '{}')"
echo "get_max_limits: $(view get_max_limits '{}')"
echo "estimate spike limits, 15 byte input: $(view estimate_execution_cost '{"resource_limits":{"max_instructions":100000000,"max_memory_mb":64,"max_execution_seconds":10},"input_len":15}')"
echo "estimate default limits: $(view estimate_execution_cost '{"resource_limits":null,"input_len":15}')"
echo "estimate 1024 byte input: $(view estimate_execution_cost '{"resource_limits":{"max_instructions":100000000,"max_memory_mb":64,"max_execution_seconds":10},"input_len":1024}')"
