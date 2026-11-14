/**
 * Numbers and cards for the landing page. Every figure is computed from the recorded evidence
 * (src/data/evidence.json, src/data/auction.json, the claims list and the recorded scenes), so the
 * page cannot drift from them. Nothing here is typed in by hand except which scenes to show.
 */
import { CLAIMS, ATTACKS, TXS } from './claims';
import evidence from './evidence.json';
import { AUCTION, utc, musdc } from './auction';
import { SCENES, txUrl, accountUrl, type Scene } from './testnet';

const txList = Object.values(TXS);
const namedTests = new Set([...CLAIMS.flatMap((c) => c.tests), ...ATTACKS.flatMap((a) => a.tests)]);
const checks = AUCTION.verify.checks;

export type Proof = { value: string; label: string; note: string; href: string; cta: string };
export const PROOF: Proof[] = [
  { value: String(txList.length), label: 'testnet transactions recorded', note: `${txList.filter((t) => t.success).length} succeeded. The others are refusals and failed attempts, kept on purpose.`, href: '/activity', cta: 'See the activity' },
  { value: String(evidence.vaults.length), label: 'share vaults on testnet', note: 'One NFT per share, each with its own payout vault.', href: '/shares/accounts/', cta: 'Check the vaults' },
  { value: String(CLAIMS.length), label: 'claims checked against code and evidence', note: 'Each claim lists the code behind it, a test and an on-chain record.', href: '/verify', cta: 'Open the claims' },
  { value: `${checks.filter((c) => c.verdict === 'PASS').length} of ${checks.length}`, label: 'checks of the auction\'s TEE proof passed', note: `An independent checker ran them on the auction settle, ${utc(AUCTION.verify.executedAt)}.`, href: '/auction', cta: 'Read the proof' },
];

export type Latest = { when: string; step: string; what: string; tx: string; txHref: string; simulated: boolean; refused: boolean };
/** Plain one-line captions for the landing cards. The recorded scene text stays on the pages that show detail. */
export function plain(s: Scene): string {
  if (s.step === 'Auction') return /settles/.test(s.what) ? 'A share was sold in a sealed-bid auction. The winner paid the second highest price.' : 'A share was listed for a sealed-bid auction.';
  if (s.step === 'Approve') return 'A pending share was approved by the song\'s registrant, and the money held for it was released.';
  if (s.step === 'Pay') return /pending/.test(s.shows) ? 'A payment was split between two shares. The half for the pending share was held until it was approved.' : 'A payment was split between the shares of a song in one step.';
  if (s.step === 'Register') return 'A song and its shares were registered.';
  if (s.step === 'Claim') return 'A holder claimed the money of its share.';
  if (s.step === 'Sale') return 'A share was sold. Its vault and its money moved to the buyer.';
  if (s.step === 'Refusal') return 'An account that does not hold the share tried to claim it and was refused.';
  if (s.step === 'Agent') return 'A payment agent acted inside its limits.';
  if (s.step === 'Dispute') return 'A claim above 100 percent opened a dispute.';
  if (s.step === 'Resolve') return 'The registrar resolved a dispute and the held money was released.';
  return 'A recorded testnet step.';
}
/** The most recent recorded scenes, newest first, each dated from its first transaction. */
export const LATEST: Latest[] = [...SCENES]
  .filter((s: Scene) => s.txs.length > 0 && TXS[s.txs[0]]?.at)
  .sort((a, b) => (TXS[b.txs[0]].at as string).localeCompare(TXS[a.txs[0]].at as string))
  .slice(0, 4)
  .map((s) => ({
    when: utc(TXS[s.txs[0]].at as string),
    step: s.step,
    what: plain(s),
    tx: s.txs[0],
    txHref: txUrl(s.txs[0]),
    simulated: Boolean(s.simulated) || /Simulated/.test(`${s.what} ${s.shows}`),
    refused: s.result === 'refused',
  }));

export const AUCTION_FACTS = {
  account: AUCTION.contract.account,
  accountHref: accountUrl(AUCTION.contract.account),
  price: musdc(AUCTION.auction.price),
  bids: AUCTION.auction.bids.length,
  settledAt: utc(AUCTION.times['17k'][0].at),
};
