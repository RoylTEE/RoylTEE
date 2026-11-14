/** Overview: the registry statement since mint, totals, and what needs attention. View calls only. */
import { allTokens, allDisputes, oneToken, short, pct, view, accessKeys, tokenBalance, pool, approvalOf, musdc, type Token, type Ledger, type Accounting, type Dispute, type Role } from './chain';
import { AGENTS } from '../data/testnet';
import { SPLIT_SCENE } from '../data/scene-labels';
import { h, badge, fig, fail, stamp, type Tone } from './ui';

const $ = <T extends Element>(s: string) => document.querySelector<T>(s)!;
const todo = (href: string, title: string, detail: string, chip: string, tone: Tone) =>
  h('li', {}, h('a', { href }, h('span', { class: 't', text: title }), h('span', { class: 'd', text: detail }), badge(chip, tone)));

async function load() {
  try {
    const tokens = await allTokens();
    const [ledgers, keys, balances, acc, disputes, roles, approval] = await Promise.all([
      pool(tokens, (t) => view<Ledger>('get_ledger', { token_id: t.token_id })),
      pool(tokens, (t) => accessKeys(t.extra.vault).then((k) => k.length)),
      pool(tokens, (t) => tokenBalance(t.extra.vault)),
      view<Accounting>('get_accounting'),
      allDisputes(),
      pool([...AGENTS], (a) => view<Role | null>('get_role', { agent: a })),
      tokens[0] ? approvalOf(tokens[0]) : Promise.resolve('not deployed'),
    ]);
    const sum = (f: (l: Ledger) => string) => ledgers.reduce((a, l) => a + BigInt(f(l)), 0n);
    const routed = sum((l) => l.total_routed_in), earned = sum((l) => l.earned), paid = sum((l) => l.total_paid_out), closing = sum((l) => l.known_balance);
    const onToken = balances.reduce((a, b) => a + BigInt(b), 0n);
    const keyless = keys.filter((k) => k === 0).length;
    const works = new Set(tokens.map((t) => t.extra.work_id)).size;
    $('[data-statement]').replaceChildren(
      fig('Opening balance', 'Opening', '0.00', 'mUSDC', 'Every share account starts empty at mint'),
      fig('Paid in through the registry', 'Paid in', `+${musdc(routed)}`, 'mUSDC', 'Split by basis points'),
      fig('Direct deposits picked up', 'Direct', `+${musdc(earned - routed)}`, 'mUSDC', 'Sent straight to a share account, then synced'),
      fig('Claimed by holders', 'Claimed', `-${musdc(paid)}`, 'mUSDC'),
      fig('Closing balance', 'Closing', musdc(closing), 'mUSDC', onToken === closing ? h('span', {}, badge('Matches', 'ok'), ' token balances of the share accounts') : h('span', {}, badge('Differs', 'warn'), ` token balances show ${musdc(onToken)}`), { wide: true, keep: true }),
    );
    $('[data-counts]').replaceChildren(
      fig('Works', 'Works', String(works)),
      fig('Share NFTs', 'Shares', String(tokens.length), '', `${tokens.filter((t) => t.extra.status === 'active').length} active`),
      fig('Share accounts with 0 keys', 'Keyless accounts', `${keyless}/${tokens.length}`, '', keyless === tokens.length ? badge('All keyless', 'ok') : badge('Check keys', 'bad'), { keep: true }),
      fig('Held in dispute escrow', 'In escrow', musdc(acc.escrow_total), 'mUSDC'),
      fig('Agent allowance held', 'Agent allowance', musdc(acc.allowance_total), 'mUSDC', '', { wide: true }),
    );
    const claimable = tokens.filter((t: Token) => BigInt(t.extra.claimable) > 0n);
    const toClaim = claimable.reduce((a, t) => a + BigInt(t.extra.claimable), 0n);
    const open = disputes.filter((d) => d.status === 'open').length;
    const active = roles.filter((r) => r && !r.revoked && !r.expired).length;
    $('[data-attention]').replaceChildren(
      todo('/shares/claims/', claimable.length ? `${claimable.length} share${claimable.length === 1 ? '' : 's'} with money to claim` : 'Nothing to claim', claimable.length ? `${musdc(toClaim)} mUSDC waits for its holders` : 'No share has a balance its holder can claim right now', claimable.length ? 'Ready to claim' : 'Clear', claimable.length ? 'ok' : 'plain'),
      todo('/disputes', open ? `${open} open dispute${open === 1 ? '' : 's'}` : 'No open disputes', `${disputes.length} recorded in total; the money in escrow is ${musdc(acc.escrow_total)} mUSDC`, open ? 'Frozen' : 'Clear', open ? 'warn' : 'plain'),
      todo('/catalog/splits/', 'Split sheet approval', approval === 'not deployed' ? 'Not on the deployed build yet; a registry update is planned' : 'Shares waiting for the work registrant are listed per work', approval === 'not deployed' ? 'Not live yet' : 'Live', approval === 'not deployed' ? 'plain' : 'info'),
      todo('/agents', `${active} active payment agent${active === 1 ? '' : 's'}`, 'Capped allowance, scoped key, cannot pick the payee', 'Scripted agent', 'plain'),
    );
    stamp(document.querySelector('[data-stamp]'));
  } catch (err) { fail($('[data-statement]'), err, () => void load()); }
}
/** The split sheet scene on work 6, read live: both shares, their approval, vault balance and held escrow. */
async function loadSplit() {
  const body = document.querySelector('[data-split-rows]');
  if (!body) return;
  try {
    const rows = await Promise.all(SPLIT_SCENE.shares.map(async (id) => {
      const t = await oneToken(id);
      if (!t) return h('tr', {}, h('td', { class: 'm', text: id }), h('td', { colspan: 5, text: 'Not found on the registry' }));
      const approval = await approvalOf(t);
      const tone: Tone = approval === 'approved' ? 'ok' : approval === 'pending' ? 'warn' : 'plain';
      return h('tr', {}, h('td', { class: 'm' }, h('a', { class: 'link', href: `/share?id=${id}`, text: id })), h('td', { class: 'm', text: short(t.owner_id) }), h('td', { text: pct(t.extra.percent_bps) }), h('td', {}, badge(approval === 'not deployed' ? 'Not on this build' : approval, tone)), h('td', { class: 'num', text: t.extra.vault_balance ? musdc(t.extra.vault_balance) : 'Unreadable' }), h('td', { class: 'num', text: musdc(t.extra.escrowed || '0') }));
    }));
    body.replaceChildren(...rows);
  } catch (err) { fail(body, err, () => void loadSplit()); }
}
void load();
void loadSplit();
