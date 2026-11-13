/** Your shares: read only lookup, or a connected wallet that can claim, approve, reject and register shares. */
import { allTokens, tokensFor, view, approvalOf, pool, musdc, pct, short, type Token, type Work, type Ledger } from './chain';
import { connectWallet, disconnectWallet, sendCalls, type RegistryCall } from './wallet';
import { REGISTRY } from '../data/testnet';
import { h, badge, acct, shareState, rightLabel, loading, fail, stamp } from './ui';
import { statement } from './sharecard';

const root = document.querySelector<HTMLElement>('[data-member]')!;
const q = <T extends Element>(s: string) => root.querySelector<T>(s)!;
const status = q<HTMLElement>('[data-status]');
const TGAS = (n: number) => `${n}000000000000`;
const ACCOUNT = /^(?=.{2,64}$)([a-z\d]+[-_])*[a-z\d]+(\.([a-z\d]+[-_])*[a-z\d]+)*$/;
let viewing = ''; let signer = '';

function setAccount(account: string, wallet: boolean) {
  viewing = account; signer = wallet ? account : '';
  q('[data-who-badge]').replaceChildren(wallet ? badge('Wallet connected', 'ok') : badge('Read only', 'info'));
  q<HTMLButtonElement>('[data-disconnect]').hidden = !wallet;
  status.replaceChildren(wallet ? 'Signed in as ' : 'Viewing ', acct(account), wallet ? '. Actions open your wallet to sign.' : '. Connect that account\'s wallet to act.');
  history.replaceState(null, '', `?account=${encodeURIComponent(account)}`);
  void refresh();
}

async function refresh() {
  const held = q('[data-held]'); const ap = q('[data-approvals]');
  held.replaceChildren(h('div', { class: 'panel-body' }, loading()));
  ap.replaceChildren(h('div', { class: 'panel-body' }, loading()));
  try {
    const [mine, all] = await Promise.all([tokensFor(viewing), allTokens()]);
    renderHeld(mine, await pool(mine, (t) => view<Ledger>('get_ledger', { token_id: t.token_id })));
    await renderApprovals(all);
    stamp(document.querySelector('[data-stamp]'));
  } catch (err) { fail(held, err, () => void refresh()); ap.replaceChildren(); }
}

function renderHeld(tokens: Token[], ledgers: Ledger[]) {
  const held = q('[data-held]');
  if (!tokens.length) { held.replaceChildren(h('div', { class: 'panel-body' }, h('div', { class: 'empty' }, h('p', { text: `${viewing} holds no shares. A share arrives when a member registers one for this account, or when a holder transfers one to it.` }), h('a', { class: 'btn', href: '/shares/', text: 'See who holds what in the ledger' })))); return; }
  const total = tokens.reduce((a, t) => a + BigInt(t.extra.claimable), 0n);
  held.replaceChildren(
    h('div', { class: 'table-wrap' }, h('table', { class: 'ledger rows-sm' },
      h('thead', {}, h('tr', {}, ...['Share', 'Work', 'Role', 'Percent', 'Status', 'Paid in', 'Claimed', 'Claimable', ''].map((t, i) => h('th', { class: i >= 3 && i <= 7 && i !== 4 ? 'num' : undefined, text: t })))),
      h('tbody', {}, ...tokens.map((t, i) => {
        const st = shareState(t); const l = ledgers[i];
        const can = !!signer && signer === t.owner_id && BigInt(t.extra.claimable) > 0n && t.extra.status === 'active' && !t.extra.payout_in_flight && !t.extra.transfer_pending;
        const why = !signer ? 'Connect this account to claim' : BigInt(t.extra.claimable) === 0n ? 'Nothing to claim' : can ? '' : 'Not claimable right now';
        const btn = h('button', { class: can ? 'btn sm primary' : 'btn sm', type: 'button', text: 'Claim', disabled: !can, title: why || undefined, 'aria-label': `Claim share ${t.token_id}`, onclick: () => void claim(t) });
        return h('tr', {},
          h('td', { class: 'p' }, h('a', { class: 'row-btn', href: `/share?id=${t.token_id}`, text: t.token_id })),
          h('td', { class: 'm nowrap', text: `Work ${t.extra.work_id}` }), h('td', { class: 'x', text: rightLabel[t.extra.right_type] ?? t.extra.right_type }),
          h('td', { class: 'num m', text: pct(t.extra.percent_bps) }), h('td', { class: 'm' }, badge(st.text, st.tone)),
          h('td', { class: 'num x', text: musdc(BigInt(l.earned)) }), h('td', { class: 'num x', text: musdc(l.total_paid_out) }), h('td', { class: 'num p-end', text: musdc(t.extra.claimable) }),
          h('td', { class: 'm claim-cell' }, btn, why && !can ? h('div', { class: 'caption only-wide', text: why }) : null));
      })),
      h('tfoot', {}, h('tr', {}, h('td', { class: 'p', colspan: '7', text: 'Total claimable' }), h('td', { class: 'num p-end', text: musdc(total) }), h('td', { class: 'x' }))))),
    h('div', { class: 'panel-body' }, h('details', {}, h('summary', { class: 'small', text: 'Statement for each share' }), ...tokens.map((t, i) => h('div', { class: 'stack' }, h('h4', { text: t.token_id }), statement(ledgers[i]))))));
}

