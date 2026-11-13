/** Catalog page: live statement, works and rights matrix, shares master table with an inspector. */
import { allTokens, view, accessKeys, tokenBalance, pool, musdc, pct, short, type Token, type Work, type Ledger, type Accounting } from './chain';
import { h, badge, acct, shareState, rightLabel, provLabel, fail, stamp, csvLink } from './ui';
import { inspect } from './sharecard';

const $ = <T extends Element>(s: string) => document.querySelector<T>(s)!;
const fig = (label: string, value: string, unit = '', note?: Node | string) => h('div', { class: 'figure' }, h('span', { class: 'label', text: label }), h('span', { class: 'value' }, value, unit ? h('small', { text: unit }) : null), note ? h('span', { class: 'note' }, note) : null);

async function load() {
  try {
    const tokens = await allTokens();
    const workIds = [...new Set(tokens.map((t) => t.extra.work_id))].sort((a, b) => a - b);
    const [works, ledgers, keys, balances, acc] = await Promise.all([
      pool(workIds, (id) => view<Work | null>('get_work', { work_id: id })),
      pool(tokens, (t) => view<Ledger>('get_ledger', { token_id: t.token_id })),
      pool(tokens, (t) => accessKeys(t.extra.vault).then((k) => k.length)),
      pool(tokens, (t) => tokenBalance(t.extra.vault)),
      view<Accounting>('get_accounting'),
    ]);
    const sum = (f: (l: Ledger) => string) => ledgers.reduce((a, l) => a + BigInt(f(l)), 0n);
    const routed = sum((l) => l.total_routed_in), earned = sum((l) => l.earned), paid = sum((l) => l.total_paid_out), closing = sum((l) => l.known_balance);
    const direct = earned - routed;
    const onToken = balances.reduce((a, b) => a + BigInt(b), 0n);
    const keyless = keys.filter((k) => k === 0).length;
    $('[data-statement]').replaceChildren(
      fig('Opening balance', '0.00', 'mUSDC', 'Every share account starts empty at mint'),
      fig('Paid in through the registry', `+${musdc(routed)}`, 'mUSDC', 'Split by basis points'),
      fig('Direct deposits picked up', `+${musdc(direct)}`, 'mUSDC', 'Sent straight to a share account, then synced'),
      fig('Claimed by holders', `-${musdc(paid)}`, 'mUSDC'),
      fig('Closing balance', musdc(closing), 'mUSDC', onToken === closing ? h('span', {}, badge('Matches', 'ok'), ' token balances of the share accounts') : h('span', {}, badge('Differs', 'warn'), ` token balances show ${musdc(onToken)}`)),
    );
    $('[data-counts]').replaceChildren(
      fig('Works', String(works.filter(Boolean).length)),
      fig('Share NFTs', String(tokens.length), '', `${tokens.filter((t) => t.extra.status === 'active').length} active`),
      fig('Share accounts with 0 keys', `${keyless} / ${tokens.length}`, '', keyless === tokens.length ? badge('All keyless', 'ok') : badge('Check keys', 'bad')),
      fig('Held in dispute escrow', musdc(acc.escrow_total), 'mUSDC'),
      fig('Agent allowance held', musdc(acc.allowance_total), 'mUSDC', h('a', { class: 'link', href: '/agents', text: 'See agents' })),
    );
    renderWorks(works.filter((w): w is Work => !!w), tokens);
    renderShares(tokens);
    stamp(document.querySelector('[data-stamp]'));
  } catch (err) { fail($('[data-statement]'), err, () => void load()); }
}

