/** Share page: reads ?id=, then fills the Summary, Statement, Account and History tabs. */
import { oneToken } from './chain';
import { SCENES, MINTS, type Scene } from '../data/testnet';
import { holderChain } from './holders';
import { h, badge, tx, acct, stamp, fail } from './ui';
import { short, view } from './chain';
import { walletFields } from './wallet-view';
import { loadShare, shareParts } from './sharecard';

const $ = (s: string) => document.querySelector(s)!;
const id = (new URLSearchParams(location.search).get('id') ?? 's1').trim();
const missing = (text: string) => $('[data-share-head]').replaceChildren(h('div', { class: 'empty' }, h('p', { text }), h('a', { class: 'btn', href: '/shares/', text: 'Pick a share in the ledger' })));

async function load() {
  if (!/^s\d{1,6}$/.test(id)) { missing('That is not a share id. Share ids look like s1 or s2.'); return; }
  try {
    const token = await oneToken(id);
    if (!token) { missing(`There is no share ${id} on the registry.`); return; }
    document.title = document.title.replace(/^Share/, `Share ${id}`);
    $('h1').textContent = `Share ${id}`;
    const crumb = document.querySelector('.crumbs [aria-current]'); if (crumb) crumb.textContent = `Share ${id}`;
    const d = await loadShare(token);
    const p = shareParts(d);
    renderHolders(holderChain(id, d.share.holder_at_mint, token.owner_id));
    void renderWallet();
    $('[data-share-head]').replaceChildren(...p.head);
    $('[data-summary]').replaceChildren(p.summary);
    $('[data-statement]').replaceChildren(p.statement);
    $('[data-account]').replaceChildren(...p.account);
    $('[data-vault-link]').replaceChildren('Share account on the explorer: ', acct(token.extra.vault));
    stamp(document.querySelector('[data-stamp]'));
  } catch (err) { fail($('[data-share-head]'), err, () => void load()); }
}

function renderHolders(c: ReturnType<typeof holderChain>) {
  const when = (at: string | null) => at ? `${at.slice(0, 10)} ${at.slice(11, 16)} UTC` : 'Not in the record';
  $('[data-holders]').replaceChildren(
    h('div', { class: 'table-wrap' }, h('table', { class: 'ledger rows-sm' },
      h('thead', {}, h('tr', {}, ...['#', 'Holder', 'How', 'When', 'Transaction'].map((x) => h('th', { text: x })))),
      h('tbody', {}, ...c.steps.map((s, i) => h('tr', {},
        h('td', { class: 'x nowrap', text: String(i + 1) }),
        h('td', { class: 'p' }, acct(s.holder)),
        h('td', { class: 'm wrap' }, s.from ? `${s.how} ${short(s.from)}` : s.how),
        h('td', { class: 'm nowrap', text: when(s.at) }),
        h('td', { class: 'p-end' }, s.tx ? tx(s.tx) : badge('Not recorded', 'warn'))))))),
    h('div', { class: 'panel-foot' }, c.matches
      ? h('span', {}, badge('Matches the chain', 'ok'), ' The owner on chain now is the last recorded holder.')
      : h('span', {}, badge('Changed after the record', 'warn'), ' The owner on chain now is ', acct(c.liveOwner), '. The newer move is in the share history on the explorer.')));
}

async function renderWallet() {
  const target = $('[data-wallet]');
  try {
    const [raw, meta] = await Promise.all([view<Record<string, unknown>>('nft_token', { token_id: id }), view<Record<string, unknown>>('nft_metadata')]);
    const f = walletFields(raw, meta);
    const pretty = (o: unknown) => h('pre', { class: 'json', tabindex: '0' }, h('code', { text: JSON.stringify(o, null, 2) }));
    target.replaceChildren(
      h('dl', { class: 'kv' }, ...f.flatMap(([k, v, tone]) => [h('dt', { text: k }), h('dd', {}, tone ? badge(v, tone) : v)])),
      h('h3', { class: 'h-sm', text: 'nft_token' }), pretty({ ...raw, metadata: { ...(raw.metadata as object), extra: parseExtra(raw) } }),
      h('p', { class: 'caption', text: 'The extra field is a JSON string on chain; it is shown parsed here.' }),
      h('h3', { class: 'h-sm', text: 'nft_metadata' }), pretty(meta));
  } catch (err) { fail(target, err, () => void renderWallet()); }
}
const parseExtra = (raw: Record<string, unknown>) => { try { return JSON.parse(String((raw.metadata as { extra?: string }).extra)); } catch { return (raw.metadata as { extra?: unknown }).extra; } };

const mint = MINTS[id];
const rows = ((mint ? [{ step: 'Mint', what: `register_share minted ${id} and deployed its keyless share account`, shows: 'The vault is created with no keys', txs: [mint.tx], tokens: [id], result: 'ok', simulated: false, n: 0 }] : []) as (Scene & { n: number })[])
  .concat(SCENES.map((s, i) => ({ ...s, n: i + 1 })).filter((s) => s.tokens.includes(id))).reverse();
$('[data-history]').replaceChildren(rows.length
  ? h('div', { class: 'table-wrap' }, h('table', { class: 'ledger rows-sm' },
      h('thead', {}, h('tr', {}, h('th', { text: 'Step' }), h('th', { text: 'What happened' }), h('th', { text: 'Result' }), h('th', { text: 'Transaction' }))),
      h('tbody', {}, ...rows.map((s) => h('tr', {},
        h('td', { class: 'p nowrap' }, h('strong', { text: s.step }), s.simulated ? h('span', { class: 'badge sim step-sim', text: 'Simulated payer' }) : null, s.agent ? h('span', { class: 'badge info step-sim', text: 'Scripted agent' }) : null),
        h('td', { class: 'p-wrap' }, s.what, h('span', { class: 'small muted only-wide' }, h('br'), s.shows)),
        h('td', { class: 'p-end' }, s.result === 'refused' ? badge('Refused', 'bad') : badge(s.result === 'info' ? 'Recorded' : 'Done', s.result === 'info' ? 'info' : 'ok')),
        h('td', { class: 'm' }, ...s.txs.map(tx)))))))
  : h('div', { class: 'panel-body' }, h('div', { class: 'empty' }, h('p', { text: 'No recorded demo transactions name this share. Its mint is in the account history on the explorer.' }))));

void load();
