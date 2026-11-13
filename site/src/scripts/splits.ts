/** Catalog, Split sheets: per work, every share with rightholder, role and percent, and the total per right and territory. */
import { allTokens, view, pool, approvalOf, pct, short, type Token, type Work } from './chain';
import { h, badge, acct, rightLabel, shareState, provLabel, fail, stamp } from './ui';
import { groupTotals } from './split-rules';

const target = document.querySelector('[data-splits]')!;
const approvalBadge = (a: string) => a === 'not deployed' ? badge('Not on the deployed build', 'plain') : a === 'approved' ? badge('Approved', 'ok') : a === 'pending' ? badge('Waiting for approval', 'warn') : badge('Rejected', 'bad');

function sheet(w: Work, rows: { t: Token; approval: string }[]) {
  const totals = groupTotals(rows.map(({ t }) => t.extra));
  return h('section', { class: 'panel sheet', id: `work-${w.id}`, 'aria-labelledby': `w${w.id}-h` },
    h('div', { class: 'panel-head' }, h('h2', { id: `w${w.id}-h`, class: 'nowrap', text: `Work ${w.id}` }), h('span', { class: 'mono small', text: `${w.code_type.toUpperCase()} ${w.code}` }),
      h('span', { class: 'caption', text: `${provLabel[w.provenance] ?? w.provenance}, registered by ${short(w.registered_by)}` })),
    h('div', { class: 'table-wrap' }, h('table', { class: 'ledger rows-sm' },
      h('thead', {}, h('tr', {}, ...['Share', 'Rightholder', 'Role', 'Basis', 'Split sheet', 'Status'].map((x) => h('th', { text: x })), h('th', { class: 'num', text: 'Percent' }))),
      h('tbody', {}, ...rows.map(({ t, approval }) => { const st = shareState(t, approval); return h('tr', {},
        h('td', { class: 'p' }, h('a', { class: 'link mono', href: `/share?id=${t.token_id}`, text: t.token_id })),
        h('td', { class: 'p' }, acct(t.owner_id, short(t.owner_id))),
        h('td', { class: 'm', text: rightLabel[t.extra.right_type] ?? t.extra.right_type }),
        h('td', { class: 'x', text: t.extra.basis }),
        h('td', { class: approval === 'not deployed' ? 'x' : 'm' }, approvalBadge(approval)),
        h('td', { class: 'm' }, badge(st.text, st.tone)),
        h('td', { class: 'num p-end', text: t.extra.status === 'void' ? `(${pct(t.extra.percent_bps)})` : pct(t.extra.percent_bps) })); })),
      h('tfoot', {}, ...totals.map(({ right: r, territory, bps }) => h('tr', { class: 'sheet-total' },
        h('td', { class: 'p', colspan: '5', text: `${rightLabel[r] ?? r}, ${territory === 'WW' ? 'World' : territory} total` }),
        h('td', { class: 'x' }),
        h('td', { class: 'num p-end' }, h('span', { text: `${pct(bps)} ` }), badge(bps === 10000 ? 'Equals 100%' : bps > 10000 ? 'Over 100%' : `${pct(10000 - bps)} open`, bps === 10000 ? 'ok' : bps > 10000 ? 'bad' : 'warn'))))))),
    h('p', { class: 'panel-foot', text: `${rows.some(({ t }) => t.extra.status === 'void') ? 'A void share is shown in brackets and is not counted in the total. ' : ''}${rows.some((r) => r.approval === 'not deployed') ? 'Split sheet approval is not on the deployed build yet.' : ''}` }));
}

async function load() {
  try {
    const tokens = await allTokens();
    const ids = [...new Set(tokens.map((t) => t.extra.work_id))].sort((a, b) => a - b);
    const [works, approvals] = await Promise.all([pool(ids, (id) => view<Work | null>('get_work', { work_id: id })), pool(tokens, approvalOf)]);
    target.replaceChildren(...works.filter((w): w is Work => !!w).map((w) => sheet(w, tokens.map((t, i) => ({ t, approval: approvals[i] })).filter(({ t }) => t.extra.work_id === w.id))));
    if (location.hash) document.getElementById(location.hash.slice(1))?.scrollIntoView();
    stamp(document.querySelector('[data-stamp]'));
  } catch (err) { fail(target, err, () => void load()); }
}
void load();
