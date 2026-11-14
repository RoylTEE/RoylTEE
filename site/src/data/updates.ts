/**
 * The project changelog, newest first. Every date is UTC.
 * Chain events take their time from the recorded transaction (evidence.json), so the time cannot drift from the chain.
 * Repository events carry the commit time in UTC, read with `TZ=UTC git log --date=format-local`.
 * Nothing here is an announcement, a partner or a press item: each entry is a build step with a link to its record.
 */
import { TXS } from './claims';
import { AUCTION, REPO_FILE_BASE } from './auction';
import { SOURCE_URL } from '../consts';
import { txUrl } from './testnet';

export type UpdateLink = { label: string; href: string; external?: boolean };
export type UpdateKind = 'Chain' | 'Code' | 'Docs' | 'Site';
export type Update = {
  /** ISO time in UTC, to the minute. A decision row has a date only. */
  at: string;
  dateOnly?: boolean;
  kind: UpdateKind;
  title: string;
  body: string;
  simulated?: boolean;
  links: UpdateLink[];
};

const tx = (label: string, hash: string): UpdateLink => ({ label, href: txUrl(hash), external: true });
const doc = (label: string, file: string): UpdateLink => ({ label, href: REPO_FILE_BASE + file, external: true });
const commit = (hash: string): UpdateLink => ({ label: `Commit ${hash}`, href: `${SOURCE_URL}/commit/${hash}`, external: true });
const at = (hash: string) => (TXS[hash].at as string).slice(0, 16) + 'Z';

const H = {
  usdc: 'APzCzPmVMWStFkwXkJXYX4JQPQqYYghjkWKYR3c6vcmK',
  reg: 'Ckk8dvY7sR2fKpDBEQ1akEVx8QSviSNmJCKkyBRjpXeN',
  mint: '7wxZ7Bmstzu9qTUayrvzbGP97tcD6GRLMaZo7pTVAqzh',
  sale: 'HZ9pD2kLbqbjmKqC68ao6KBTUpQmQstyd2Tro8hWwYPy',
  resolve: '7wvLcnuB5L6w3rh3tBL2Jgg7yTMErD9QduC1XLSAYUVi',
  redeploy: '9oR5awwxiJPouDhpYdTxTquskc8YpoFHdmAJy4SrrAMp',
  audit: 'BGBUJDTnnjpevSUZkdmfeVASxbKHn1FQDAZqj75kH5rU',
  final: '52jp95BVJCk6a9bUGyYtYEQq99R56Xiz5zxJ1vCHBJrD',
  list: 'Ae6ZccUJNUoRZRSPesWyJGCnpSPZ8hs11Ch5WjFANHiR',
  settle: 'CATqXVEauHtQ1mpFHQo7VxQdj9fCwnEQQUFGh9NK5esW',
  sheetReg: 'EMdw5gBJg8LAeuZ96n767iYiV6zyYhAgqmQ1Gf89W2sy',
  approve: '5dgncSm1CyDuiEKRzeUpZJTWrBnHzNuuEP6k51RNseGa',
};
const WITHDRAW = AUCTION.auction.withdrawnTx;

