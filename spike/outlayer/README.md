# OutLayer call-path spike

This directory is a throwaway testnet check, in its own Cargo workspace. It is not a member of the repository's root workspace, and it is not part of the registry, vault or Mock USDC contracts.

It answers one question: can a NEAR testnet contract call `outlayer.testnet` `request_execution`, get the WASI output back in its callback, and have that run attested as a genuine Intel TDX execution? The answer, for this echo, is yes.

It does not implement a sealed-bid auction, bid privacy, or on-chain quote checks. The registry contracts do not use a TEE. The site's help chat TEE label is unchanged: it still only reads what NEAR AI Cloud reports.

## Layout

| Path | What |
|---|---|
| `echo-wasi/` | Tiny WASI Preview 1 command (`no_std`). Reads stdin and prints `{"ok":true,"echo":"<stdin as a JSON string>","len":N}`. Built with the installed `wasm32-unknown-unknown` target; it imports `fd_read`, `fd_write` and `proc_exit` from `wasi_snapshot_preview1` itself. The OutLayer source field uses build target `wasm32-wasip1`. |
| `tee-probe/` | Throwaway contract, near-sdk 5.30.1 (same as the rest of the repo). `probe(input)` forwards the attached NEAR to `request_execution`, then `on_result` stores and logs the result. Views: `last_result`, `get_config`, `balance`. |
| `scripts/build.sh` | Rebuilds both wasm files into `out/`, remaps machine paths, and fails if a local path is left in either file. |
| `scripts/run-echo.mjs` | Runs `out/echo_wasi.wasm` locally under Node's WASI Preview 1. |
| `scripts/views.sh` | Read-only views on `outlayer.testnet`. No keys, no writes. |
| `scripts/fastfs_payload.py` | Builds the FastFS upload payload (Borsh). No keys, no network. |
| `scripts/check-fastfs.sh` | Read-only: fetches the FastFS URL and compares its sha256 to `out/echo_wasi.wasm`. |
| `scripts/check-probe.sh` | Read-only: probe config, last result, transaction logs, attestation record. |
| `out/echo_wasi.wasm` | The module that was uploaded and attested: 4,965 bytes, sha256 `4a5f117f43a25ebe569f7fdab751de9f0ecfd15309944fb16bde2b290b69f859`. |
| `out/tee_probe.wasm` | The contract that was deployed: 184,401 bytes, sha256 `f0b24099cb5e37b7d416eba3988995cfa2b18ebee6d15d16e1be3c466c5bf77e`, NEAR code hash `HCaVhK4CVa7eF6VufsexRckEtsLAyfRcEwtL8HPEUs1o`. |
| `verify.log` | Output of `outlayer-verify` (git `90c27d9`) on the probe transaction. Typographic dashes from the tool were replaced with ASCII hyphens so this file can live in the repo. |
| `verify-bundle.json` | Evidence bundle written by that same run, unchanged, for offline re-checking. |

## Rebuild and test

From this directory, with Rust 1.99.1 and the `wasm32-unknown-unknown` target (the same pin as the repo root `rust-toolchain.toml`):

```
cargo test --workspace --locked
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
bash scripts/build.sh
```

`cargo test` is expected to report 5 passed tests in `echo_wasi` and 14 passed tests in `tee_probe`.

`scripts/build.sh` should reprint the sha256 values in the table above. The checked-in `out/*.wasm` files are the exact bytes that were uploaded and deployed.

Local echo under Node (no chain):

```
printf '{"hello":"tee"}' | node scripts/run-echo.mjs
```

Expected stdout: `{"ok":true,"echo":"{\"hello\":\"tee\"}","len":15}`.

Read-only checks (no keys):

```
scripts/views.sh
scripts/check-fastfs.sh probe.royltee-dev.testnet
scripts/check-probe.sh 7LFZ3MoTENEP8cJfGYAcrgPhJ81es2CZkYDWjpfXzfQ1 royltee-dev.testnet
```

## What ran on testnet

All times below are UTC. Fees are the sum of `tokens_burnt` on every outcome in NEAR RPC `EXPERIMENTAL_tx_status`.

