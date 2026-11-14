/**
 * The recorded sealed-bid auction run (testnet, 2026-11-14 UTC), read from a build-time copy of
 * deployments/auction-testnet.json and deployments/auction-settle-verify.txt (src/data/auction.json,
 * written by scripts/sync-auction.mjs; tests/auction.test.ts fails when the copy is out of date).
 * Everything here is a recorded fact. All mock USDC amounts are Simulated money.
 */
import record from './auction.json';

export const AUCTION = record;
export const AUCTION_ACCOUNT = record.contract.account;
export const EVIDENCE_FILE = record.verifier.evidenceFile;
export const REPO_FILE_BASE = 'https://github.com/RoylTEE/RoylTEE/blob/main/';

/** Mock USDC base units (6 decimals) as a plain number string, for example 70000000 gives "70". */
export function musdc(units: string): string {
  const n = BigInt(units);
  const whole = n / 1_000_000n;
  const frac = (n % 1_000_000n).toString().padStart(6, '0').replace(/0+$/, '');
  return frac ? `${whole}.${frac}` : `${whole}`;
}
/** "2026-11-14T10:38:50Z" or "2026-11-14T10:18:51.696Z" as "2026-11-14 10:38 UTC". */
export function utc(iso: string): string {
  return `${iso.slice(0, 10)} ${iso.slice(11, 16)} UTC`;
}

export type Stage = { key: 'listed' | 'sealed' | 'settled' | 'withdrawn' | 'refused'; title: string; badge: string; lines: string[]; txs: { hash: string; label: string }[] };

const a = record.auction;
const t = record.times;
const first = (k: keyof typeof t) => t[k][0];

export const STAGES: Stage[] = [
  {
    key: 'refused', title: 'Outsider refused, twice', badge: 'Refused',
    lines: [
      `Before any share was listed, an account with no credential asked OutLayer directly to open bids, first naming ${AUCTION_ACCOUNT} as the contract and then naming itself (${utc(t['17g'][0].at)} and ${utc(t['17g'][1].at)}).`,
      'The OutLayer keystore refused both: "Access denied by access condition". No instructions ran and the answer was null. The two transactions succeed because they only queue the request; the refusal is in the answer.',
      `OutLayer requests ${record.outlayerRequests.filter((r) => r.step.startsWith('17g')).map((r) => r.requestId).join(' and ')}.`,
    ],
    txs: t['17g'].map((x, i) => ({ hash: x.hash, label: i === 0 ? 'Outsider names the auction as contract' : 'Outsider names itself as contract' })),
  },
  {
    key: 'listed', title: 'Listed', badge: 'Listed',
    lines: [
      `${a.seller} listed share ${a.token} (work 4, a share NFT with its own vault) with nft_transfer_call. The auction contract now holds the share.`,
      `Reserve ${musdc(a.reserve)} mUSDC, maximum deposit ${musdc(a.maxDeposit)} mUSDC, ends ${utc(a.endsAtUtc)}. Listed ${utc(first('17i').at)}.`,
    ],
    txs: [{ hash: a.listedTx, label: 'Seller lists s10 (auction 0)' }],
  },
  {
    key: 'sealed', title: 'Bids sealed', badge: '2 bids',
    lines: [
      `Two bidders each sent a sealed bid with a ${musdc(a.maxDeposit)} mUSDC deposit (${a.bids.map((b) => b.bidder.split('.')[0]).join(' and ')}). Each amount was encrypted to the TEE public key before it left the bidder, so only ciphertext is on chain and nobody can read it before the end.`,
      'After the sale the contract clears the ciphertext. This page shows no bid amount. The only number the settle published is the price.',
    ],
    txs: a.bids.map((b, i) => ({ hash: b.tx, label: `Sealed bid ${i + 1} by ${b.bidder.split('.')[0]}` })),
  },
  {
    key: 'settled', title: 'Settled in the TEE', badge: 'Sold',
    lines: [
      `After the end time, ${first('17k').signer} called settle (${utc(first('17k').at)}). The contract asked OutLayer to run the audited module inside Intel TDX hardware. OutLayer request ${record.outlayerRequests.find((r) => r.step === '17k')!.requestId}, task ${record.verify.taskId}, executed ${utc(record.verify.executedAt!)}.`,
      `The module opened the bids and printed only the winner and the price: ${a.winner.split('.')[0]} won at ${musdc(a.price)} mUSDC, the second highest bid, not their own. Share ${a.token} moved to ${a.tokenOwnerAfter.split('.')[0]}.`,
      `OutLayer charged ${record.outlayerRequests.find((r) => r.step === '17k')!.chargedNear} NEAR and refunded ${record.outlayerRequests.find((r) => r.step === '17k')!.refundedNear} NEAR of the 0.02 NEAR attached.`,
    ],
    txs: [{ hash: a.settledTx, label: 'Settle through the real OutLayer TEE' }],
  },
  {
    key: 'withdrawn', title: 'Withdrawn', badge: 'Paid',
    lines: [
      `Withdraw ran for the seller and both bidders (${utc(first('17m').at)}). ${a.seller.split('.')[0]} was paid ${musdc(a.price)} mUSDC, the losing bidder got the full ${musdc(a.maxDeposit)} mUSDC deposit back and the winner got ${musdc(String(BigInt(a.maxDeposit) - BigInt(a.price)))} mUSDC change.`,
      `The auction owes nothing afterwards: owed total ${record.accountingAfter.owedTotal}, pending ${record.accountingAfter.pendingTotal}, escrow ${record.accountingAfter.escrowTotal}, open auctions ${record.accountingAfter.openAuctions}.`,
    ],
    txs: [{ hash: a.withdrawnTx, label: 'Withdraw for the three accounts' }],
  },
];

/** The order the timeline shows: the outsider attempts came first in time, then the sale. */
export const TIMELINE: Stage['key'][] = ['refused', 'listed', 'sealed', 'settled', 'withdrawn'];

export const WHAT_TEE = {
  inside: [
    'Opening the sealed bids, which only the TEE can decrypt.',
    'Choosing the winner and the second price from the opened bids, and printing only those two facts.',
    'Proving, through the attestation, that this exact input produced this exact output on genuine Intel TDX hardware running code that OutLayer approved on chain.',
  ],
  outside: [
    'The auction contract. It is an ordinary NEAR contract on testnet. It moves the share and the money, checks the member credential and limits what an answer can do.',
    'The registry, the share vaults and mock USDC. They do not use a TEE.',
    'Fairness. The contract bounds the answer and keeps the money safe, but it cannot prove the bids were opened honestly or the price computed correctly. That still rests on OutLayer as executor.',
    'Recovery. A delivery whose callback is lost waits for the operator, who keeps the auction account full access key on testnet.',
  ],
};