export const UPDATES: Update[] = ([
  { at: '2026-11-12T18:48Z', kind: 'Docs', title: 'Threat model and design written before any code', body: 'The transfer threat model and the design were written first, so each attack had a row before there was a contract to attack.', links: [doc('Threat model', 'THREAT_MODEL.md'), doc('Design', 'DESIGN.md'), commit('281c2c6')] },
  { at: '2026-11-13T03:29Z', kind: 'Code', title: 'Registry, vault and Mock USDC contracts built', body: 'NEP-171, NEP-177 and NEP-181 shares, a vault factory, claim and transfer locks, and payment routing, with tests.', links: [commit('6315db6')] },
  { at: '2026-11-13T04:36Z', kind: 'Docs', title: 'Manual audit pass', body: 'A manual audit of the contracts by the builder. It is not an independent review.', links: [doc('Audit notes', 'AUDIT.md'), commit('053fd3d')] },
  { at: at(H.usdc), kind: 'Chain', title: 'Mock USDC deployed to testnet', body: 'The mock payment token went live as usdc.royltee-dev.testnet. It has no value.', simulated: true, links: [tx('Deploy transaction', H.usdc)] },
  { at: at(H.reg), kind: 'Chain', title: 'Registry deployed to testnet', body: 'The registry went live as reg.royltee-dev.testnet.', links: [tx('Deploy transaction', H.reg)] },
  { at: at(H.mint), kind: 'Chain', title: 'First share minted with a vault that has no key', body: 'Share s1 was registered. Its vault was created in the same batch with no access key, and none has been added since.', links: [tx('Registration transaction', H.mint), { label: 'Share s1', href: '/share?id=s1' }] },
  { at: at(H.sale), kind: 'Chain', title: 'Catalog sale of share s2', body: 'The share moved to another member. The vault address stayed and the unclaimed money went with it.', links: [tx('Sale transaction', H.sale), { label: 'The catalog sale, step by step', href: '/how-it-works#lifecycle' }] },
  { at: at(H.resolve), kind: 'Chain', title: 'A dispute resolved on testnet', body: 'The registrar resolved the work 2 dispute with a document hash, and the held money was released to the existing share.', simulated: true, links: [tx('Resolve transaction', H.resolve), { label: 'Disputes', href: '/how-it-works#disputes' }] },
  { at: '2026-11-13', dateOnly: true, kind: 'Docs', title: 'Decision: registry and Mock USDC keys stay on testnet', body: 'The full access keys of the registry and the Mock USDC accounts stay in place until the owner says they can be deleted, so in-place fixes stay possible. This is decision 68 in the log.', links: [doc('Decisions log', 'DECISIONS_LOG.md'), { label: 'Plan, not done', href: '/security#plan' }] },
  { at: '2026-11-13T09:09Z', kind: 'Code', title: 'Split sheet and escrow built', body: 'A share registered by someone other than the work\'s registrant is held, with its money in escrow, until the registrant approves it.', links: [commit('2653566')] },
  { at: '2026-11-13T10:18Z', kind: 'Docs', title: 'verify:testnet, README and MIT license', body: 'A read-only command that checks the vaults, code hashes and balances against the deploy record, a README written for readers who want to check, and the MIT license.', links: [doc('README', 'README.md'), doc('License', 'LICENSE'), commit('1a336ae')] },
  { at: at(H.redeploy), kind: 'Chain', title: 'Registry redeployed with the contract review fixes', body: 'The registry code was replaced in place after a contract review.', links: [tx('Deploy transaction', H.redeploy)] },
  { at: at(H.audit), kind: 'Chain', title: 'Registry redeployed with an audit fix', body: 'A fix for the gas of the payment forward, found in the audit pass.', links: [tx('Deploy transaction', H.audit)] },
  { at: at(H.final), kind: 'Chain', title: 'Final registry redeploy', body: 'The final contract fixes, deployed with a check in the same transaction that nothing was in flight.', links: [tx('Deploy transaction', H.final)] },
  { at: at(H.list), kind: 'Chain', title: 'Sealed-bid auction run on testnet', body: 'Share s10 was listed at 10:18, settled through the OutLayer TEE at 10:39 at the second price, 70 mUSDC, and the seller was paid at 10:46 (all UTC). A checker passed all 5 attestation checks.', simulated: true, links: [tx('List', H.list), tx('Settle', H.settle), tx('Withdraw', WITHDRAW), { label: 'The auction page', href: '/auction' }] },
  { at: at(H.sheetReg), kind: 'Chain', title: 'Split sheet scene recorded on testnet', body: 'On work 6, one share was approved at once and the other waited in escrow until the work\'s registrant signed it at 10:57 UTC.', simulated: true, links: [tx('Work registered', H.sheetReg), tx('Share approved', H.approve), { label: 'Share s15', href: '/share?id=s15' }] },
  { at: '2026-11-14T12:20Z', kind: 'Site', title: 'Auction page', body: 'A page that shows the sealed-bid run with its transactions and attestation checks.', links: [{ label: 'Auction', href: '/auction' }, commit('fffaa98')] },
  { at: '2026-11-14T12:28Z', kind: 'Docs', title: 'Whitepaper 1.0', body: 'The design, the evidence and the limits in one document, on the site and as a PDF.', links: [{ label: 'Whitepaper', href: '/whitepaper' }, { label: 'PDF', href: '/royltee-whitepaper.pdf' }, commit('c56e8bb')] },
  { at: '2026-11-14T12:58Z', kind: 'Site', title: 'Landing page redesigned as a story', body: 'The home page was rebuilt as a story, with the detail moved to the pages behind it.', links: [{ label: 'Home', href: '/' }, commit('57f27c8')] },
] as Update[]).sort((a, b) => (a.at < b.at ? 1 : a.at > b.at ? -1 : 0));
