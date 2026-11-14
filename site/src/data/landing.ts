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
  { value: String(evidence.vaults.length), label: 'share vaults on testnet', note: 'One NFT per share, each with its own payout account and a recorded code hash.', href: '/shares/accounts/', cta: 'Check the accounts' },
  { value: String(CLAIMS.length), label: 'claims, each tied to code, a test and evidence', note: `${namedTests.size} named tests are cited across the claims and the attack list.`, href: '/verify', cta: 'Open the claims' },
  { value: `${checks.filter((c) => c.verdict === 'PASS').length} of ${checks.length}`, label: 'TEE attestation checks passed', note: `Run with ${AUCTION.verifier.tool} on the sealed-bid auction settle, ${utc(AUCTION.verify.executedAt)}.`, href: '/auction', cta: 'Read the attestation' },
];

export type Latest = { when: string; step: string; what: string; shows: string; tx: string; txHref: string; simulated: boolean; refused: boolean };
/** The most recent recorded scenes, newest first, each dated from its first transaction. */
export const LATEST: Latest[] = [...SCENES]
  .filter((s: Scene) => s.txs.length > 0 && TXS[s.txs[0]]?.at)
  .sort((a, b) => (TXS[b.txs[0]].at as string).localeCompare(TXS[a.txs[0]].at as string))
  .slice(0, 4)
  .map((s) => ({
    when: utc(TXS[s.txs[0]].at as string),
    step: s.step,
    what: s.what,
    shows: s.shows,
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
