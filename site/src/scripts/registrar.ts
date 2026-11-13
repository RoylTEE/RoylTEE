/** Governance, Registrar: the configured registrar, its keys, and member credentials, read live. */
import { view, accessKeys, utc, short } from './chain';
import { REGISTRAR, ROLE_OF, PAYER } from '../data/testnet';
import { h, badge, acct, fail, stamp } from './ui';

type Credential = { status: string; granted_at: string; updated_at: string } | null;
const MEMBERS = ['writer-a.royltee-dev.testnet', 'writer-b.royltee-dev.testnet', 'buyer-pub.royltee-dev.testnet'];

async function load() {
  const box = document.querySelector('[data-registrar]')!;
  try {
    const [config, keys, creds, payer] = await Promise.all([
      view<{ registrar: string; usdc: string }>('get_config'),
      accessKeys(REGISTRAR),
      Promise.all(MEMBERS.map((a) => view<Credential>('get_credential', { account_id: a }))),
      view<boolean>('is_payer', { account_id: PAYER }),
    ]);
    const full = keys.filter((k) => k.access_key.permission === 'FullAccess').length;
    document.querySelector('[data-reg-badge]')!.replaceChildren(config.registrar === REGISTRAR ? badge('Matches the contract config', 'ok') : badge('Differs from the contract config', 'bad'));
    box.replaceChildren(h('dl', { class: 'kv' },
      h('dt', { text: 'Registrar in get_config' }), h('dd', {}, acct(config.registrar)),
      h('dt', { text: 'Payment token' }), h('dd', {}, acct(config.usdc)),
      h('dt', { text: 'Registrar access keys' }), h('dd', { text: `${keys.length} (${full} full access)` }),
      h('dt', { text: 'Payer role' }), h('dd', {}, short(PAYER), ' ', payer ? badge('Payer', 'info') : badge('Not a payer', 'plain'), ' ', badge('Simulated payer', 'sim'))),
      h('p', { class: 'small muted', text: 'The registrar signs with an ordinary account key. That is the trust assumption of this demo, and why each action carries a document hash.' }));
    document.querySelector('[data-members]')!.replaceChildren(h('table', { class: 'ledger rows-sm' },
      h('thead', {}, h('tr', {}, ...['Member', 'Role in the demo', 'Credential', 'Granted'].map((x) => h('th', { text: x })))),
      h('tbody', {}, ...MEMBERS.map((a, i) => { const c = creds[i]; return h('tr', {},
        h('td', { class: 'p' }, acct(a, short(a))),
        h('td', { class: 'm', text: ROLE_OF[a] ?? '' }),
        h('td', { class: 'p-end' }, c ? badge(c.status === 'active' ? 'Active' : c.status, c.status === 'active' ? 'ok' : 'warn') : badge('None', 'plain')),
        h('td', { class: 'm', text: c ? utc(c.granted_at) : 'none' })); }))));
    stamp(document.querySelector('[data-stamp]'));
  } catch (err) { fail(box, err, () => void load()); }
}
const ACCOUNT = /^(?=.{2,64}$)([a-z\d]+[-_])*[a-z\d]+(\.([a-z\d]+[-_])*[a-z\d]+)*$/;
const form = document.querySelector<HTMLFormElement>('[data-member-check]')!;
form.addEventListener('submit', async (e) => {
  e.preventDefault();
  const out = form.querySelector('[data-member-result]')!;
  const a = (form.querySelector<HTMLInputElement>('#mc')!.value || '').trim().toLowerCase();
  if (!ACCOUNT.test(a)) { out.replaceChildren(h('p', { class: 'small', text: 'That is not a NEAR account id.' })); return; }
  out.replaceChildren(h('p', { class: 'loading', text: 'Reading testnet' }));
  try {
    const [c, active] = await Promise.all([view<Credential>('get_credential', { account_id: a }), view<boolean>('is_active_member', { account_id: a })]);
    out.replaceChildren(h('dl', { class: 'kv' },
      h('dt', { text: 'Account' }), h('dd', {}, acct(a)),
      h('dt', { text: 'get_credential' }), h('dd', {}, c ? badge(c.status === 'active' ? 'Active' : c.status, c.status === 'active' ? 'ok' : 'warn') : badge('No credential', 'plain'), c ? ` granted ${utc(c.granted_at)}` : ''),
      h('dt', { text: 'is_active_member' }), h('dd', {}, active ? badge('true', 'ok') : badge('false', 'plain'), active ? ' Can receive a share transfer' : ' Cannot receive a share transfer')));
  } catch (err) { fail(out, err); }
});
void load();
