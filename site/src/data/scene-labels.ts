/**
 * One line per recorded scene saying what NEAR itself does in that step, in plain words.
 * Shown on the Activity table as "NEAR does". Keyed by the step name, with a few scenes
 * overridden by the first transaction of the scene. Every scene must get a line (tests/dashboard-start.test.ts).
 */
import { SCENES, type Scene } from './testnet';

const BY_STEP: Record<string, string> = {
  Pay: 'runs the registry code that splits one payment by basis points and credits each share vault in the same transaction.',
  Deposit: 'records a plain token transfer to the vault; the sync call then reads the balance on chain.',
  Claim: 'checks that the caller holds the share NFT, then pays the vault balance out to that account.',
  Sale: 'moves the NFT to the buyer; the vault follows its owner and still holds no keys.',
  Refusal: 'rejects the call because the signer does not hold the share, so no money moves.',
  Agent: 'checks the allowance and the limits in the contract before it pays, and the contract picks the payees.',
  Dispute: 'records the dispute and holds the contested money in escrow.',
  Resolve: 'applies the registrar decision and releases the escrow to the share that won.',
  Register: 'records the registration and mints the share NFT, with its own vault account that has no keys.',
  Approve: 'checks that the signer registered the work, then releases the held payment to the share vault.',
  Auction: 'moves the share NFT as part of a sealed-bid sale run by a separate auction contract.',
};

const BY_TX: Record<string, string> = {
  '7Bwi9rmQk4R4iApdVTXMWLkGjMgDnh2qZiVEfGPY5NAH': 'removes the access key from the agent account for good, so only a limited key is left.',
  CmRvd9ZNcfsR5EnMsc3VJ6YYncE3mQgLWqF21YhS1w5G: 'lets the limited function call key call only the methods it was granted.',
  '2547uRUDcHokGMabzgLXdHZVfuG1VzDyxBc2X1rxmFb3': 'reverts the call because the statement line was already paid, so nothing is paid twice.',
  '7Vu5wRDqx7dj487LAk3e69wFt6FmH8bMNfzpL8E2LuaW': 'reverts the call because it is over the per transaction limit, so nothing is paid.',
  C8H28Nx1MZ5Pjb13uzkvheAh7S6Jf5mdqqHqmM1n7Mu: 'pays the undisputed share as usual and holds the contested part in escrow.',
  '5xYY2sWQQrZL7XCiQ8CVKhw27pUWDRhnN5k3doqjsgjB': 'mints the share NFT as pending: it exists, but nothing is paid to it until the work registrant signs.',
  '8y7g9RfonZSfhBHKvRVAiYytyK1zEeo2zp5kgJV4cokL': 'pays s14 at once and holds the half for pending s15 in escrow instead of paying it.',
  Ae6ZccUJNUoRZRSPesWyJGCnpSPZ8hs11Ch5WjFANHiR: 'moves the share NFT to the auction contract, which holds it for the sale.',
  CATqXVEauHtQ1mpFHQo7VxQdj9fCwnEQQUFGh9NK5esW: 'applies the answer from the TEE: the share and the money move in one transaction.',
};

export function nearDoes(s: Scene): string {
  return BY_TX[s.txs[0]] ?? BY_STEP[s.step] ?? '';
}
export const allLabelled = () => SCENES.every((s) => nearDoes(s) !== '');

/** The split sheet scene on work 6: pending share to approved share, with the real transactions. */
export const SPLIT_SCENE = {
  workId: 6,
  workCode: 'ZZRYL2600004',
  shares: ['s14', 's15'] as const,
  steps: [
    { label: 'writer-a registers work 6 and its own share s14, approved at once', txs: ['EMdw5gBJg8LAeuZ96n767iYiV6zyYhAgqmQ1Gf89W2sy', '9aUzpdgBPrdJ3XaSn3YdKu19yjaBjKZj1S9wzowL9Nwt'] },
    { label: 'writer-b registers s15, which stays pending', txs: ['5xYY2sWQQrZL7XCiQ8CVKhw27pUWDRhnN5k3doqjsgjB'] },
    { label: 'Payer pays 1.0 mUSDC: s14 gets 0.5, the 0.5 for s15 goes to escrow', txs: ['8y7g9RfonZSfhBHKvRVAiYytyK1zEeo2zp5kgJV4cokL'] },
    { label: 'writer-a signs approve_share on s15: the held 0.5 mUSDC is released', txs: ['5dgncSm1CyDuiEKRzeUpZJTWrBnHzNuuEP6k51RNseGa'] },
  ],
} as const;
