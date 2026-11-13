/** Shares, Share accounts: keys, NEAR against the storage minimum, and Mock USDC held, for every share account. */
import { allTokens, accessKeys, viewAccount, tokenBalance, storagePrice, pool, musdc, near } from './chain';
import { h, badge, acct, fig, fail, stamp } from './ui';

const target = document.querySelector('[data-accounts]')!;
async function load() {
  try {
    const tokens = await allTokens();
    const [keys, accts, held, perByte] = await Promise.all([
      pool(tokens, (t) => accessKeys(t.extra.vault)), pool(tokens, (t) => viewAccount(t.extra.vault)), pool(tokens, (t) => tokenBalance(t.extra.vault)), storagePrice().catch(() => 10n ** 19n),
    ]);
    const rows = tokens.map((t, i) => { const reserve = BigInt(accts[i].storage_usage) * perByte; return { t, keys: keys[i].length, bal: BigInt(accts[i].amount), reserve, head: BigInt(accts[i].amount) - reserve, held: held[i] }; });
    const keyless = rows.filter((r) => r.keys === 0).length, above = rows.filter((r) => r.head > 0n).length;
    document.querySelector('[data-acc-fig]')!.replaceChildren(
      fig('Share accounts', 'Accounts', String(rows.length)),
      fig('With 0 access keys', 'Keyless', `${keyless}/${rows.length}`, '', keyless === rows.length ? badge('All keyless', 'ok') : badge('Check keys', 'bad'), { keep: true }),
      fig('Above the storage minimum', 'Above minimum', `${above}/${rows.length}`, '', above === rows.length ? badge('All above', 'ok') : badge('Top up needed', 'bad'), { keep: true }),
      fig('Mock USDC held', 'mUSDC held', musdc(rows.reduce((a, r) => a + BigInt(r.held), 0n)), 'mUSDC'),
    );
    target.replaceChildren(h('table', { class: 'ledger rows-sm' },
      h('thead', {}, h('tr', {}, ...['Share', 'Share account', 'Access keys'].map((x) => h('th', { text: x })), ...['NEAR balance', 'Storage reserve', 'Above minimum', 'mUSDC held'].map((x) => h('th', { class: 'num', text: x })))),
      h('tbody', {}, ...rows.map((r) => h('tr', {},
        h('td', { class: 'p' }, h('a', { class: 'link mono', href: `/share?id=${r.t.token_id}#account`, text: r.t.token_id })),
        h('td', { class: 'x' }, acct(r.t.extra.vault)),
        h('td', { class: 'p' }, r.keys === 0 ? badge('0 keys', 'ok') : badge(`${r.keys} keys`, 'bad')),
        h('td', { class: 'num m', text: `${near(r.bal)} NEAR` }),
        h('td', { class: 'num x', text: near(r.reserve) }),
        h('td', { class: 'num m' }, h('span', { class: 'only-narrow', text: 'free ' }), near(r.head)),
        h('td', { class: 'num p-end', text: musdc(r.held) }))))));
    stamp(document.querySelector('[data-stamp]'));
  } catch (err) { fail(target, err, () => void load()); }
}
void load();
