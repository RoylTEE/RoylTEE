/** Disputes page: list_disputes and the escrow totals, read live. */
import { view, allDisputes, musdc, utc, short, type Dispute, type Accounting } from './chain';
import { h, badge, acct, fig, stamp, fail, rightLabel } from './ui';
import { SCENES, txUrl } from '../data/testnet';

const outcomeText: Record<string, string> = { uphold_existing: 'Existing shares upheld, claimant voided', uphold_claimant: 'Claimant upheld with new splits', void_duplicate: 'Duplicate work voided', keep_both: 'Both kept', swap_payable: 'Payable work swapped' };

async function load() {
  try {
    const [list, acc] = await Promise.all([allDisputes(), view<Accounting>('get_accounting')]);
    const open = list.filter((d) => d.status === 'open').length;
    document.querySelector('[data-dfig]')!.replaceChildren(fig('Open', 'Open', String(open)), fig('Resolved', 'Resolved', String(list.length - open)), fig('Held in dispute escrow', 'In escrow', musdc(acc.escrow_total), 'mUSDC'), fig('Unassigned, waiting for the registrar', 'Unassigned', musdc(acc.unallocated_total), 'mUSDC'));
    const target = document.querySelector('[data-disputes]')!;
    if (!list.length) { target.replaceChildren(h('div', { class: 'panel-body' }, h('div', { class: 'empty' }, h('p', { text: 'No disputes have been opened. A dispute opens on its own when a share would push a work past 100 percent, or when a code is registered twice.' })))); return; }
    target.replaceChildren(h('div', { class: 'table-wrap' }, h('table', { class: 'ledger rows-sm' },
      h('thead', {}, h('tr', {}, ...['Dispute', 'Kind', 'Shares', 'Opened', 'Status', 'Outcome', 'Document hash'].map((t) => h('th', { text: t })))),
      h('tbody', {}, ...list.map((d) => {
        const [kind, body] = Object.entries(d.kind)[0] ?? ['unknown', {}];
        const res = typeof d.status === 'object' ? d.status.resolved : null;
        const outcome = res ? (typeof res.outcome === 'string' ? res.outcome : Object.keys(res.outcome)[0]) : '';
        const shares = [...(body.contested ?? []), ...(body.claimant ? [body.claimant] : [])];
        const scene = SCENES.find((s) => s.step === 'Resolve');
        return h('tr', {},
          h('td', { class: 'p nowrap' }, h('strong', { text: `Dispute ${d.id}` })),
          h('td', { class: 'p-wrap' }, kind === 'overallocation' ? `Over 100 percent${body.group ? `, work ${body.group.work_id} ${rightLabel[body.group.right_type] ?? ''}` : ''}` : 'Code registered twice'),
          h('td', { class: 'm mono' }, ...shares.flatMap((s, i) => [i ? ', ' : '', h('a', { class: 'link', href: `/share?id=${s}`, text: s === body.claimant ? `${s} (claimant)` : s })])),
          h('td', { class: 'x' }, utc(d.opened_at), h('br'), h('span', { class: 'small muted' }, 'by ', acct(d.opened_by, short(d.opened_by)))),
          h('td', { class: 'p-end' }, res ? badge('Resolved', 'ok') : d.status === 'open' ? badge('Open', 'bad') : badge(String(d.status), 'plain')),
          h('td', { class: 'm' }, res ? outcomeText[outcome] ?? outcome : 'Waiting for the registrar', res && d.id === 0 && scene ? h('span', { class: 'tx-after' }, ' ', h('a', { class: 'acct-link', href: txUrl(scene.txs[0]), target: '_blank', rel: 'noopener', text: scene.txs[0].slice(0, 8) })) : null),
          h('td', { class: 'x mono small', text: res ? `${res.doc_hash.slice(0, 10)}...` : '' }));
      })))));
    stamp(document.querySelector('[data-stamp]'));
  } catch (err) { fail(document.querySelector('[data-disputes]')!, err, () => void load()); }
}
void load();
