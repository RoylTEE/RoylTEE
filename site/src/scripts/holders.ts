/** Holder chain of one share from the recorded mint and transfers, checked against the live owner. */
import { MINTS, TRANSFERS } from '../data/testnet';

export type HolderStep = { holder: string; how: string; tx: string | null; at: string | null; from?: string };
export type HolderChain = { steps: HolderStep[]; recordedHolder: string; liveOwner: string; matches: boolean };

export function holderChain(token: string, holderAtMint: string, liveOwner: string): HolderChain {
  const mint = MINTS[token];
  const steps: HolderStep[] = [{ holder: holderAtMint, how: 'Minted to', tx: mint?.tx ?? null, at: mint?.at ?? null }];
  for (const t of TRANSFERS.filter((x) => x.token === token).sort((a, b) => a.at.localeCompare(b.at))) {
    steps.push({ holder: t.to, how: `${t.what}, transferred from`, tx: t.tx, at: t.at, from: t.from });
  }
  const recordedHolder = steps[steps.length - 1].holder;
  return { steps, recordedHolder, liveOwner, matches: recordedHolder === liveOwner };
}