async function claim(t: Token) {
  status.textContent = `Waiting for your wallet to sign claim(${t.token_id})...`;
  try { await sendCalls(REGISTRY, [{ method: 'claim', args: { token_id: t.token_id }, deposit: '1', gas: TGAS(100) }]); status.textContent = `Claim for ${t.token_id} sent. Refreshing.`; await refresh(); }
  catch (err) { status.textContent = `Claim not sent: ${(err as Error).message || 'cancelled in the wallet'}.`; }
}

async function renderApprovals(all: Token[]) {
  const ap = q('[data-approvals]');
  const workIds = [...new Set(all.map((t) => t.extra.work_id))];
  const works = (await pool(workIds, (id) => view<Work | null>('get_work', { work_id: id }))).filter((w): w is Work => !!w);
  const mineWorks = new Set(works.filter((w) => w.registered_by === viewing).map((w) => w.id));
  const candidates = all.filter((t) => mineWorks.has(t.extra.work_id) && t.extra.status === 'active');
  const approvals = await pool(candidates, (t) => approvalOf(t));
  if (approvals.includes('not deployed')) {
    ap.replaceChildren(h('div', { class: 'panel-body' }, h('div', { class: 'notice' }, badge('Not live yet', 'info'), h('p', { text: 'The registry deployed on testnet predates split sheet approval. This list fills in after the registry update. Until then every share counts as approved.' }))));
    return;
  }
  const pending = candidates.filter((_, i) => approvals[i] === 'pending');
  if (!mineWorks.size) { ap.replaceChildren(h('div', { class: 'panel-body' }, h('div', { class: 'empty' }, h('p', { text: `${short(viewing)} has not registered any works, so no split sheet waits for its signature.` })))); return; }
  if (!pending.length) { ap.replaceChildren(h('div', { class: 'panel-body' }, h('div', { class: 'empty' }, h('p', { text: 'Nothing waits for a signature. When another member registers a share on one of these works, it appears here for review.' }), h('a', { class: 'btn', href: '#draft-h', text: 'Draft a split sheet' })))); return; }
  ap.replaceChildren(h('div', { class: 'panel-body' }, ...pending.map((t) => reviewCard(t, all))));
}

function sheetTable(rows: { holder: string; role: string; bps: number; state: Node }[], total: number) {
  return h('div', { class: 'table-wrap' }, h('table', { class: 'ledger rows-sm' },
    h('thead', {}, h('tr', {}, h('th', { text: 'Rightholder' }), h('th', { text: 'Role' }), h('th', { text: 'Status' }), h('th', { class: 'num', text: 'Percent' }))),
    h('tbody', {}, ...rows.map((r) => h('tr', {}, h('td', { class: 'p acct', text: short(r.holder) }), h('td', { class: 'm', text: r.role }), h('td', { class: 'm' }, r.state), h('td', { class: 'num p-end', text: pct(r.bps) })))),
    h('tfoot', {}, h('tr', {}, h('td', { class: 'p', colspan: '3' }, totalLine(total)), h('td', { class: 'num p-end', text: pct(total) })))));
}
const totalLine = (bps: number) => h('span', { class: 'total' }, 'Total', bps === 10000 ? badge('Equals 100%', 'ok') : bps > 10000 ? badge(`Over by ${pct(bps - 10000)}`, 'bad') : badge(`${pct(10000 - bps)} not assigned`, 'warn'));