function renderWorks(works: Work[], tokens: Token[]) {
  const cell = (w: Work, composition: boolean) => {
    const groups = new Map<string, number>();
    for (const t of tokens) if (t.extra.work_id === w.id && t.extra.status === 'active' && (t.extra.right_type === 'master') !== composition) groups.set(t.extra.right_type, (groups.get(t.extra.right_type) ?? 0) + t.extra.percent_bps);
    if (!groups.size) return h('td', { class: 'dim', text: 'None registered' });
    return h('td', {}, ...[...groups].map(([r, bps]) => badge(`${rightLabel[r]} ${pct(bps)}`, bps === 10000 ? 'ok' : bps > 10000 ? 'bad' : 'warn')));
  };
  const check = (w: Work) => {
    const totals = new Map<string, number>();
    for (const t of tokens) if (t.extra.work_id === w.id && t.extra.status === 'active') totals.set(`${t.extra.right_type}/${t.extra.territory}`, (totals.get(`${t.extra.right_type}/${t.extra.territory}`) ?? 0) + t.extra.percent_bps);
    const all = [...totals.values()];
    if (!all.length) return badge('No shares yet', 'plain');
    if (all.every((b) => b === 10000)) return badge('Total 100%', 'ok');
    if (all.some((b) => b > 10000)) return badge('Over 100%', 'bad');
    return badge(`${pct(10000 - Math.max(...all))} open`, 'warn');
  };
  $('[data-works]').replaceChildren(h('table', { class: 'ledger matrix' },
    h('thead', {}, h('tr', {}, ...['Work', 'Code', 'Provenance', 'Composition', 'Recording', 'Split check', 'Registered by'].map((t) => h('th', { text: t })))),
    h('tbody', {}, ...works.map((w) => h('tr', {},
      h('td', {}, h('strong', { text: `Work ${w.id}` })),
      h('td', { class: 'mono', text: `${w.code_type.toUpperCase()} ${w.code}` }),
      h('td', { text: provLabel[w.provenance] ?? w.provenance }),
      cell(w, true), cell(w, false), h('td', {}, check(w)),
      h('td', {}, acct(w.registered_by, short(w.registered_by))))))));
}

function renderShares(tokens: Token[]) {
  const detail = $('[data-inspector]');
  const rows = tokens.map((t) => {
    const st = shareState(t);
    const select = () => { body.querySelectorAll('tr[aria-selected]').forEach((r) => r.setAttribute('aria-selected', 'false')); tr.setAttribute('aria-selected', 'true'); history.replaceState(null, '', `#${t.token_id}`); void inspect(detail, t, { compact: true }); if (matchMedia('(max-width: 991px)').matches) detail.scrollIntoView({ behavior: 'smooth', block: 'start' }); };
    const tr = h('tr', { 'data-row': t.token_id, 'aria-selected': 'false', onclick: select },
      h('td', {}, h('button', { class: 'row-btn', type: 'button', text: t.token_id, 'aria-label': `Inspect share ${t.token_id}` })),
      h('td', { text: `Work ${t.extra.work_id}` }),
      h('td', { class: 'acct', text: short(t.owner_id) }),
      h('td', { text: rightLabel[t.extra.right_type] ?? t.extra.right_type }),
      h('td', { class: 'num', text: pct(t.extra.percent_bps) }),
      h('td', {}, badge(st.text, st.tone)),
      h('td', { class: 'num', text: musdc(t.extra.claimable) }));
    return tr;
  });
  const body = h('tbody', {}, ...rows);
  $('[data-shares]').replaceChildren(h('table', { class: 'ledger' },
    h('thead', {}, h('tr', {}, h('th', { text: 'Share' }), h('th', { text: 'Work' }), h('th', { text: 'Rightholder' }), h('th', { text: 'Role' }), h('th', { class: 'num', text: 'Percent' }), h('th', { text: 'Status' }), h('th', { class: 'num', text: 'Claimable' }))), body));
  $('[data-export]').replaceChildren(csvLink('royltee-shares-testnet.csv', [['share', 'work', 'rightholder', 'right', 'territory', 'basis_points', 'status', 'earned_musdc', 'paid_out_musdc', 'claimable_musdc', 'share_account'],
    ...tokens.map((t) => [t.token_id, t.extra.work_id, t.owner_id, t.extra.right_type, t.extra.territory, t.extra.percent_bps, t.extra.status, musdc(t.extra.earned), musdc(t.extra.paid_out), musdc(t.extra.claimable), t.extra.vault])]));
  const pick = rows.find((r) => r.dataset.row === location.hash.slice(1)) ?? rows[0];
  if (pick) { pick.setAttribute('aria-selected', 'true'); void inspect(detail, tokens[rows.indexOf(pick)], { compact: true }); }
}

void load();
