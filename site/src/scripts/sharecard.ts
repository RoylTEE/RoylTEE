/** The share inspector: used beside the catalog table and as the body of the share page. */
import { view, viewAccount, accessKeys, tokenBalance, storagePrice, approvalOf, musdc, near, pct, utc, short, type Token, type Work, type Share, type Ledger } from './chain';
import { h, badge, acct, shareState, rightLabel, side, provLabel, tierLabel, loading, fail } from './ui';

let priceCache: Promise<bigint> | undefined;
const price = () => (priceCache ??= storagePrice().catch(() => 10n ** 19n));

export type ShareData = { token: Token; share: Share; ledger: Ledger; work: Work | null; approval: string; keys: number | null; keyList: string[]; nearBal: bigint; storage: number; perByte: bigint; vaultToken: string };

export async function loadShare(token: Token): Promise<ShareData> {
  const [share, ledger, work, approval, acc, keys, vaultToken, perByte] = await Promise.all([
    view<Share>('get_share', { token_id: token.token_id }),
    view<Ledger>('get_ledger', { token_id: token.token_id }),
    view<Work | null>('get_work', { work_id: token.extra.work_id }),
    approvalOf(token),
    viewAccount(token.extra.vault),
    accessKeys(token.extra.vault),
    tokenBalance(token.extra.vault),
    price(),
  ]);
  return { token, share, ledger, work, approval, keys: keys.length, keyList: keys.map((k) => k.public_key), nearBal: BigInt(acc.amount), storage: acc.storage_usage, perByte, vaultToken };
}

const kv = (rows: [string, Node | string][]) => h('dl', { class: 'kv' }, ...rows.flatMap(([k, v]) => [h('dt', { text: k }), h('dd', {}, v)]));
const line = (label: string, value: string, sign = '', strong = false) => h('tr', {}, h('td', {}, strong ? h('strong', { text: label }) : label), h('td', { class: 'num', text: `${sign}${value}` }));

export function statement(l: Ledger) {
  const routed = BigInt(l.total_routed_in), earned = BigInt(l.earned), paid = BigInt(l.total_paid_out), closing = BigInt(l.known_balance);
  const direct = earned - routed > 0n ? earned - routed : 0n;
  const ok = routed + direct - paid === closing;
  return h('div', { class: 'stmt' },
    h('table', { class: 'ledger' },
      h('thead', {}, h('tr', {}, h('th', { text: 'Since mint' }), h('th', { class: 'num', text: 'mUSDC' }))),
      h('tbody', {}, line('Opening balance', '0.00'), line('Paid in through the registry', musdc(routed), '+ '), line('Direct deposits picked up', musdc(direct), '+ '), line('Claimed by the holder', musdc(paid), paid ? '- ' : '')),
      h('tfoot', {}, line('Closing balance', musdc(closing), '', true))),
    ok ? null : h('p', { class: 'caption', text: 'Opening plus activity does not equal the closing balance here; a payment may be settling.' }),
    BigInt(l.escrowed) > 0n ? h('p', { class: 'notice warn' }, badge('Held', 'warn'), `${musdc(l.escrowed)} mUSDC is held in escrow for this share.`) : null);
}

export function minBalance(d: ShareData) {
  const reserve = BigInt(d.storage) * d.perByte; const head = d.nearBal - reserve;
  const tone = head > 0n ? 'ok' : 'bad';
  return h('div', { class: 'stack' },
    h('table', { class: 'ledger' },
      h('thead', {}, h('tr', {}, h('th', { text: 'Share account NEAR' }), h('th', { class: 'num', text: 'NEAR' }))),
      h('tbody', {}, line('Balance', near(d.nearBal)), line(`Storage reserve (${d.storage.toLocaleString('en-US')} bytes)`, near(reserve), '- ')),
      h('tfoot', {}, line('Above the minimum', near(head), '', true))),
    h('p', { class: 'caption' }, badge(head > 0n ? 'Above minimum' : 'Below minimum', tone), ' The account must keep enough NEAR to pay for its own storage. Storage price read live from the protocol config.'));
}

export function keysBlock(d: ShareData) {
  return d.keys === 0
    ? h('p', { class: 'keys-ok' }, badge('0 access keys', 'ok'), ' Nobody can sign for this account. Only the registry contract can move money out, and only to the current holder.')
    : h('p', { class: 'notice bad' }, badge(`${d.keys} access keys`, 'bad'), ` Unexpected: ${d.keyList.join(', ')}`);
}

export function renderShare(target: Element, d: ShareData, opts: { compact?: boolean } = {}) {
  const t = d.token, e = t.extra, st = shareState(t, d.approval);
  const approvalBadge = d.approval === 'not deployed' ? badge('Not on the deployed build', 'plain') : badge(d.approval === 'approved' ? 'Approved' : d.approval === 'pending' ? 'Waiting for approval' : 'Rejected', d.approval === 'approved' ? 'ok' : d.approval === 'pending' ? 'warn' : 'bad');
  target.replaceChildren(...(<(Node | null)[]>[
    h('div', { class: 'insp-head' },
      h('div', {}, h('p', { class: 'eyebrow', text: `Share NFT ${t.token_id}` }), h('h3', { text: d.work ? `Work ${d.work.id}, ${rightLabel[e.right_type] ?? e.right_type} ${pct(e.percent_bps)}` : t.token_id }), d.work ? h('p', { class: 'mono small dim', text: `${d.work.code_type.toUpperCase()} ${d.work.code}` }) : null),
      badge(st.text, st.tone)),
    st.why ? h('p', { class: 'small muted', text: st.why }) : null,
    kv([
      ['Rightholder', acct(t.owner_id)],
      ['Role', `${rightLabel[e.right_type] ?? e.right_type} (${side(e.right_type)})`],
      ['Percent', `${pct(e.percent_bps)} (${e.percent_bps.toLocaleString('en-US')} basis points)`],
      ['Territory', e.territory === 'WW' ? 'World' : e.territory],
      ['Basis', e.basis],
      ['Provenance', provLabel[e.provenance] ?? e.provenance],
      ['Evidence', tierLabel[e.evidence_tier] ?? e.evidence_tier],
      ['Split sheet', approvalBadge],
      ['First holder', short(d.share.holder_at_mint)],
      ['Last transfer effective', utc(d.share.last_effective_from)],
    ]),
    h('h4', { text: 'Statement' }), statement(d.ledger),
    h('h4', { text: 'Share account' }), h('p', {}, acct(e.vault)), keysBlock(d), minBalance(d),
    h('p', { class: 'caption', text: `Mock USDC (testnet) held by the account: ${musdc(d.vaultToken)} mUSDC.` }),
    opts.compact ? h('a', { class: 'btn', href: `/share?id=${encodeURIComponent(t.token_id)}`, text: 'Open the full share page' }) : null,
  ]).filter((n): n is Node => !!n));
}

export async function inspect(target: Element, token: Token, opts: { compact?: boolean } = {}) {
  target.replaceChildren(loading(`Reading share ${token.token_id}`));
  try { renderShare(target, await loadShare(token), opts); }
  catch (err) { fail(target, err, () => void inspect(target, token, opts)); }
}