function reviewCard(t: Token, all: Token[]) {
  const group = all.filter((x) => x.extra.work_id === t.extra.work_id && x.extra.right_type === t.extra.right_type && x.extra.territory === t.extra.territory && x.extra.status === 'active');
  const after = group.reduce((a, x) => a + x.extra.percent_bps, 0);
  const rows = group.map((x) => ({ holder: x.owner_id, role: rightLabel[x.extra.right_type], bps: x.extra.percent_bps, state: x.token_id === t.token_id ? badge('Waiting for approval', 'warn') : badge('Approved', 'ok') }));
  const act = (method: 'approve_share' | 'reject_share') => async () => {
    status.textContent = `Waiting for your wallet to sign ${method}(${t.token_id})...`;
    try { await sendCalls(REGISTRY, [{ method, args: { token_id: t.token_id, doc_hash: null }, deposit: '1', gas: TGAS(100) }]); status.textContent = `${method} for ${t.token_id} sent. Refreshing.`; await refresh(); }
    catch (err) { status.textContent = `Not sent: ${(err as Error).message || 'cancelled in the wallet'}.`; }
  };
  const can = signer === viewing;
  return h('div', { class: 'review' },
    h('div', { class: 'insp-head' }, h('div', {}, h('p', { class: 'eyebrow', text: `Share ${t.token_id}, registered for ${short(t.owner_id)}` }), h('h3', { text: `Work ${t.extra.work_id}, ${rightLabel[t.extra.right_type]}, ${t.extra.territory === 'WW' ? 'World' : t.extra.territory}` })), badge('Review', 'warn')),
    h('p', { class: 'small muted', text: 'If you approve, the money held for this share moves into its share account and it is paid like any other. If you reject, the share is void and its held money waits for the registrar.' }),
    sheetTable(rows, after),
    h('div', { class: 'actions' },
      h('button', { class: 'btn primary', type: 'button', text: 'Approve and sign', disabled: !can, onclick: act('approve_share') }),
      h('button', { class: 'btn danger', type: 'button', text: 'Reject and sign', disabled: !can, onclick: act('reject_share') }),
      can ? null : h('span', { class: 'caption', text: 'Connect this account to sign.' })));
}

