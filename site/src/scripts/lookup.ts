/** Catalog lookup: search in the browser, then a payee view per right from get_group. */
import { allTokens, view, pool, pct, short, type Token, type Work } from './chain';
import { h, badge, acct, rightLabel, fail, stamp } from './ui';
import { lookup, payees, kindOf } from './lookup-match';

type GroupView = { shares: string[]; active_bps: number; reserved_bps: number; unallocated: string; claimant_bps: number; open_dispute: number | null };
const out = document.querySelector('[data-lookup]')!;
const form = document.querySelector<HTMLFormElement>('[data-lookup-form]')!;
const input = form.querySelector<HTMLInputElement>('#q')!;
let works: Work[] = [], tokens: Token[] = [];

async function payeeTable(w: Work) {
  const groups = await pool(w.groups, async (g) => ({ g, v: await view<GroupView | null>('get_group', { work_id: g.work_id, right_type: g.right_type, territory: g.territory }) }));
  return groups.map(({ g, v }) => {
    const rows = payees(v?.shares ?? [], tokens);
    return h('div', { class: 'stack' },
      h('h3', { class: 'h-sm' }, `${rightLabel[g.right_type] ?? g.right_type}, ${g.territory === 'WW' ? 'World' : g.territory} `, v?.active_bps === 10000 ? badge('Total 100%', 'ok') : badge(`${pct(v?.active_bps ?? 0)} active`, 'warn'), v?.open_dispute != null ? badge(`Dispute ${v.open_dispute} open`, 'bad') : null),
      h('div', { class: 'table-wrap' }, h('table', { class: 'ledger rows-sm' },
        h('thead', {}, h('tr', {}, ...['Share', 'Paid to now', 'Percent', 'Share account', 'Status'].map((x) => h('th', { text: x })))),
        h('tbody', {}, ...rows.map((r) => h('tr', {},
          h('td', { class: 'p' }, h('a', { class: 'link mono', href: `/share?id=${r.token}`, text: r.token })),
          h('td', { class: 'p-wrap' }, acct(r.payee, short(r.payee))),
          h('td', { class: 'p-end num', text: pct(r.bps) }),
          h('td', { class: 'x' }, acct(r.vault, short(r.vault))),
          h('td', { class: 'm' }, r.status !== 'active' ? badge(r.status === 'void' ? 'Void, not paid' : r.status, 'plain') : r.paid ? badge('Paid on each payment', 'ok') : badge('Held in escrow', 'warn'))))))));
  });
}

async function render(q: string) {
  const url = new URL(location.href);
  if (q) url.searchParams.set('q', q); else url.searchParams.delete('q');
  history.replaceState(null, '', url);
  if (!q.trim()) { out.replaceChildren(h('div', { class: 'empty' }, h('p', { text: `${works.length} works and ${tokens.length} shares on the registry. Type a code, a share id or an account above.` }))); return; }
  const hits = lookup(q, works, tokens);
  if (!hits.length) {
    out.replaceChildren(h('div', { class: 'empty' }, h('p', { text: kindOf(q) === 'code' ? `No work on this registry has the code ${q.trim()}.` : `Nothing matches ${q.trim()}.` }), h('a', { class: 'btn', href: '/catalog/', text: 'Browse all works' })));
    return;
  }
  out.replaceChildren(h('p', { class: 'caption', text: `${hits.length} work${hits.length === 1 ? '' : 's'} found` }));
  for (const hit of hits) {
    const panel = h('section', { class: 'panel' },
      h('div', { class: 'panel-head' }, h('h2', {}, h('a', { class: 'link', href: `/catalog/splits/#work-${hit.work.id}`, text: `Work ${hit.work.id}` })), h('span', { class: 'mono small dim', text: `${hit.work.code_type.toUpperCase()} ${hit.work.code}` }), h('span', { class: 'end' }, badge(hit.why, 'info'))),
      h('div', { class: 'panel-body stack' }, h('p', { class: 'loading', text: 'Reading payees' })));
    out.append(panel);
    payeeTable(hit.work).then((t) => panel.querySelector('.panel-body')!.replaceChildren(...t), (err) => fail(panel.querySelector('.panel-body')!, err));
  }
}

form.addEventListener('submit', (e) => { e.preventDefault(); void render(input.value); });

async function load() {
  try {
    tokens = await allTokens();
    const ids = [...new Set(tokens.map((t) => t.extra.work_id))].sort((a, b) => a - b);
    works = (await pool(ids, (id) => view<Work | null>('get_work', { work_id: id }))).filter((w): w is Work => !!w);
    const q = new URLSearchParams(location.search).get('q') ?? '';
    input.value = q;
    stamp(document.querySelector('[data-stamp]'));
    await render(q);
  } catch (err) { fail(out, err, () => void load()); }
}
void load();
