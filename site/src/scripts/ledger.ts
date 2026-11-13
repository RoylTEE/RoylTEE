/** Shares, Ledger: master table plus an inspector on wide screens; on narrow screens a row opens the share page. */
import { allTokens, musdc, pct, short, type Token } from './chain';
import { h, badge, shareState, rightLabel, fail, stamp, csvLink } from './ui';
import { inspect } from './sharecard';

const $ = <T extends Element>(s: string) => document.querySelector<T>(s)!;
const narrow = matchMedia('(max-width: 767px)');

function render(tokens: Token[]) {
  const detail = $('[data-inspector]');
  const url = new URL(location.href);
  const rows = tokens.map((t) => {
    const st = shareState(t);
    const href = `/share?id=${encodeURIComponent(t.token_id)}`;
    const select = (e?: Event) => {
      if (narrow.matches) { if (!(e?.target instanceof HTMLAnchorElement)) location.href = href; return; }
      e?.preventDefault();
      body.querySelectorAll('tr[aria-selected]').forEach((r) => r.setAttribute('aria-selected', 'false'));
      tr.setAttribute('aria-selected', 'true');
      url.searchParams.set('share', t.token_id); history.replaceState(null, '', url);
      void inspect(detail, t, { compact: true });
    };
    const tr = h('tr', { 'data-row': t.token_id, 'aria-selected': 'false', onclick: select },
      h('td', { class: 'p' }, h('a', { class: 'row-btn', href, text: t.token_id, 'aria-label': `Share ${t.token_id}` })),
      h('td', { class: 'm nowrap', text: `Work ${t.extra.work_id}` }),
      h('td', { class: 'p acct', text: short(t.owner_id) }),
      h('td', { class: 'm', text: rightLabel[t.extra.right_type] ?? t.extra.right_type }),
      h('td', { class: 'num p-end', text: pct(t.extra.percent_bps) }),
      h('td', { class: 'm' }, badge(st.text, st.tone)),
      h('td', { class: 'num x', text: musdc(t.extra.claimable) }));
    return tr;
  });
  const body = h('tbody', {}, ...rows);
  $('[data-shares]').replaceChildren(h('table', { class: 'ledger rows-sm' },
    h('thead', {}, h('tr', {}, h('th', { text: 'Share' }), h('th', { text: 'Work' }), h('th', { text: 'Rightholder' }), h('th', { text: 'Role' }), h('th', { class: 'num', text: 'Percent' }), h('th', { text: 'Status' }), h('th', { class: 'num', text: 'Claimable' }))), body));
  $('[data-export]').replaceChildren(csvLink('royltee-shares-testnet.csv', [['share', 'work', 'rightholder', 'right', 'territory', 'basis_points', 'status', 'earned_musdc', 'paid_out_musdc', 'claimable_musdc', 'share_account'],
    ...tokens.map((t) => [t.token_id, t.extra.work_id, t.owner_id, t.extra.right_type, t.extra.territory, t.extra.percent_bps, t.extra.status, musdc(t.extra.earned), musdc(t.extra.paid_out), musdc(t.extra.claimable), t.extra.vault])]));
  if (narrow.matches) return;
  const pick = rows.find((r) => r.dataset.row === url.searchParams.get('share')) ?? rows[0];
  if (pick) { pick.setAttribute('aria-selected', 'true'); void inspect(detail, tokens[rows.indexOf(pick)], { compact: true }); }
}

async function load() {
  try { render(await allTokens()); stamp(document.querySelector('[data-stamp]')); }
  catch (err) { fail($('[data-shares]'), err, () => void load()); }
}
void load();
