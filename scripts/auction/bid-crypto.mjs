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

/** Builds the bid plaintext. Amount is a decimal string in micro mUSDC. */
export function bidPlaintext({ auctionId, contract, bidder, amount, salt }) {
  if (!/^[0-9]+$/.test(String(amount))) throw new Error("amount must be a decimal string");
  return JSON.stringify({
    v: 1,
    auction: String(auctionId),
    contract,
    bidder,
    amount: String(amount),
    salt: salt ?? randomBytes(16).toString("hex"),
  });
}

/** Encrypts a bid plaintext to the TEE public key (64 hex characters). */
export function sealBid(pubkeyHex, plaintext) {
  if (!/^[0-9a-f]{64}$/.test(pubkeyHex)) throw new Error("pubkey must be 64 lowercase hex characters");
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
