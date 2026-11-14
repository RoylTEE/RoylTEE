/** Verify: live code hashes against the deploy record, keyless share accounts, keys left on contract accounts. */
import { allTokens, view, viewAccount, accessKeys, pool, tokenBalance } from './chain';
import { DEPLOY_RECORD, REGISTRY, TOKEN, AGENTS } from '../data/testnet';
import { CLAIMS, TOKEN_OWNERS, AGENT_KEYS } from '../data/claims';
import { evaluate, type Facts } from './verify-claims';
import { h, badge, acct, fig, fail, stamp } from './ui';
import { keyStatus } from './key-status';

const $ = (s: string) => document.querySelector(s)!;
const cut = (x: string) => `${x.slice(0, 8)}...${x.slice(-6)}`;
const match = (ok: boolean) => ok ? badge('Matches', 'ok') : badge('Differs', 'bad');

/** Fills the Check now column of the claims table with read-only view calls. */
async function loadClaims(vaultKeyCounts: number[]) {
  const rows = [...document.querySelectorAll<HTMLElement>('[data-claim][data-check]')].filter((r) => r.dataset.check);
  try {
    const agent = AGENTS[0];
    const [ledgerS1, ledgerS2, accounting, registryMusdc, s2, agentKeys, role, dispute0, outsiderActive, memberActive, usdcMeta] = await Promise.all([
      view<Facts['ledgerS1']>('get_ledger', { token_id: 's1' }), view<Facts['ledgerS2']>('get_ledger', { token_id: 's2' }),
      view<Facts['accounting']>('get_accounting'), tokenBalance(REGISTRY), view<{ owner_id: string }>('nft_token', { token_id: 's2' }),
      accessKeys(agent), view<Facts['role']>('get_role', { agent }), view<Facts['dispute0']>('get_dispute', { dispute_id: 0 }),
      view<boolean>('is_active_member', { account_id: 'outsider.royltee-dev.testnet' }), view<boolean>('is_active_member', { account_id: 'writer-a.royltee-dev.testnet' }),
      view<Facts['usdcMeta']>('ft_metadata', {}, TOKEN),
    ]);
    const results = evaluate({
      vaultKeyCounts, ledgerS1, ledgerS2, accounting, registryMusdc, s2Owner: s2.owner_id, recordedS2Owner: TOKEN_OWNERS.s2,
      agentKeys: agentKeys.map((k) => ({ permission: k.access_key.permission })), registry: REGISTRY, recordedAgentMethods: AGENT_KEYS[0].methods,
      role, dispute0, outsiderActive, memberActive, usdcMeta,
    });
    for (const row of rows) {
      const r = results[row.dataset.check as keyof typeof results];
      const cell = row.querySelector('[data-result]')!;
      cell.replaceChildren(badge(r.ok ? 'Pass' : 'Fail', r.ok ? 'ok' : 'bad'), h('div', { class: 'caption', text: r.detail }));
    }
    const live = CLAIMS.filter((c) => c.check);
    const passed = live.filter((c) => results[c.check!].ok).length;
    $('[data-claims-sum]').replaceChildren(fig('Claims that pass a live check now', 'Claims', `${passed}/${live.length}`, '', passed === live.length ? badge('All pass', 'ok') : badge('Check', 'bad'), { keep: true }));
  } catch (err) {
    for (const row of rows) row.querySelector('[data-result]')!.replaceChildren(badge('Not read', 'warn'), h('div', { class: 'caption', text: (err as Error).message }));
  }
}

