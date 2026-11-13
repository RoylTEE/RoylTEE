/** Shares, Claims: shares with a claimable balance, the total, and where each holder claims it. */
import { allTokens, musdc, short } from './chain';
import { h, badge, fig, shareState, fail, stamp } from './ui';

const target = document.querySelector('[data-claims]')!;
async function load() {
  try {
    const tokens = await allTokens();
    const open = tokens.filter((t) => BigInt(t.extra.claimable) > 0n);
    const total = open.reduce((a, t) => a + BigInt(t.extra.claimable), 0n);
    const paid = tokens.reduce((a, t) => a + BigInt(t.extra.paid_out), 0n);
    document.querySelector('[data-claim-fig]')!.replaceChildren(
      fig('Waiting to be claimed', 'To claim', musdc(total), 'mUSDC'),
      fig('Shares with a balance', 'Shares', `${open.length}/${tokens.length}`),
      fig('Claimed by holders since mint', 'Claimed', musdc(paid), 'mUSDC', '', { wide: true }));
    target.replaceChildren(open.length
      ? h('table', { class: 'ledger rows-sm' },
          h('thead', {}, h('tr', {}, ...['Share', 'Holder', 'Work', 'Status'].map((x) => h('th', { text: x })), h('th', { class: 'num', text: 'Claimable' }), h('th', { text: 'Claim' }))),
          h('tbody', {}, ...open.map((t) => { const st = shareState(t); return h('tr', {},
            h('td', { class: 'p' }, h('a', { class: 'link mono', href: `/share?id=${t.token_id}`, text: t.token_id })),
            h('td', { class: 'p acct', text: short(t.owner_id) }),
            h('td', { class: 'm nowrap', text: `Work ${t.extra.work_id}` }),
            h('td', { class: 'x' }, badge(st.text, st.tone)),
            h('td', { class: 'num p-end', text: `${musdc(t.extra.claimable)} mUSDC` }),
            h('td', { class: 'm' }, h('a', { class: 'link', href: `/member?account=${encodeURIComponent(t.owner_id)}#holdings`, text: 'Claim in Your shares' }))); })))
      : h('div', { class: 'panel-body' }, h('div', { class: 'empty' }, h('p', { text: 'Every share account is claimed down to zero. New money shows up here after the next payment.' }), h('a', { class: 'btn', href: '/activity#payments', text: 'See recorded payments' }))));
    stamp(document.querySelector('[data-stamp]'));
  } catch (err) { fail(target, err, () => void load()); }
}
void load();
