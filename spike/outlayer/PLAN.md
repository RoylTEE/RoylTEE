# OutLayer call-path check (completed)

This is the record of the testnet call-path check. How to rebuild, test and re-verify is in [README.md](README.md). The spike lives under `spike/outlayer/` and is not part of the registry.

## Goal

Prove that a NEAR testnet contract can call `outlayer.testnet` `request_execution`, get the WASI output back in its callback, and that the run has a TEE attestation record. This is a call-path check, not an auction.

## Result

Pass, on 2026-11-13 (UTC). `last_result.ok` was true with the echo of `{"hello":"tee"}`. `outlayer-verify` (git `90c27d9`) returned PROVEN on all five checks for transaction `7LFZ3MoTENEP8cJfGYAcrgPhJ81es2CZkYDWjpfXzfQ1`. Details, fees, hashes and reproduce commands are in README.md.

A pass does not prove a sealed-bid auction, bid privacy, or on-chain attestation checks. OutLayer's own docs say it runs on single operator keys today (UNVERIFIED beyond those docs).

## What was built

- `echo-wasi/`: WASI Preview 1 command, Rust, `no_std`. Built with the installed `wasm32-unknown-unknown` target. The OutLayer source field uses build target `wasm32-wasip1`.
- `tee-probe/`: throwaway contract, near-sdk 5.30.1. `probe(input)` payable calls `request_execution` with a WasmUrl source, small limits, `response_format` Json, and `payer_account_id` set to the caller, then `on_result` stores and logs the result.
- Scripts for a remapped wasm build, a local Node WASI run, a FastFS payload, and read-only FastFS, probe and `outlayer.testnet` checks.

## Sources for the argument shapes

- https://app.outlayer.ai/docs/near-integration (parameters, WasmUrl needs an https URL plus sha256 plus build_target, resource_limits must always be passed, FastFS workflow).
- https://github.com/fastnear/near-outlayer at 2d50bfab: `contract/src/lib.rs` (`ExecutionSource`, `ResourceLimits`, `ResponseFormat`, `ExecutionResponse`), `contract/src/execution.rs` (`request_execution`, payment check, `on_execution_response` returns the output JSON or null), `contract/src/projects.rs` (`code_source_error`: hash 64 lowercase hex, url https), `scripts/upload_wasm_fastfs.py` (FastFS payload and the near CLI call), `docs/CLI.md` (FastFS URL form, gas 1 on purpose), `API.md` (`GET /attestations/by-tx/{tx_hash}`).
- https://github.com/out-layer/private-dao-example at b412ab38: `dao-contract/src/lib.rs` (`ext_outlayer` signature, unused-gas weight 1, 10 Tgas callback, Project source JSON).
- https://app.outlayer.ai/docs/trust-verification (dashboard verify, `outlayer-verify tx <near-tx-hash>`, install command).

Whether the deployed `outlayer.testnet` code hash `AsbxRWTQ2mQAazgz1bbAC72oZ1H8UKL33JKwZmG2pjn2` (read on 2026-11-13 20:19 UTC) matches that published source commit is UNVERIFIED. The probe callback handles both the current return shape (guest JSON, or `null` on failure) and the older `{success, result, error}` wrapper.

## Publish path chosen

The echo module was published by a FastFS testnet upload from `probe.royltee-dev.testnet` to `outlayer.testnet` method `__fastdata_fastfs`. The call fails out of gas on purpose; the FastFS indexer reads the arguments and serves the file at `https://test.fastfs.io/<sender>/outlayer.testnet/<sha256>.wasm`. The probe was then initialised with a WasmUrl source pointing at that file, hash `4a5f117f43a25ebe569f7fdab751de9f0ecfd15309944fb16bde2b290b69f859`, build target `wasm32-wasip1`. No OutLayer dashboard project was created.

## Chain writes that were run

Each step was a separate owner yes, signed with the legacy keychain on testnet only. Commands are recorded here as they were run, from this directory, with `network-config testnet` and `sign-with-legacy-keychain send`. Signing was never done from a key printed in a terminal or stored as plaintext in the repo.

1. Create the probe account, 2.5 NEAR, transaction `4GEXsjmgjmxEf979z98n9o9hTqHjXBUxgRkuScYjzgUs`, fee 0.0071149895375 NEAR.
2. FastFS upload of the echo module, transaction `Ht2ebzCBd8NpVAqcLWPRz4bQNoQqeyf2evLksghB4hDi`, fee 0.0001450760736569 NEAR. Expected failed execution (out of gas). Then `scripts/check-fastfs.sh probe.royltee-dev.testnet` printed MATCH.
3. Deploy and init `tee_probe` in one transaction, `jT8VwctJf5Z1Sr4rmC9hg2qofGxsB7cUVubdpYVi3j5`, fee 0.0015145889260714 NEAR. Code hash `HCaVhK4CVa7eF6VufsexRckEtsLAyfRcEwtL8HPEUs1o`.
4. One probe call signed by `royltee-dev.testnet` (so that account is `payer_account_id` and receives the refund), 0.01 NEAR attached, transaction `7LFZ3MoTENEP8cJfGYAcrgPhJ81es2CZkYDWjpfXzfQ1`. OutLayer charged 0.0010626 NEAR and refunded 0.0089374 NEAR. The transaction's own gas fee was 0.0010867809661522 NEAR.
5. Attestation: `outlayer-verify tx 7LFZ3MoTENEP8cJfGYAcrgPhJ81es2CZkYDWjpfXzfQ1 --network testnet --api https://testnet-api.outlayer.ai` returned PROVEN. The `--api https://testnet-api.outlayer.ai` flag is required because the tool's default API address is retired. The evidence bundle is `verify-bundle.json`.

Deleting the probe account was not run, and is not part of this record.
