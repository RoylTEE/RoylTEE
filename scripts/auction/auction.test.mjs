// Runs the built WASI module (contracts/res/auction_wasi.wasm) under Node's
// WASI with bids sealed by the demo script. Run `npm run build:wasm` first.
// The master secret below is a test value for this file only (Simulated):
// the real one is generated inside the TEE and nobody sees it.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { existsSync } from "node:fs";
import { test } from "node:test";
import { spawnSync } from "node:child_process";
import { AMOUNT_WIDTH, bidMsg, bidPlaintext, checkBidAmount, isUsablePubkey, sealBid } from "./bid-crypto.mjs";
import { runWasi } from "./run-wasi.mjs";

const WASM = new URL("../../contracts/res/auction_wasi.wasm", import.meta.url).pathname;
const MASTER = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";
// The shipped module serves this account (compiled in as the default).
const CONTRACT = "auction.royltee-dev.testnet";

assert.ok(existsSync(WASM), "build first: npm run build:wasm");

// `run(req)` uses the test secret and the auction contract as the caller (what
// OutLayer reports as NEAR_PREDECESSOR_ID); `run(req, null)` runs with no
// secret; `run(req, MASTER, caller)` picks the caller (`null` leaves it unset).
const run = async (req, master = MASTER, caller = CONTRACT) => {
  const { code, out } = await runWasi(WASM, req, master === null ? undefined : master, caller === null ? undefined : caller);
  assert.equal(code, 0);
  return { out, json: JSON.parse(out) };
};

const PK_REQ = { mode: "pubkey", contract: CONTRACT };
const PUBKEY = (await run(PK_REQ)).json.pubkey;
const pubkey = async () => PUBKEY;

const bid = (pk, auctionId, bidder, amount, over = {}) => ({
  bidder,
  ct: sealBid(pk, bidPlaintext({ auctionId, contract: CONTRACT, bidder, amount, ...over })),
});

const settle = (bids, extra = {}) => ({
  mode: "settle",
  auction_id: "5",
  contract: CONTRACT,
  pubkey: PUBKEY,
  key_version: 1,
  reserve: "10000000",
  max_deposit: "100000000",
  bids,
  ...extra,
});

test("pubkey mode prints a 32 byte key and never the secret", async () => {
  const { out, json } = await run(PK_REQ);
  assert.match(json.pubkey, /^[0-9a-f]{64}$/);
  assert.equal(json.ok, true);
  assert.ok(!out.includes(MASTER));
  assert.equal((await run(PK_REQ)).json.pubkey, json.pubkey, "deterministic");
  assert.notEqual((await run(PK_REQ, MASTER.replace("0", "1"))).json.pubkey, json.pubkey);
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
  const other = (await run(PK_REQ, MASTER.replace("0", "9"))).json.pubkey;
  const { json } = await run(settle([bid(other, 5, "a.testnet", "50000000")]));
  assert.equal(json.winner, null);
});

