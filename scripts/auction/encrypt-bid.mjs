#!/usr/bin/env node
// Encrypts one sealed bid and prints the ft_transfer_call msg to send.
//
// Usage:
//   node scripts/auction/encrypt-bid.mjs --pubkey <64 hex> --auction 0 \
//     --contract auction.royltee-dev.testnet --bidder buyer-pub.royltee-dev.testnet \
//     --amount 120000000
//
// Prints one JSON line: {"msg": "<ft_transfer_call msg>", "ciphertext": "<base64>"}.
// The amount stays on this machine; only the ciphertext goes on chain.
// Read the public key with:
//   near contract call-function as-read-only auction.royltee-dev.testnet get_tee_pubkey json-args '{}' network-config testnet now
import { bidMsg, bidPlaintext, sealBid } from "./bid-crypto.mjs";

const args = Object.fromEntries(
  process.argv
    .slice(2)
    .reduce((acc, a, i, all) => (a.startsWith("--") ? [...acc, [a.slice(2), all[i + 1]]] : acc), []),
);
for (const k of ["pubkey", "auction", "contract", "bidder", "amount"]) {
  if (!args[k]) {
    console.error(`missing --${k}`);
    process.exit(2);
  }
}
const ciphertext = sealBid(
  args.pubkey,
  bidPlaintext({
    auctionId: args.auction,
    contract: args.contract,
    bidder: args.bidder,
    amount: args.amount,
  }),
);
console.log(JSON.stringify({ msg: bidMsg(args.auction, ciphertext), ciphertext }));
