#!/usr/bin/env node
// Used by the sandbox tests (contracts/tests/tests/auction.rs) to run the real
// WASI module and the real bid encryption with a Simulated test secret.
//
//   node sandbox-helper.mjs pubkey
//   node sandbox-helper.mjs seal <pubkey> <auction> <contract> <bidder> <amount>
//   node sandbox-helper.mjs run <file with the request JSON>
//
// The master secret here is a public test value. The real one is generated
// inside the TEE and nobody sees it.
import { readFileSync } from "node:fs";
import { bidPlaintext, sealBid } from "./bid-crypto.mjs";
import { runWasi } from "./run-wasi.mjs";

const WASM = new URL("../../contracts/res/auction_wasi.wasm", import.meta.url).pathname;
const TEST_MASTER = "5eed5eed5eed5eed5eed5eed5eed5eed5eed5eed5eed5eed5eed5eed5eed5eed";

const [cmd, ...a] = process.argv.slice(2);
if (cmd === "pubkey") {
  const { out } = await runWasi(WASM, { mode: "pubkey" }, TEST_MASTER);
  console.log(JSON.parse(out).pubkey);
} else if (cmd === "seal") {
  const [pubkey, auctionId, contract, bidder, amount] = a;
  console.log(sealBid(pubkey, bidPlaintext({ auctionId, contract, bidder, amount })));
} else if (cmd === "run") {
  const { out } = await runWasi(WASM, readFileSync(a[0], "utf8"), TEST_MASTER);
  console.log(out);
} else {
  console.error("usage: pubkey | seal ... | run <file>");
  process.exit(2);
}
