/** Your shares, Organisation agents tab: the four org-signed agent controls. Each call is signed by the organisation wallet. */
import { view, musdc, near, utc, short, type Role } from './chain';
import { sendCalls } from './wallet';
import { REGISTRY, REGISTRAR, TOKEN_DECIMALS } from '../data/testnet';
import { h, badge, acct, fail } from './ui';
import { grantChecks, lineChecks, revokeChecks, withdrawChecks, allOk, parseUnits, grantDeposit, LINE_COST_YOCTO, type Check, type RoleFacts } from './agent-rules';

const TGAS = (n: number) => `${n}000000000000`;
const list = (c: Check[]) => h('ul', { class: 'checklist' }, ...c.map((x) => h('li', {}, badge(x.ok ? 'OK' : 'Blocks', x.ok ? 'ok' : 'bad'), h('span', { text: x.text }))));
const field = (id: string, label: string, input: HTMLElement, hint?: string) => h('div', { class: 'field' }, h('label', { for: id, text: label }), input, hint ? h('p', { class: 'hint', text: hint }) : null);
const inp = (id: string, value = '', extra: Record<string, unknown> = {}) => h('input', { id, value, autocomplete: 'off', spellcheck: 'false', ...extra });

export function renderOrg(box: Element, viewing: string, signer: string, status: HTMLElement) {
  const org = signer || viewing;
  const agentIn = inp('o-agent', org ? `pay-agent.${org}` : '');
  const out = h('div', { class: 'stack' });
  const read = async () => {
    const agent = agentIn.value.trim().toLowerCase();
    out.replaceChildren(h('p', { class: 'loading', text: 'Reading the role' }));
    try {
      const [role, cred, payer, active, orgCred] = await Promise.all([
        view<Role | null>('get_role', { agent }), view<unknown>('get_credential', { account_id: agent }), view<boolean>('is_payer', { account_id: agent }),
        org ? view<boolean>('is_active_member', { account_id: org }) : Promise.resolve(null),
        org ? view<{ status: string } | null>('get_credential', { account_id: org }) : Promise.resolve(null)]);
      paint(agent, role, !!cred, payer, active, orgCred?.status === 'suspended');
    } catch (err) { fail(out, err, () => void read()); }
  };
  const sign = async (label: string, method: string, args: Record<string, unknown>, deposit: string, gas: number) => {
    status.textContent = 'Waiting for the organisation wallet...';
    try { await sendCalls(REGISTRY, [{ method, args, deposit, gas: TGAS(gas) }]); status.textContent = `${label} sent. Reading the role again.`; await read(); }
    catch (err) { status.textContent = `Not sent: ${(err as Error).message || 'cancelled in the wallet'}.`; }
  };
  const signBtn = (text: string, ok: boolean, fn: () => void) => h('div', { class: 'actions' },
    h('button', { class: 'btn primary', type: 'button', text, disabled: !signer || !ok, onclick: fn }),
    !signer ? h('span', { class: 'caption', text: 'Needs the org wallet. Connect it above.' }) : !ok ? h('span', { class: 'caption', text: 'Fix what blocks it first.' }) : null);
  const card = (title: string, method: string, body: Node[]) => h('details', { class: 'org-card' }, h('summary', { class: 'org-card-head' }, h('span', { class: 'h-sm', text: title }), h('span', { class: 'mono small dim', text: method }), h('span', { class: 'end' }, badge('Needs the org wallet', 'warn'))), h('div', { class: 'org-card-body' }, ...body));

  function paint(agent: string, role: Role | null, agentCred: boolean, agentPayer: boolean, orgActive: boolean | null, orgSuspended: boolean) {
    const facts: RoleFacts = role ? { org: role.org, revoked: role.revoked, allowance: role.allowance } : null;
    const now = role
      ? h('dl', { class: 'kv' },
          h('dt', { text: 'Status' }), h('dd', {}, role.revoked ? badge('Revoked', 'bad') : role.expired ? badge('Expired', 'bad') : badge('Active', 'ok')),
          h('dt', { text: 'Organisation' }), h('dd', {}, acct(role.org, short(role.org))),
          h('dt', { text: 'Allowance left' }), h('dd', { class: 'mono', text: `${musdc(role.allowance)} mUSDC` }),
          h('dt', { text: 'Limits' }), h('dd', { class: 'mono', text: `${musdc(role.per_tx_limit)} per payment, ${musdc(role.daily_limit)} a day, ${role.max_tx_per_day} payments a day` }),
          h('dt', { text: 'Expires' }), h('dd', { text: utc(role.expires_at) }),
          h('dt', { text: 'Line credits' }), h('dd', { class: 'mono', text: String(role.line_credits) }))
      : h('p', { class: 'small', text: `${agent} has no role yet.` });

    // grant_role
    const m1 = h('input', { type: 'checkbox', id: 'o-m1', checked: true }); const m2 = h('input', { type: 'checkbox', id: 'o-m2', checked: true });
    const per = inp('o-per', '1', { inputmode: 'decimal' }); const day = inp('o-day', '5', { inputmode: 'decimal' }); const max = inp('o-max', '10', { inputmode: 'numeric' });
    const exp = h('input', { id: 'o-exp', type: 'date', value: new Date(Date.now() + 30 * 864e5).toISOString().slice(0, 10) });
    const lines = inp('o-lines', '10', { inputmode: 'numeric' });
    const gChecks = h('div'); const gAct = h('div');
    const gUpdate = () => {
      const i = { org, orgActive, agent, agentHasCredential: agentCred, agentIsPayer: agentPayer, methods: [m1.checked && 'pay_from_allowance', m2.checked && 'report'].filter(Boolean) as string[], perTx: parseUnits(per.value, TOKEN_DECIMALS), daily: parseUnits(day.value, TOKEN_DECIMALS), maxTx: Number(max.value), expiresMs: Date.parse(`${exp.value}T23:59:59Z`), nowMs: Date.now(), role: facts };
      const c = grantChecks(i); const n = Math.max(0, Math.floor(Number(lines.value) || 0)); const dep = grantDeposit(n);
      gChecks.replaceChildren(list(c), h('p', { class: 'small', text: `Attaches ${near(dep)} NEAR: 0.01 for role storage plus ${n} statement line${n === 1 ? '' : 's'} at 0.001 each. What role storage does not use becomes more line credits.` }));
      gAct.replaceChildren(signBtn('Grant the role', allOk(c), () => void sign('grant_role', 'grant_role', { agent, methods: i.methods, per_tx_limit: String(i.perTx), daily_limit: String(i.daily), max_tx_per_day: i.maxTx, expires_at: String(BigInt(i.expiresMs) * 1_000_000n) }, dep.toString(), 50)));
    };
    for (const e of [m1, m2, per, day, max, exp, lines]) e.addEventListener('input', gUpdate);
    gUpdate();

    // add_line_credits
    const more = inp('o-more', '10', { inputmode: 'numeric' }); const lChecks = h('div'); const lAct = h('div');
    const lUpdate = () => { const n = Number(more.value); const c = lineChecks(org, facts, n); const dep = BigInt(Number.isInteger(n) && n > 0 ? n : 0) * LINE_COST_YOCTO;
      lChecks.replaceChildren(list(c)); lAct.replaceChildren(signBtn(`Add ${Number.isInteger(n) && n > 0 ? n : 0} line credits for ${near(dep)} NEAR`, allOk(c), () => void sign('add_line_credits', 'add_line_credits', { agent }, dep.toString(), 30))); };
    more.addEventListener('input', lUpdate); lUpdate();

    // revoke_role
    const rc = revokeChecks(org, REGISTRAR, facts);

    // withdraw_allowance
    const amt = inp('o-amt', role ? musdc(role.allowance) : '', { inputmode: 'decimal' }); const wChecks = h('div'); const wAct = h('div');
    const wUpdate = () => { const a = parseUnits(amt.value, TOKEN_DECIMALS); const c = withdrawChecks(org, orgSuspended, facts, a);
      wChecks.replaceChildren(list(c)); wAct.replaceChildren(signBtn('Withdraw to the organisation', allOk(c), () => void sign('withdraw_allowance', 'withdraw_allowance', { agent, amount: String(a) }, '1', 100))); };
    amt.addEventListener('input', wUpdate); wUpdate();

    out.replaceChildren(
      h('div', { class: 'org-now' }, h('h3', { class: 'h-sm', text: 'Role now' }), now),
      card('Grant a role with limits', 'grant_role', [
        h('fieldset', { class: 'methods' }, h('legend', { text: 'Methods the agent may call' }), h('label', {}, m1, ' pay_from_allowance'), h('label', {}, m2, ' report')),
        h('div', { class: 'org-grid' }, field('o-per', 'Per payment limit, mUSDC', per), field('o-day', 'Daily limit, mUSDC', day), field('o-max', 'Payments a day', max, `1 to 1000`), field('o-exp', 'Expires (end of day, UTC)', exp), field('o-lines', 'Statement lines to prepay', lines)),
        gChecks, gAct]),
      card('Add statement line credits', 'add_line_credits', [h('p', { class: 'small', text: 'Each statement line the agent pays or reports uses one credit. 0.001 NEAR each.' }), field('o-more', 'Line credits to add', more), lChecks, lAct]),
      card('Revoke the role (kill switch)', 'revoke_role', [h('p', { class: 'small', text: 'Stops the agent at once. Its allowance stays and can still be withdrawn below. Attaches 1 yoctoNEAR.' }), list(rc), signBtn('Revoke now', allOk(rc), () => void sign('revoke_role', 'revoke_role', { agent }, '1', 30))]),
      card('Withdraw unused allowance', 'withdraw_allowance', [h('p', { class: 'small', text: 'Sends Mock USDC from the allowance back to the organisation. Attaches 1 yoctoNEAR.' }), field('o-amt', 'Amount, mUSDC', amt), wChecks, wAct]));
  }

  box.replaceChildren(
    h('p', { class: 'small' }, signer ? 'Checks and signing as ' : 'Read only. Checks show what happens if ', acct(org || 'the organisation'), signer ? ', the connected organisation wallet.' : ' signs; signing needs its wallet connected above.'),
    h('form', { class: 'field', onsubmit: (e: Event) => { e.preventDefault(); void read(); } },
      h('label', { for: 'o-agent', text: 'Agent account' }),
      h('div', { class: 'inline' }, agentIn, h('button', { class: 'btn', type: 'submit', text: 'Read the role' })),
      h('p', { class: 'hint', text: 'An agent must be a direct sub-account of the organisation. Try pay-agent.buyer-pub.royltee-dev.testnet.' })),
    out);
  if (org) void read();
}
