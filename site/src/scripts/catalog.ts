/** Catalog, Works and rights: one row per work with composition and recording shares side by side. */
import { allTokens, view, pool, pct, short, type Token, type Work } from './chain';
import { h, badge, acct, rightLabel, provLabel, fail, stamp } from './ui';

const target = document.querySelector('[data-works]')!;

/** Split check for one work: every active right and territory must add up to exactly 100%. */
export function splitCheck(workId: number, tokens: Token[]) {
  const totals = new Map<string, number>();
  for (const t of tokens) if (t.extra.work_id === workId && t.extra.status === 'active') totals.set(`${t.extra.right_type}/${t.extra.territory}`, (totals.get(`${t.extra.right_type}/${t.extra.territory}`) ?? 0) + t.extra.percent_bps);
  const all = [...totals.values()];
  if (!all.length) return badge('No shares yet', 'plain');
  if (all.every((b) => b === 10000)) return badge('Total 100%', 'ok');
  if (all.some((b) => b > 10000)) return badge('Over 100%', 'bad');
  return badge(`${pct(10000 - Math.max(...all))} open`, 'warn');
}

function side(w: Work, tokens: Token[], composition: boolean) {
  const groups = new Map<string, number>();
  for (const t of tokens) if (t.extra.work_id === w.id && t.extra.status === 'active' && (t.extra.right_type === 'master') !== composition) groups.set(t.extra.right_type, (groups.get(t.extra.right_type) ?? 0) + t.extra.percent_bps);
  const label = h('span', { class: 'only-narrow', text: composition ? 'Composition: ' : 'Recording: ' });
  if (!groups.size) return h('td', { class: 'm' }, label, h('span', { class: 'dim', text: 'none' }));
  return h('td', { class: 'm' }, label, ...[...groups].map(([r, bps]) => h('span', { class: 'mono', text: `${rightLabel[r]} ${pct(bps)} ` })));
}

async function load() {
  try {
    const tokens = await allTokens();
    const ids = [...new Set(tokens.map((t) => t.extra.work_id))].sort((a, b) => a - b);
    const works = (await pool(ids, (id) => view<Work | null>('get_work', { work_id: id }))).filter((w): w is Work => !!w);
    target.replaceChildren(h('table', { class: 'ledger rows-sm matrix' },
      h('thead', {}, h('tr', {}, ...['Work', 'Code', 'Provenance', 'Composition', 'Recording', 'Split check', 'Registered by'].map((t) => h('th', { text: t })))),
      h('tbody', {}, ...works.map((w) => h('tr', {},
        h('td', { class: 'p nowrap' }, h('a', { class: 'link', href: `/catalog/splits/#work-${w.id}` }, h('strong', { text: `Work ${w.id}` }))),
        h('td', { class: 'p mono small dim', text: `${w.code_type.toUpperCase()} ${w.code}` }),
        h('td', { class: 'x', text: provLabel[w.provenance] ?? w.provenance }),
        side(w, tokens, true), side(w, tokens, false),
        h('td', { class: 'p-end' }, splitCheck(w.id, tokens)),
        h('td', { class: 'x' }, acct(w.registered_by, short(w.registered_by))))))));
    stamp(document.querySelector('[data-stamp]'));
  } catch (err) { fail(target, err, () => void load()); }
}
void load();
