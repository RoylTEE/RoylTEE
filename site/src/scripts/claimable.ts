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
