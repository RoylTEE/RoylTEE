/** Verify: live code hashes against the deploy record, keyless share accounts, keys left on contract accounts. */
import { allTokens, view, viewAccount, accessKeys, pool } from './chain';
import { DEPLOY_RECORD, REGISTRY, TOKEN } from '../data/testnet';
import { h, badge, acct, fig, fail, stamp } from './ui';
import { keyStatus } from './key-status';

const $ = (s: string) => document.querySelector(s)!;
const cut = (x: string) => `${x.slice(0, 8)}...${x.slice(-6)}`;
const match = (ok: boolean) => ok ? badge('Matches', 'ok') : badge('Differs', 'bad');

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
  } catch (err) { fail($('[data-code]'), err, () => void load()); }
}
void load();