| Step | Transaction | When (UTC) | Fee (NEAR) |
|---|---|---|---|
| Root created `probe.royltee-dev.testnet` with 2.5 NEAR | [`4GEXsjmgjmxEf979z98n9o9hTqHjXBUxgRkuScYjzgUs`](https://testnet.nearblocks.io/txns/4GEXsjmgjmxEf979z98n9o9hTqHjXBUxgRkuScYjzgUs) | 2026-11-13 20:23:23 | 0.0071149895375, paid by `royltee-dev.testnet` |
| FastFS upload of `echo_wasi.wasm`. The call fails out of gas by design; the indexer then serves the file at the URL below. | [`Ht2ebzCBd8NpVAqcLWPRz4bQNoQqeyf2evLksghB4hDi`](https://testnet.nearblocks.io/txns/Ht2ebzCBd8NpVAqcLWPRz4bQNoQqeyf2evLksghB4hDi) | 2026-11-13 20:29:54 | 0.0001450760736569, paid by the probe account |
| Deploy and init of `tee_probe` in one transaction | [`jT8VwctJf5Z1Sr4rmC9hg2qofGxsB7cUVubdpYVi3j5`](https://testnet.nearblocks.io/txns/jT8VwctJf5Z1Sr4rmC9hg2qofGxsB7cUVubdpYVi3j5) | 2026-11-13 20:30:26 | 0.0015145889260714, paid by the probe account |
| `probe({"hello":"tee"})`, signed by `royltee-dev.testnet`, 0.01 NEAR attached | [`7LFZ3MoTENEP8cJfGYAcrgPhJ81es2CZkYDWjpfXzfQ1`](https://testnet.nearblocks.io/txns/7LFZ3MoTENEP8cJfGYAcrgPhJ81es2CZkYDWjpfXzfQ1) | 2026-11-13 20:31:07 | gas 0.0010867809661522, paid by `royltee-dev.testnet`. OutLayer charged 0.0010626 NEAR and refunded 0.0089374 NEAR. |

FastFS URL:

`https://test.fastfs.io/probe.royltee-dev.testnet/outlayer.testnet/4a5f117f43a25ebe569f7fdab751de9f0ecfd15309944fb16bde2b290b69f859.wasm`

The probe account's only key is the full-access public key `ed25519:3vSHK273p4gXTuKBQj5v3iUEm4vHxBdmYkP4MPX3EroF`. This key is not in the earlier key backup archive.

The callback log on the probe transaction was: `tee_probe result probe_id=0 ok=true output={"echo":"{\"hello\":\"tee\"}","len":15,"ok":true} error=none`. (OutLayer returns object keys in that order; the local renderer prints `ok` first. The bytes that were attested are the OutLayer output.)

## Re-verify the attestation

Install `outlayer-verify` from https://github.com/out-layer/outlayer-verify at git `90c27d9` (for example `cargo install --git https://github.com/out-layer/outlayer-verify outlayer-verify`, which is a global install and needs its own yes if you follow this project's rules).

Online, against the transaction. The tool's default API address is retired, so the `--api` flag is required:

```
outlayer-verify tx 7LFZ3MoTENEP8cJfGYAcrgPhJ81es2CZkYDWjpfXzfQ1 --network testnet --api https://testnet-api.outlayer.ai
```

Offline, against the evidence bundle in this directory (no network):

```
outlayer-verify bundle verify-bundle.json --offline
```

The recorded run (see `verify.log`) returned PROVEN on all five checks, for task 97995, executed at 2026-11-13 20:31:17 UTC:

- Authenticity: the quote is genuine Intel TDX. The Intel signature chain is valid and the TCB status is UpToDate.
- Identity: the worker build is on the approved list on `worker.outlayer.testnet`.
- Binding: the quote covers exactly this task.
- Input: `{"hello":"tee"}`, sha256 `409dcbcaf2467885300523cda3cfba1fc4f5a02c11dac1e930aad5202aeb40de`.
- Output: sha256 `69249e97227b6c576a795b2cf000fcba3e804258c1b9386fba0e4f89fdf344c1` (bytes `{"echo":"{\"hello\":\"tee\"}","len":15,"ok":true}`).

## Caveats

- This is a call-path proof only. The sealed-bid auction is not built. Nothing here proves bid privacy, and a NEAR contract cannot verify the TDX quote itself.
- OutLayer's own docs say governance runs on single operator keys today (UNVERIFIED beyond those docs).
- The registry, vault and Mock USDC contracts do not use a TEE and were not changed by this spike.
- The help chat TEE flag still only reads what NEAR AI Cloud reports. That path is unchanged.
