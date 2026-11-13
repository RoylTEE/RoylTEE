/**
 * Review 03d M1 (THREAT_MODEL T21): the ledger's claimable figure can run ahead of the vault's real
 * token balance (on the deployed registry anyone can call sync). Pages show the lower of the two.
 */
export function shownClaimable(ledger: string, vaultBalance: string | null): string {
  const l = BigInt(ledger || '0');
  if (vaultBalance === null) return l.toString();
  const v = BigInt(vaultBalance);
  return (v < l ? v : l).toString();
}

/**
 * Second reviewer 03d item 5: four separate figures per share. `inVault` is the vault's real balance,
 * `claimable` the lower of ledger and vault, `unsynced` money in the vault the ledger has not recorded
 * yet (a plain deposit before sync), `held` money held in dispute escrow by the registry.
 */
export type Breakdown = { inVault: bigint | null; claimable: bigint; unsynced: bigint; held: bigint };
export function breakdown(e: { ledger_claimable?: string; claimable: string; vault_balance?: string | null; escrowed?: string }): Breakdown {
  const ledger = BigInt(e.ledger_claimable ?? e.claimable ?? '0');
  const inVault = e.vault_balance === null || e.vault_balance === undefined ? null : BigInt(e.vault_balance);
  const claimable = inVault === null ? ledger : (inVault < ledger ? inVault : ledger);
  const unsynced = inVault !== null && inVault > ledger ? inVault - ledger : 0n;
  return { inVault, claimable, unsynced, held: BigInt(e.escrowed || '0') };
}
