// Runs the built WASI module (contracts/res/auction_wasi.wasm) under Node's
// WASI with bids sealed by the demo script. Run `npm run build:wasm` first.
// The master secret below is a test value for this file only (Simulated):
// the real one is generated inside the TEE and nobody sees it.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { existsSync } from "node:fs";
import { test } from "node:test";
import { bidMsg, bidPlaintext, sealBid } from "./bid-crypto.mjs";
import { runWasi } from "./run-wasi.mjs";

const WASM = new URL("../../contracts/res/auction_wasi.wasm", import.meta.url).pathname;
const MASTER = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";
const CONTRACT = "auction.example.testnet";

assert.ok(existsSync(WASM), "build first: npm run build:wasm");

// `run(req)` uses the test secret; `run(req, null)` runs with none set.
const run = async (req, master = MASTER) => {
  const { code, out } = await runWasi(WASM, req, master === null ? undefined : master);
  assert.equal(code, 0);
  return { out, json: JSON.parse(out) };
};

const pubkey = async () => (await run({ mode: "pubkey" })).json.pubkey;

const bid = (pk, auctionId, bidder, amount, over = {}) => ({
  bidder,
  ct: sealBid(pk, bidPlaintext({ auctionId, contract: CONTRACT, bidder, amount, ...over })),
});

const settle = (bids, extra = {}) => ({
  mode: "settle",
  auction_id: "5",
  contract: CONTRACT,
  reserve: "10000000",
  max_deposit: "100000000",
  bids,
  ...extra,
});

test("pubkey mode prints a 32 byte key and never the secret", async () => {
  const { out, json } = await run({ mode: "pubkey" });
  assert.match(json.pubkey, /^[0-9a-f]{64}$/);
  assert.equal(json.ok, true);
  assert.ok(!out.includes(MASTER));
  assert.equal((await run({ mode: "pubkey" })).json.pubkey, json.pubkey, "deterministic");
  assert.notEqual((await run({ mode: "pubkey" }, MASTER.replace("0", "1"))).json.pubkey, json.pubkey);
});

test("second price: highest bid wins and pays the second highest", async () => {
  const pk = await pubkey();
  const { out, json } = await run(
    settle([
      bid(pk, 5, "a.testnet", "50000000"),
      bid(pk, 5, "b.testnet", "90000000"),
      bid(pk, 5, "c.testnet", "70000000"),
    ]),
  );
  assert.equal(json.winner, "b.testnet");
  assert.equal(json.price, "70000000");
  // Losing bids and the winner's own top bid never appear.
  for (const secret of ["50000000", "90000000"]) assert.ok(!out.includes(secret), secret);
  assert.deepEqual(Object.keys(json).sort(), ["auction", "bids_hash", "ok", "price", "v", "winner"]);
});

test("one valid bid pays the reserve", async () => {
  const pk = await pubkey();
  const { json } = await run(settle([bid(pk, 5, "a.testnet", "55000000")]));
  assert.equal(json.winner, "a.testnet");
  assert.equal(json.price, "10000000");
});

test("a tie goes to the earlier bid at the tied price", async () => {
  const pk = await pubkey();
  const { json } = await run(
    settle([bid(pk, 5, "a.testnet", "60000000"), bid(pk, 5, "b.testnet", "60000000")]),
  );
  assert.equal(json.winner, "a.testnet");
  assert.equal(json.price, "60000000");
});

test("copied ciphertext, wrong auction, wrong contract, bad amounts are dropped", async () => {
  const pk = await pubkey();
  const a = bid(pk, 5, "a.testnet", "40000000");
  const { json } = await run(
    settle([
      a,
      { bidder: "thief.testnet", ct: a.ct },
      bid(pk, 6, "d.testnet", "99000000"),
      bid(pk, 5, "e.testnet", "99000000", { contract: "other.testnet" }),
      bid(pk, 5, "f.testnet", "100000001"),
      bid(pk, 5, "g.testnet", "9999999"),
    ]),
  );
  assert.equal(json.winner, "a.testnet");
  assert.equal(json.price, "10000000");
});

test("no valid bid: no winner", async () => {
  const pk = await pubkey();
  const { json } = await run(settle([bid(pk, 5, "a.testnet", "1")]));
  assert.equal(json.ok, true);
  assert.equal(json.winner, null);
  assert.equal(json.price, null);
});

test("bids sealed to another key do not open", async () => {
  const other = (await run({ mode: "pubkey" }, MASTER.replace("0", "9"))).json.pubkey;
  const { json } = await run(settle([bid(other, 5, "a.testnet", "50000000")]));
  assert.equal(json.winner, null);
});

test("missing secret and bad requests fail with ok=false", async () => {
  assert.equal((await run({ mode: "pubkey" }, null)).json.ok, false);
  assert.equal((await run("not json")).json.ok, false);
  assert.equal((await run({ mode: "other" })).json.ok, false);
});

test("bids_hash is the sha256 of the canonical text the contract also builds", async () => {
  const pk = await pubkey();
  const bids = [bid(pk, 5, "a.testnet", "50000000"), bid(pk, 5, "b.testnet", "60000000")];
  const { json } = await run(settle(bids));
  const text =
    "royltee-auction-bids-v1\n5\n" + bids.map((b) => `${b.bidder}\n${b.ct}\n`).join("");
  assert.equal(json.bids_hash, createHash("sha256").update(text).digest("hex"));
});

test("eight bids settle quickly", async () => {
  const pk = await pubkey();
  const bids = Array.from({ length: 8 }, (_, i) => bid(pk, 5, `bidder${i}.testnet`, String(20_000_000 + i * 1_000_000)));
  const t = Date.now();
  const { json } = await run(settle(bids));
  assert.equal(json.winner, "bidder7.testnet");
  assert.equal(json.price, "26000000");
  assert.ok(Date.now() - t < 5000);
});

test("the ft_transfer_call msg is the JSON the contract parses", () => {
  const msg = JSON.parse(bidMsg(5, "QUJD"));
  assert.deepEqual(msg, { auction_id: "5", ciphertext: "QUJD" });
});

test("a ciphertext fits the contract limit", async () => {
  const pk = await pubkey();
  const b = bid(pk, 123456, "very-long-account-name-for-a-bidder.royltee-dev.testnet", "100000000");
  assert.ok(b.ct.length <= 512, `ciphertext is ${b.ct.length} chars`);
});