test("missing secret and bad requests fail with ok=false", async () => {
  assert.equal((await run(PK_REQ, null)).json.ok, false);
  assert.equal((await run("not json")).json.ok, false);
  assert.equal((await run({ mode: "other", contract: CONTRACT })).json.ok, false);
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

test("a direct request from another caller is refused (C1)", async () => {
  const pk = await pubkey();
  const victim = bid(pk, 5, "a.testnet", "50000000");
  const attacker = bid(pk, 5, "x.testnet", "100000000");
  const req = settle([victim, attacker]);
  for (const caller of ["x.testnet", "", null, `${CONTRACT}.evil`]) {
    const { json } = await run(req, MASTER, caller);
    assert.equal(json.ok, false, `caller ${caller}`);
    assert.equal(json.price, undefined);
    assert.equal(json.winner, undefined);
    assert.equal((await run(PK_REQ, MASTER, caller)).json.ok, false);
  }
  assert.equal((await run(req)).json.price, "50000000", "the contract itself is admitted");
});

test("a request naming another contract is refused even from that contract (B-02)", async () => {
  const pk = await pubkey();
  const other = "attacker.testnet";
  const req = settle([bid(pk, 5, "a.testnet", "50000000")], { contract: other });
  assert.equal((await run(req, MASTER, other)).json.ok, false, "attacker names itself");
  assert.equal((await run({ mode: "pubkey", contract: other }, MASTER, other)).json.ok, false);
  // Right caller but a request that names someone else is refused too.
  assert.equal((await run(req, MASTER, CONTRACT)).json.ok, false);
});

test("a key that is not the published one fails explicitly (B-08)", async () => {
  const pk = await pubkey();
  const req = settle([bid(pk, 5, "a.testnet", "50000000")]);
  assert.equal((await run(req)).json.ok, true);
  const wrongKey = await run(req, MASTER.replace("0", "1"));
  assert.equal(wrongKey.json.ok, false);
  assert.match(wrongKey.json.error, /key mismatch/);
  assert.equal((await run({ ...req, key_version: 2 })).json.ok, false);
  assert.equal((await run({ ...req, pubkey: undefined })).json.ok, false);
  assert.equal((await run(PK_REQ)).json.key_version, 1);
});

test("low order public keys are refused by the bid script (B-08)", () => {
  assert.equal(isUsablePubkey(PUBKEY), true);
  for (const k of [
    "0".repeat(64),
    `01${"0".repeat(62)}`,
    "e0eb7a7c3b41b8ae1656e3faf19fc46ada098deb9c32b1fd866205165f49b880",
    `ed${"ff".repeat(30)}ff`,
    "A".repeat(64),
  ]) {
    assert.equal(isUsablePubkey(k), false, k);
    assert.throws(() => sealBid(k, "x"));
  }
});

test("amounts are padded to 38 digits and the module rejects other widths", async () => {
  const pk = await pubkey();
  const p = JSON.parse(bidPlaintext({ auctionId: 5, contract: CONTRACT, bidder: "a.testnet", amount: "50000000" }));
  assert.equal(p.amount.length, AMOUNT_WIDTH);
  assert.equal(p.amount, "50000000".padStart(38, "0"));
  // Ciphertext length is the same for a 1 digit and an 18 digit bid.
  const small = bid(pk, 5, "a.testnet", "1").ct.length;
  const big = bid(pk, 5, "a.testnet", "100000000000000000").ct.length;
  assert.equal(small, big);
  // An unpadded amount sealed by hand is dropped.
  const raw = { ...JSON.parse(bidPlaintext({ auctionId: 5, contract: CONTRACT, bidder: "a.testnet", amount: "50000000" })), amount: "50000000" };
  const { json } = await run(settle([{ bidder: "a.testnet", ct: sealBid(pk, JSON.stringify(raw)) }]));
  assert.equal(json.winner, null);
  assert.throws(() => bidPlaintext({ auctionId: 5, contract: CONTRACT, bidder: "a", amount: "1".repeat(39) }));
});

test("encrypt-bid refuses an amount outside the reserve and max deposit (L5)", () => {
  assert.doesNotThrow(() => checkBidAmount({ amount: "10000000", reserve: "10000000", maxDeposit: "100000000" }));
  assert.doesNotThrow(() => checkBidAmount({ amount: "100000000", reserve: "10000000", maxDeposit: "100000000" }));
  assert.throws(() => checkBidAmount({ amount: "9999999", reserve: "10000000", maxDeposit: "100000000" }), /below the reserve/);
  assert.throws(() => checkBidAmount({ amount: "100000001", reserve: "10000000", maxDeposit: "100000000" }), /above max_deposit/);
  assert.throws(() => checkBidAmount({ amount: "1e8", reserve: "1", maxDeposit: "100000000" }));
  const script = new URL("./encrypt-bid.mjs", import.meta.url).pathname;
  const base = ["--pubkey", "00".repeat(32), "--auction", "0", "--contract", CONTRACT, "--bidder", "a.testnet", "--reserve", "10000000", "--max-deposit", "100000000"];
  const bad = spawnSync("node", [script, ...base, "--amount", "5"], { encoding: "utf8" });
  assert.equal(bad.status, 2);
  assert.match(bad.stderr, /below the reserve/);
  const noRange = spawnSync("node", [script, ...base.slice(0, 8), "--amount", "50000000"], { encoding: "utf8" });
  assert.equal(noRange.status, 2, "reserve and max-deposit are required");
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