/* Draft editor: rightholder, role, basis and percent per row, a live total per role, then a review step. */
type Row = { holder: string; role: string; basis: string; percent: string };
async function draft() {
  const box = q('[data-draft]');
  try {
    const all = await allTokens();
    const ids = [...new Set(all.map((t) => t.extra.work_id))].sort((a, b) => a - b);
    const works = (await pool(ids, (id) => view<Work | null>('get_work', { work_id: id }))).filter((w): w is Work => !!w && w.status === 'active');
    if (!works.length) { box.replaceChildren(h('div', { class: 'empty' }, h('p', { text: 'There are no active works yet. Register the work first with register_work; the README shows the command.' }))); return; }
    let rows: Row[] = [{ holder: '', role: 'publishing', basis: 'copyright', percent: '' }];
    const work = h('select', { id: 'd-work' }, ...works.map((w) => h('option', { value: String(w.id), text: `Work ${w.id}, ${w.code_type.toUpperCase()} ${w.code}` })));
    const terr = h('input', { id: 'd-terr', value: 'WW', maxLength: 2, autocomplete: 'off' });
    const evidence = h('input', { id: 'd-ev', placeholder: 'For example: signed split sheet, 2026-11-13', autocomplete: 'off' });
    const body = h('tbody'); const totals = h('div', { class: 'stack' }); const review = h('div');
    const existing = (role: string) => all.filter((t) => t.extra.work_id === Number(work.value) && t.extra.right_type === role && t.extra.territory === terr.value.toUpperCase() && t.extra.status === 'active').reduce((a, t) => a + t.extra.percent_bps, 0);
    const bpsOf = (r: Row) => Math.round(Number(r.percent) * 100);
    const paint = () => {
      body.replaceChildren(...rows.map((r, i) => h('tr', {},
        h('td', {}, h('label', { class: 'sr', for: `d-h-${i}`, text: `Rightholder ${i + 1}` }), h('input', { id: `d-h-${i}`, value: r.holder, placeholder: 'account.testnet', spellcheck: false, autocomplete: 'off', oninput: (e: Event) => { r.holder = (e.target as HTMLInputElement).value.trim(); sums(); } })),
        h('td', {}, h('label', { class: 'sr', for: `d-r-${i}`, text: `Role ${i + 1}` }), sel(`d-r-${i}`, ['publishing', 'performance', 'mechanical', 'master'], r.role, (v) => { r.role = v; sums(); })),
        h('td', {}, h('label', { class: 'sr', for: `d-b-${i}`, text: `Basis ${i + 1}` }), sel(`d-b-${i}`, ['copyright', 'contract', 'none'], r.basis, (v) => { r.basis = v; })),
        h('td', { class: 'num' }, h('label', { class: 'sr', for: `d-p-${i}`, text: `Percent ${i + 1}` }), h('input', { id: `d-p-${i}`, type: 'number', min: '0.01', max: '100', step: '0.01', value: r.percent, inputMode: 'decimal', oninput: (e: Event) => { r.percent = (e.target as HTMLInputElement).value; sums(); } })),
        h('td', {}, h('button', { class: 'btn sm quiet', type: 'button', text: 'Remove', 'aria-label': `Remove row ${i + 1}`, disabled: rows.length === 1, onclick: () => { rows = rows.filter((_, k) => k !== i); paint(); } })))));
      sums();
    };
    const sums = () => {
      review.replaceChildren();
      const roles = [...new Set(rows.map((r) => r.role))];
      totals.replaceChildren(...roles.map((role) => {
        const have = existing(role); const add = rows.filter((r) => r.role === role).reduce((a, r) => a + (bpsOf(r) || 0), 0);
        return h('p', { class: 'small' }, totalLine(have + add), ` ${rightLabel[role]}: ${pct(have)} already registered plus ${pct(add)} in this draft.`);
      }));
    };
    const doReview = async () => {
      const problems: string[] = [];
      for (const r of rows) {
        if (!ACCOUNT.test(r.holder)) problems.push(`"${r.holder || 'empty'}" is not a NEAR account id.`);
        const b = bpsOf(r); if (!(b >= 1 && b <= 10000)) problems.push(`Percent for ${r.holder || 'a row'} must be between 0.01 and 100.`);
      }
      if (!/^(WW|[A-Z]{2})$/.test(terr.value.toUpperCase())) problems.push('Territory must be WW or a two letter country code.');
      for (const role of new Set(rows.map((r) => r.role))) { const t = existing(role) + rows.filter((r) => r.role === role).reduce((a, r) => a + bpsOf(r), 0); if (t > 10000) problems.push(`${rightLabel[role]} would add up to ${pct(t)}. Fix the split first; above 100 percent opens a dispute.`); }
      if (!evidence.value.trim()) problems.push('Add an evidence reference. Only its SHA-256 hash goes on chain.');
      if (problems.length) { review.replaceChildren(h('div', { class: 'notice bad', role: 'alert' }, badge('Fix first', 'bad'), h('ul', {}, ...problems.map((p) => h('li', { text: p }))))); return; }
      review.replaceChildren(loading('Checking each rightholder has an active credential'));
      const creds = await pool(rows, (r) => view<{ status: string } | null>('get_credential', { account_id: r.holder }).catch(() => null));
      const digest = Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256', new TextEncoder().encode(evidence.value.trim())))).map((b) => b.toString(16).padStart(2, '0')).join('');
      const w = works.find((x) => x.id === Number(work.value))!;
      const calls: RegistryCall[] = rows.map((r) => ({ method: 'register_share', deposit: '200000000000000000000000', gas: TGAS(300), args: { work_id: w.id, right_type: r.role, territory: terr.value.toUpperCase(), percent_bps: bpsOf(r), basis: r.basis, holder: r.holder, evidence_hash: digest } }));
      const blocked = creds.some((c) => !c || c.status !== 'active');
      review.replaceChildren(h('div', { class: 'review' },
        h('div', { class: 'insp-head' }, h('h3', { text: 'Review before you sign' }), badge(blocked ? 'Cannot send' : 'Ready to sign', blocked ? 'bad' : 'ok')),
        sheetTable(rows.map((r, i) => ({ holder: r.holder, role: rightLabel[r.role], bps: bpsOf(r), state: creds[i]?.status === 'active' ? badge('Member', 'ok') : badge('No active credential', 'bad') })), rows.reduce((a, r) => a + bpsOf(r), 0)),
        h('p', { class: 'small muted', text: signer && signer !== w.registered_by ? `You did not register work ${w.id}, so each new share waits until ${short(w.registered_by)} approves it.` : `${rows.length} transaction${rows.length > 1 ? 's' : ''} to ${REGISTRY}, 0.2 NEAR attached to each. Evidence hash ${digest.slice(0, 12)}...` }),
        h('div', { class: 'actions' },
          h('button', { class: 'btn primary', type: 'button', text: `Sign ${rows.length} in your wallet`, disabled: blocked || !signer, onclick: async () => { status.textContent = 'Waiting for your wallet...'; try { await sendCalls(REGISTRY, calls); status.textContent = 'Sent. New shares appear in the catalog once minted.'; } catch (err) { status.textContent = `Not sent: ${(err as Error).message || 'cancelled in the wallet'}.`; } } }),
          h('button', { class: 'btn', type: 'button', text: 'Back to editing', onclick: () => review.replaceChildren() }),
          !signer ? h('span', { class: 'caption', text: 'Connect a wallet to sign.' }) : null)));
    };
    work.addEventListener('change', sums); terr.addEventListener('input', sums);
    box.replaceChildren(
      h('div', { class: 'draft-meta' }, h('div', { class: 'field' }, h('label', { for: 'd-work', text: 'Work' }), work), h('div', { class: 'field' }, h('label', { for: 'd-terr', text: 'Territory' }), terr), h('div', { class: 'field' }, h('label', { for: 'd-ev', text: 'Evidence reference' }), evidence)),
      h('div', { class: 'table-wrap' }, h('table', { class: 'ledger draft-rows' }, h('thead', {}, h('tr', {}, h('th', { text: 'Rightholder' }), h('th', { text: 'Role' }), h('th', { text: 'Basis' }), h('th', { class: 'num', text: 'Percent' }), h('th', {}, h('span', { class: 'sr', text: 'Remove' })))), body)),
      h('div', { class: 'actions' }, h('button', { class: 'btn', type: 'button', text: 'Add a rightholder', onclick: () => { rows.push({ holder: '', role: rows[rows.length - 1].role, basis: 'copyright', percent: '' }); paint(); } }), h('button', { class: 'btn primary', type: 'button', text: 'Review', onclick: () => void doReview() })),
      totals, review);
    paint();
  } catch (err) { fail(box, err, () => void draft()); }
}
const sel = (id: string, opts: string[], value: string, on: (v: string) => void) => h('select', { id, onchange: (e: Event) => on((e.target as HTMLSelectElement).value) }, ...opts.map((o) => h('option', { value: o, text: rightLabel[o] ?? o, selected: o === value })));

q('[data-lookup]').addEventListener('submit', (e) => { e.preventDefault(); const v = (q<HTMLInputElement>('#acct-in').value || '').trim().toLowerCase(); if (!ACCOUNT.test(v)) { status.textContent = 'That is not a NEAR account id.'; return; } setAccount(v, false); });
q('[data-connect]').addEventListener('click', async () => { status.textContent = 'Waiting for your wallet...'; try { setAccount(await connectWallet(), true); } catch (err) { status.textContent = `Not connected: ${(err as Error).message || 'cancelled'}.`; } });
q('[data-disconnect]').addEventListener('click', async () => { await disconnectWallet(); setAccount(viewing, false); });
const start = new URLSearchParams(location.search).get('account');
if (start && ACCOUNT.test(start)) { q<HTMLInputElement>('#acct-in').value = start; setAccount(start, false); }
void draft();
