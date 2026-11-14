// Sealed-bid encryption for the RoylTEE share auction. No npm dependencies:
// Node's built-in crypto does X25519, HKDF-SHA256 and ChaCha20-Poly1305.
//
// Wire format (base64 of): ephemeral X25519 public key (32 bytes) || nonce
// (12 bytes) || ChaCha20-Poly1305 ciphertext with its 16 byte tag.
// Key = HKDF-SHA256(salt = ephemeral public || recipient public,
//                   ikm = X25519 shared secret, info = "royltee-auction-bid-v1").
// This matches tee/auction-wasi/src/lib.rs (open_bid). It is the same cipher
// suite OutLayer documents for its own secrets, but it is NOT the eciesjs
// wire format.
import {
  createCipheriv,
  createPublicKey,
  diffieHellman,
  generateKeyPairSync,
  hkdfSync,
  randomBytes,
} from "node:crypto";

const SPKI_X25519_PREFIX = Buffer.from("302a300506032b656e032100", "hex");
const BID_INFO = "royltee-auction-bid-v1";

function publicKeyFromRaw(raw) {
  return createPublicKey({
    key: Buffer.concat([SPKI_X25519_PREFIX, raw]),
    format: "der",
    type: "spki",
  });
}

function rawPublic(keyObject) {
  return Buffer.from(keyObject.export({ format: "jwk" }).x, "base64url");
}

/** Every amount is zero padded to this width, so the ciphertext length does
 * not reveal how many digits the bid has. The module rejects any other width. */
export const AMOUNT_WIDTH = 38;

/** Throws unless reserve <= amount <= maxDeposit (all decimal strings, micro
 * mUSDC). A bid outside the range is silently dropped by the module while its
 * deposit stays locked, so refuse to make one. */
export function checkBidAmount({ amount, reserve, maxDeposit }) {
  for (const [name, v] of [["amount", amount], ["reserve", reserve], ["max deposit", maxDeposit]]) {
    if (!/^[0-9]{1,38}$/.test(String(v))) throw new Error(`${name} must be a decimal string of at most 38 digits`);
  }
  const a = BigInt(amount);
  if (a < BigInt(reserve)) throw new Error(`amount ${amount} is below the reserve ${reserve}; the bid would be dropped`);
  if (a > BigInt(maxDeposit)) throw new Error(`amount ${amount} is above max_deposit ${maxDeposit}; the bid would be dropped`);
}

/** Builds the bid plaintext. Amount is a decimal string in micro mUSDC. */
export function bidPlaintext({ auctionId, contract, bidder, amount, salt }) {
  if (!/^[0-9]{1,38}$/.test(String(amount))) throw new Error("amount must be a decimal string of at most 38 digits");
  return JSON.stringify({
    v: 1,
    auction: String(auctionId),
    contract,
    bidder,
    amount: String(amount).padStart(AMOUNT_WIDTH, "0"),
    salt: salt ?? randomBytes(16).toString("hex"),
  });
}

// X25519 points of small order (zero, one, two of order 8, and the three
// encodings of the field prime and its neighbours). A bid sealed to one of
// them could be opened by anyone, so refuse them. Matches the contract.
const LOW_ORDER = [
  "0000000000000000000000000000000000000000000000000000000000000000",
  "0100000000000000000000000000000000000000000000000000000000000000",
  "e0eb7a7c3b41b8ae1656e3faf19fc46ada098deb9c32b1fd866205165f49b800",
  "5f9c95bca3508c24b1d0b1559c83ef5b04445cc4581c8e86d8224eddd09f1157",
  "ecffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff7f",
  "edffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff7f",
  "eeffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff7f",
];

/** True for a usable X25519 public key: 64 lowercase hex characters, not a low order point. */
export function isUsablePubkey(hex) {
  if (!/^[0-9a-f]{64}$/.test(hex)) return false;
  const top = (parseInt(hex.slice(62), 16) & 0x7f).toString(16).padStart(2, "0");
  return !LOW_ORDER.includes(hex.slice(0, 62) + top);
}

/** Encrypts a bid plaintext to the TEE public key (64 hex characters). */
export function sealBid(pubkeyHex, plaintext) {
  if (!isUsablePubkey(pubkeyHex)) throw new Error("pubkey must be 64 lowercase hex characters and not a low order point");
  const recipient = Buffer.from(pubkeyHex, "hex");
  const eph = generateKeyPairSync("x25519");
  const ephPub = rawPublic(eph.publicKey);
  const shared = diffieHellman({ privateKey: eph.privateKey, publicKey: publicKeyFromRaw(recipient) });
  const key = Buffer.from(
    hkdfSync("sha256", shared, Buffer.concat([ephPub, recipient]), BID_INFO, 32),
  );
  const nonce = randomBytes(12);
  const c = createCipheriv("chacha20-poly1305", key, nonce, { authTagLength: 16 });
  const body = Buffer.concat([c.update(plaintext, "utf8"), c.final(), c.getAuthTag()]);
  return Buffer.concat([ephPub, nonce, body]).toString("base64");
}

/** The `msg` string for mock USDC `ft_transfer_call` to the auction. */
export function bidMsg(auctionId, ciphertext) {
  return JSON.stringify({ auction_id: String(auctionId), ciphertext });
}
