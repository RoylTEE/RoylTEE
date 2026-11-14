#!/usr/bin/env node
// Used by the sandbox tests (contracts/tests/tests/auction.rs) to run the real
// WASI module and the real bid encryption with a Simulated test secret.
//
//   node sandbox-helper.mjs pubkey <contract>
//   node sandbox-helper.mjs seal <pubkey> <auction> <contract> <bidder> <amount>
//   node sandbox-helper.mjs seal-max <pubkey> <auction> <contract> <bidder> <amount>
//       (same, with a long salt so the ciphertext is the largest the
//        contract accepts: 512 base64 characters)
//   node sandbox-helper.mjs run <file with the request JSON> <caller>
//
// <caller> is what OutLayer reports as the account that called it
// (NEAR_PREDECESSOR_ID); the sandbox tests take it from the mock OutLayer.
//
// The master secret here is a public test value. The real one is generated
// inside the TEE and nobody sees it.
import { readFileSync } from "node:fs";
import { bidPlaintext, sealBid } from "./bid-crypto.mjs";
import { runWasi } from "./run-wasi.mjs";

// The sandbox deploys the auction as auction.test.near, so this uses the test
// build of the module (contracts/res/auction_wasi_test.wasm), which differs
// from the shipped one only in the auction account compiled into it.
const WASM = new URL("../../contracts/res/auction_wasi_test.wasm", import.meta.url).pathname;
const TEST_MASTER = "5eed5eed5eed5eed5eed5eed5eed5eed5eed5eed5eed5eed5eed5eed5eed5eed";

const [cmd, ...a] = process.argv.slice(2);
if (cmd === "pubkey") {
  const { out } = await runWasi(WASM, { mode: "pubkey", contract: a[0] }, TEST_MASTER, a[0]);
  const k = JSON.parse(out);
  if (!k.ok) throw new Error(`pubkey request failed: ${out}`);
  console.log(k.pubkey);
} else if (cmd === "seal") {
  const [pubkey, auctionId, contract, bidder, amount] = a;
  console.log(sealBid(pubkey, bidPlaintext({ auctionId, contract, bidder, amount })));
} else if (cmd === "seal-max") {
  const [pubkey, auctionId, contract, bidder, amount] = a;
  const base = bidPlaintext({ auctionId, contract, bidder, amount, salt: "" });
  // raw = 32 + 12 + plaintext + 16 bytes; base64 of 384 bytes is 512 characters.
  const saltLen = 384 - 60 - Buffer.byteLength(base);
  if (saltLen < 0) throw new Error("bidder or contract name too long for a max size bid");
  const ct = sealBid(pubkey, bidPlaintext({ auctionId, contract, bidder, amount, salt: "a".repeat(saltLen) }));
  if (ct.length !== 512) throw new Error(`ciphertext is ${ct.length} characters`);
  console.log(ct);
} else if (cmd === "run") {
  const { out } = await runWasi(WASM, readFileSync(a[0], "utf8"), TEST_MASTER, a[1]);
  console.log(out);
} else {
  console.error("usage: pubkey <contract> | seal ... | run <file> <caller>");
  process.exit(2);
}
