/** Share page: reads ?id=, then fills the Summary, Statement, Account and History tabs. */
import { oneToken } from './chain';
import { SCENES } from '../data/testnet';
import { h, badge, tx, acct, stamp, fail } from './ui';
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
    const p = shareParts(await loadShare(token));
    $('[data-share-head]').replaceChildren(...p.head);
    $('[data-summary]').replaceChildren(p.summary);
    $('[data-statement]').replaceChildren(p.statement);
    $('[data-account]').replaceChildren(...p.account);
    $('[data-vault-link]').replaceChildren('Share account on the explorer: ', acct(token.extra.vault));
    stamp(document.querySelector('[data-stamp]'));
  } catch (err) { fail($('[data-share-head]'), err, () => void load()); }
}

const rows = SCENES.map((s, i) => ({ ...s, n: i + 1 })).filter((s) => s.tokens.includes(id)).reverse();
$('[data-history]').replaceChildren(rows.length
  ? h('div', { class: 'table-wrap' }, h('table', { class: 'ledger rows-sm' },
      h('thead', {}, h('tr', {}, h('th', { text: 'Step' }), h('th', { text: 'What happened' }), h('th', { text: 'Result' }), h('th', { text: 'Transaction' }))),
      h('tbody', {}, ...rows.map((s) => h('tr', {},
        h('td', { class: 'p nowrap' }, h('strong', { text: s.step }), s.simulated ? h('span', { class: 'badge sim step-sim', text: 'Simulated payer' }) : null),
        h('td', { class: 'p-wrap' }, s.what, h('span', { class: 'small muted only-wide' }, h('br'), s.shows)),
        h('td', { class: 'p-end' }, s.result === 'refused' ? badge('Refused', 'bad') : badge(s.result === 'info' ? 'Recorded' : 'Done', s.result === 'info' ? 'info' : 'ok')),
        h('td', { class: 'm' }, ...s.txs.map(tx)))))))
  : h('div', { class: 'panel-body' }, h('div', { class: 'empty' }, h('p', { text: 'No recorded demo transactions name this share. Its mint is in the account history on the explorer.' }))));

void load();