async function load() {
  try {
    const tokens = await allTokens();
    const [reg, usdc, vaultHex, vaults, vkeys, regKeys, usdcKeys] = await Promise.all([
      viewAccount(REGISTRY), viewAccount(TOKEN), view<string>('get_vault_code_hash'),
      pool(tokens, (t) => viewAccount(t.extra.vault)), pool(tokens, (t) => accessKeys(t.extra.vault)),
      accessKeys(REGISTRY), accessKeys(TOKEN),
    ]);
    const rows: [string, string, string, boolean][] = [
      ['Registry', DEPLOY_RECORD.registry.codeHash, reg.code_hash, reg.code_hash === DEPLOY_RECORD.registry.codeHash],
      ['Mock USDC (testnet)', DEPLOY_RECORD.mockUsdc.codeHash, usdc.code_hash, usdc.code_hash === DEPLOY_RECORD.mockUsdc.codeHash],
      ['Vault code the registry deploys', DEPLOY_RECORD.vault.sha256, vaultHex, vaultHex === DEPLOY_RECORD.vault.sha256],
    ];
    // Each share account must run a recorded vault build: the current one or one from an earlier deploy (review 03d L5).
    const vaultOk = vaults.map((v) => v.code_hash === DEPLOY_RECORD.vault.codeHash || DEPLOY_RECORD.earlierVaultCodeHashes.includes(v.code_hash));
    const keyless = vkeys.filter((k) => k.length === 0).length;
    const full = (ks: typeof regKeys) => ks.filter((k) => k.access_key.permission === 'FullAccess').length;
    $('[data-vfig]').replaceChildren(
      fig('Contract code matches the record', 'Code matches', `${rows.filter((r) => r[3]).length}/${rows.length}`, '', rows.every((r) => r[3]) ? badge('All match', 'ok') : badge('Check', 'bad'), { keep: true }),
      fig('Share accounts running the vault code', 'Vault code', `${vaultOk.filter(Boolean).length}/${tokens.length}`),
      fig('Share accounts with 0 keys', 'Keyless', `${keyless}/${tokens.length}`, '', keyless === tokens.length ? badge('All keyless', 'ok') : badge('Check keys', 'bad'), { keep: true }),
      fig('Full access keys on contract accounts', 'Contract keys', String(full(regKeys) + full(usdcKeys)), '', badge('Kept on testnet', 'warn'), { keep: true }),
    );
    $('[data-code]').replaceChildren(h('table', { class: 'ledger rows-sm' },
      h('thead', {}, h('tr', {}, ...['Contract', 'Deploy record', 'On chain now', 'Result'].map((x) => h('th', { text: x })))),
      h('tbody', {}, ...rows.map(([n, want, got, ok]) => h('tr', {}, h('td', { class: 'p', text: n }), h('td', { class: 'm hash', title: want, text: cut(want) }), h('td', { class: 'm hash', title: got, text: cut(got) }), h('td', { class: 'p-end' }, match(ok)))))));
    $('[data-vaults]').replaceChildren(h('table', { class: 'ledger rows-sm' },
      h('thead', {}, h('tr', {}, ...['Share', 'Share account', 'Code hash', 'Vault code', 'Access keys'].map((x) => h('th', { text: x })))),
      h('tbody', {}, ...tokens.map((t, i) => h('tr', {},
        h('td', { class: 'p' }, h('a', { class: 'link mono', href: `/share?id=${t.token_id}#account`, text: t.token_id })),
        h('td', { class: 'x' }, acct(t.extra.vault)),
        h('td', { class: 'm hash', title: vaults[i].code_hash, text: cut(vaults[i].code_hash) }),
        h('td', { class: 'm' }, match(vaultOk[i])),
        h('td', { class: 'p-end' }, vkeys[i].length === 0 ? badge('0 keys', 'ok') : badge(`${vkeys[i].length} keys`, 'bad')))))));
    $('[data-keys]').replaceChildren(h('table', { class: 'ledger rows-sm' },
      h('thead', {}, h('tr', {}, ...['Account', 'Keys', 'Full access', 'Status'].map((x) => h('th', { text: x })))),
      h('tbody', {}, ...([[REGISTRY, regKeys], [TOKEN, usdcKeys]] as const).map(([a, ks]) => h('tr', {},
        h('td', { class: 'p' }, acct(a)), h('td', { class: 'm', text: `${ks.length} key${ks.length === 1 ? '' : 's'}` }), h('td', { class: 'm', text: `${full(ks)} full access` }),
        h('td', { class: 'p-end' }, ((s) => h('span', { title: s.detail.join('\n') || undefined }, badge(s.text, s.tone), ...s.detail.filter((d) => !d.endsWith('full access')).map((d) => h('div', { class: 'caption mono', text: d }))))(keyStatus(ks))))))));
    stamp(document.querySelector('[data-stamp]'));
    void loadClaims(vkeys.map((k) => k.length));
  } catch (err) { fail($('[data-code]'), err, () => void load()); }
}
void load();
