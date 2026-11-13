/** Agents page: each known agent's role and its access keys, read live. */
import { view, accessKeys, musdc, utc, short, type Role } from './chain';
import { AGENTS, REGISTRY } from '../data/testnet';
import { h, badge, acct, stamp, fail } from './ui';

const target = document.querySelector('[data-agents]')!;
const meter = (label: string, used: bigint, cap: bigint, fmt: (v: bigint) => string) => {
  const ratio = cap > 0n ? Number((used * 1000n) / cap) / 10 : 0;
  return h('div', { class: 'meter' },
    h('div', { class: 'meter-top' }, h('span', { text: label }), h('span', { class: 'num', text: `${fmt(used)} of ${fmt(cap)}` })),
    h('div', { class: 'meter-bar', role: 'img', 'aria-label': `${ratio.toFixed(0)} percent used` }, h('span', { style: `width:${Math.min(100, ratio)}%` })));
};

async function card(agent: string) {
  const [role, keys] = await Promise.all([view<Role | null>('get_role', { agent }), accessKeys(agent)]);
  if (!role) return h('div', { class: 'empty' }, h('p', { text: `${agent} has no role on the registry.` }));
  const state = role.revoked ? badge('Revoked', 'bad') : role.expired ? badge('Expired', 'bad') : badge('Active', 'ok');
  const fc = keys.filter((k) => k.access_key.permission !== 'FullAccess');
  const full = keys.length - fc.length;
  return h('div', { class: 'agent-grid' },
    h('section', { class: 'panel' },
      h('div', { class: 'panel-head' }, h('h2', { text: 'Payment agent' }), state, h('span', { class: 'end' }, badge('Scripted agent', 'plain'))),
      h('div', { class: 'panel-body' },
        h('dl', { class: 'kv' },
          h('dt', { text: 'Agent account' }), h('dd', {}, acct(agent)),
          h('dt', { text: 'Organisation' }), h('dd', {}, acct(role.org, short(role.org))),
          h('dt', { text: 'Allowance left' }), h('dd', { class: 'mono', text: `${musdc(role.allowance)} mUSDC` }),
          h('dt', { text: 'Per payment limit' }), h('dd', { class: 'mono', text: `${musdc(role.per_tx_limit)} mUSDC` }),
          h('dt', { text: 'Role expires' }), h('dd', { text: utc(role.expires_at) }),
          h('dt', { text: 'Statement line credits' }), h('dd', { class: 'mono', text: String(role.line_credits) })),
        meter('Spent in the last 24 hours', BigInt(role.spent_last_24h), BigInt(role.daily_limit), (v) => `${musdc(v)} mUSDC`),
        meter('Payments in the last 24 hours', BigInt(role.tx_last_24h), BigInt(role.max_tx_per_day), (v) => v.toString()))),
    h('section', { class: 'panel' },
      h('div', { class: 'panel-head' }, h('h2', { text: 'What its key can call' })),
      h('div', { class: 'panel-body' },
        h('p', { class: 'small muted', text: 'Read live from the access key list of the agent account.' }),
        ...fc.map((k) => { const p = k.access_key.permission as { FunctionCall: { receiver_id: string; method_names: string[] } }; return h('div', { class: 'stack' }, h('p', { class: 'small' }, 'Function call key for ', h('span', { class: 'mono', text: p.FunctionCall.receiver_id === REGISTRY ? 'the registry' : p.FunctionCall.receiver_id })), h('div', { class: 'method-list' }, ...p.FunctionCall.method_names.map((m) => h('span', { class: 'method', text: m })))); }),
        full ? h('p', { class: 'notice bad' }, badge(`${full} full access key`, 'bad'), 'Unexpected for an agent.') : h('p', { class: 'small' }, badge('No full access key', 'ok')),
        h('p', { class: 'label', text: 'Cannot call' }),
        h('div', { class: 'method-list' }, ...['nft_transfer', 'claim', 'withdraw_allowance', 'grant_role', 'register_share'].map((m) => h('span', { class: 'method no', text: m }))),
        h('p', { class: 'small muted', text: 'Those methods need a deposit of 1 yoctoNEAR, and a function call key cannot attach one. They also check ownership or role in the contract.' }))));
}

async function load() {
  try { target.replaceChildren(...(await Promise.all(AGENTS.map(card)))); stamp(document.querySelector('[data-stamp]')); }
  catch (err) { fail(target, err, () => void load()); }
}
void load();
