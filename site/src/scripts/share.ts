/** Share page: reads ?id=, then renders the inspector body in full and the recorded history. */
import { oneToken } from './chain';
import { SCENES } from '../data/testnet';
import { h, badge, tx, acct, stamp, fail } from './ui';
import { inspect } from './sharecard';

const target = document.querySelector('[data-share]')!;
const hist = document.querySelector('[data-history]')!;
const id = (new URLSearchParams(location.search).get('id') ?? 's1').trim();

async function load() {
  if (!/^s\d{1,6}$/.test(id)) { target.replaceChildren(h('div', { class: 'empty' }, h('p', { text: 'That is not a share id. Share ids look like s1 or s2.' }), h('a', { class: 'btn', href: '/dashboard/', text: 'Pick a share in the catalog' }))); return; }
  try {
    const token = await oneToken(id);
    if (!token) { target.replaceChildren(h('div', { class: 'empty' }, h('p', { text: `There is no share ${id} on the registry.` }), h('a', { class: 'btn', href: '/dashboard/', text: 'Pick a share in the catalog' }))); return; }
    document.title = `Share ${id} . RoylTEE`;
    await inspect(target, token);
    document.querySelector('[data-vault-link]')!.replaceChildren('Share account on the explorer: ', acct(token.extra.vault));
    stamp(document.querySelector('[data-stamp]'));
  } catch (err) { fail(target, err, () => void load()); }
}

const rows = SCENES.filter((s) => s.tokens.includes(id));
hist.replaceChildren(rows.length
  ? h('div', { class: 'table-wrap' }, h('table', { class: 'ledger' },
      h('thead', {}, h('tr', {}, h('th', { text: 'Step' }), h('th', { text: 'What happened' }), h('th', { text: 'Result' }), h('th', { text: 'Transaction' }))),
      h('tbody', {}, ...rows.map((s) => h('tr', {},
        h('td', {}, h('strong', { text: s.step }), s.simulated ? h('div', {}, badge('Simulated payer', 'sim')) : null),
        h('td', {}, s.what, h('br'), h('span', { class: 'small muted', text: s.shows })),
        h('td', {}, s.result === 'refused' ? badge('Refused', 'bad') : badge(s.result === 'info' ? 'Recorded' : 'Done', s.result === 'info' ? 'info' : 'ok')),
        h('td', {}, ...s.txs.map(tx)))))))
  : h('div', { class: 'panel-body' }, h('div', { class: 'empty' }, h('p', { text: 'No recorded demo transactions name this share. Its mint is in the account history on the explorer.' }))));

void load();
