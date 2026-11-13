/**
 * Public facts about the NEAR testnet deployment, copied from the repository README and
 * deployments/testnet.json. Live values on the registry pages come from view calls, not from here.
 * The scene list is the recorded on-chain demo (all on 2026-11-13 UTC).
 */
export const NETWORK = 'testnet';
export const REGISTRY = 'reg.royltee-dev.testnet';
export const TOKEN = 'usdc.royltee-dev.testnet';
export const REGISTRAR = 'registrar.royltee-dev.testnet';
export const AGENTS = ['pay-agent.buyer-pub.royltee-dev.testnet'] as const;
export const PAYER = 'payer.royltee-dev.testnet';
export const TOKEN_DECIMALS = 6;
/** Both hosts are already in the site CSP connect-src. The first is tried first. */
export const RPC_URLS = ['https://test.rpc.fastnear.com', 'https://rpc.testnet.near.org'] as const;
export const EXPLORER = 'https://testnet.nearblocks.io';
export const txUrl = (hash: string) => `${EXPLORER}/txns/${hash}`;
export const accountUrl = (account: string) => `${EXPLORER}/address/${account}`;
/** Display names for the demo accounts. All fictional. */
export const ROLE_OF: Record<string, string> = {
  'writer-a.royltee-dev.testnet': 'Writer A',
  'writer-b.royltee-dev.testnet': 'Writer B',
  'buyer-pub.royltee-dev.testnet': 'Buyer publisher',
  'pay-agent.buyer-pub.royltee-dev.testnet': 'Payment agent',
  'payer.royltee-dev.testnet': 'Payer (Simulated)',
  'outsider.royltee-dev.testnet': 'Outsider, no credential',
  'registrar.royltee-dev.testnet': 'Registrar',
};
export type Scene = { step: string; what: string; shows: string; txs: string[]; tokens: string[]; result: 'ok' | 'refused' | 'info'; simulated?: boolean };
export const SCENES: Scene[] = [
  { step: 'Pay', what: 'Payer pays work 0 once, 1.0 mUSDC, split 0.5 and 0.5 into vaults s1 and s2', shows: 'One payment, split by basis points', txs: ['HBzKi5ptr5sGvPSgwWQSUNMKBRJksNCp3tAeT4xByo8u'], tokens: ['s1', 's2'], result: 'ok', simulated: true },
  { step: 'Deposit', what: 'Plain 0.2 mUSDC straight into vault s3, then sync', shows: 'A direct deposit is picked up', txs: ['5wiQVMdfYtuFvUsYcaTeM31sZPizytAaT8etsDx9dQZo', '2Jsht1EatGqZfeVrZePKyVMQPyqyeqTCpEcoaHWK44LY'], tokens: ['s3'], result: 'ok' },
  { step: 'Claim', what: 'writer-a claims s1', shows: 'The holder claims', txs: ['FLzUW7hfLkY1waMuwRDoLCdhAxKsXyxv2FmJ4UoRnsNH'], tokens: ['s1'], result: 'ok' },
  { step: 'Sale', what: 'Catalog sale: writer-b transfers s2 to buyer-pub', shows: 'Control moves with the NFT. Vault s2 keeps 0 keys', txs: ['HZ9pD2kLbqbjmKqC68ao6KBTUpQmQstyd2Tro8hWwYPy'], tokens: ['s2'], result: 'ok' },
  { step: 'Claim', what: 'buyer-pub claims s2, the 0.5 paid before the sale', shows: 'Unclaimed money moved with the token', txs: ['M4pWDbEgnUfz4TiJA8ZZdNVCnwV9vzsBnnEqRgALDrp'], tokens: ['s2'], result: 'ok' },
  { step: 'Refusal', what: 'outsider tries to claim s1', shows: 'Refused: only the current holder can claim', txs: ['GvcHLtG7vLQXihNgbH6cmGSnG8QWhsNgzYGFQ2URwAVB'], tokens: ['s1'], result: 'refused' },
  { step: 'Agent', what: 'Agent pays work 1 from its allowance, line agent-line-1', shows: 'The contract picks the payees. Signed with the agent full access key before that key was deleted', txs: ['7DJThzgCgnZYf9uNhPgH5ZsuJKx8mJqHkiLcqszdRSTu'], tokens: ['s3', 's4'], result: 'ok', simulated: true },
  { step: 'Agent', what: 'Agent full access key deleted', shows: 'From here the agent holds a function call key only', txs: ['7Bwi9rmQk4R4iApdVTXMWLkGjMgDnh2qZiVEfGPY5NAH'], tokens: [], result: 'info' },
  { step: 'Agent', what: 'Agent report, signed with the function call key', shows: 'The scoped key works', txs: ['CmRvd9ZNcfsR5EnMsc3VJ6YYncE3mQgLWqF21YhS1w5G'], tokens: [], result: 'ok' },
  { step: 'Agent', what: 'Agent submits agent-line-1 again', shows: 'Refused: a statement line pays once', txs: ['2547uRUDcHokGMabzgLXdHZVfuG1VzDyxBc2X1rxmFb3'], tokens: [], result: 'refused' },
  { step: 'Dispute', what: 'writer-b registers a 20 percent share on work 2, contesting s5', shows: 'Over 100 percent: s7 minted as claimant, dispute 0 opened', txs: ['3uKby5aP5pN6sMEp1EJSWn7xDEzyH2nASNzS7ccstJba'], tokens: ['s5', 's7'], result: 'info' },
  { step: 'Pay', what: 'Payer pays work 2, 1.0 mUSDC', shows: 's6 paid 0.3 as usual. The 0.7 for s5 held in dispute escrow', txs: ['C8H28Nx1MZ5Pjb13uzkvheAh7S6Jf5mdqqHqmM1n7Mu'], tokens: ['s5', 's6'], result: 'ok', simulated: true },
  { step: 'Resolve', what: 'Registrar resolves dispute 0, uphold existing, with a document hash', shows: 's7 void. The 0.7 in escrow released to s5 only', txs: ['7wvLcnuB5L6w3rh3tBL2Jgg7yTMErD9QduC1XLSAYUVi'], tokens: ['s5', 's7'], result: 'ok' },
];

/** Activity filters for a recorded scene. A row can sit in more than one. */
export function categoriesOf(s: Scene): string[] {
  const c = new Set<string>();
  if (s.step === 'Pay' || s.step === 'Deposit') c.add('payments');
  if (s.step === 'Claim' || s.step === 'Refusal') c.add('claims');
  if (s.step === 'Sale') c.add('sale');
  if (s.step === 'Agent') c.add('agent');
  if (s.step === 'Dispute' || s.step === 'Resolve' || /escrow/i.test(s.shows)) c.add('dispute');
  if (s.result === 'refused') c.add('refused');
  if (s.step === 'Resolve') c.add('registrar');
  return [...c];
}

/** Code hashes from deployments/testnet.json in the contracts repository (recorded 2026-11-13 07:59 UTC). */
export const DEPLOY_RECORD = {
  registry: { account: 'reg.royltee-dev.testnet', codeHash: 'GZkyojcHYDip7pZsDYvkgkjEtgGakPZZcX28cgSyAVhq', sha256: 'e7439e120c7c00e3b71a658b1083dfd28bfd67708d820ef84a10157c87de10f8' },
  mockUsdc: { account: 'usdc.royltee-dev.testnet', codeHash: '47ybTZxhaJ9ZW3FC9jwaDdLfwgkJ59cCrnkSn5oop67t', sha256: '2e5c896b2ead7d1bd12a2944971d6a393902084cf47616c54d18e7a577efc0bb' },
  vault: { codeHash: 'BBLu1w41doKx3yLRHm1tKS47wQQeB8qev9B5P3iEHKKU', sha256: '973b2cbdbe904de710dcc7bf54a7b4164156f8c81d2932191e6968514aaaa727' },
} as const;

/** register_share transactions that minted each share (deployments/testnet.json, steps 9 and 13). */
export const MINTS: Record<string, { tx: string; at: string }> = {
  s1: { tx: '7wxZ7Bmstzu9qTUayrvzbGP97tcD6GRLMaZo7pTVAqzh', at: '2026-11-13T05:49:49Z' },
  s2: { tx: '5w2dvQS54div1eWYPrfFBteYK541cQiCxfvfKhEYw51c', at: '2026-11-13T05:49:56Z' },
  s3: { tx: '6TM8ZRPrH88GXN9AcgBCF7WGkyLLnn64PnkRnqNEg3Fz', at: '2026-11-13T05:50:03Z' },
  s4: { tx: 'DPSu7aR4HCsTp4mvvAKgbTbwcAzDnvADYBmvsoSpANuH', at: '2026-11-13T05:50:10Z' },
  s5: { tx: '79TqB2ZMs8DcTGpsvYidKvffe4ytf7gWngWDveJnVhDf', at: '2026-11-13T05:50:17Z' },
  s6: { tx: '4VFK3fPFo5Amk9B6r2KzBqpkVEmYNtN53vpcHyLC9q5J', at: '2026-11-13T05:50:23Z' },
  s7: { tx: '3uKby5aP5pN6sMEp1EJSWn7xDEzyH2nASNzS7ccstJba', at: '2026-11-13T07:42:25Z' },
};
/** nft_transfer transactions in the record. Each emits an nft_transfer event. */
export const TRANSFERS: { token: string; from: string; to: string; tx: string; at: string; what: string }[] = [
  { token: 's2', from: 'writer-b.royltee-dev.testnet', to: 'buyer-pub.royltee-dev.testnet', tx: 'HZ9pD2kLbqbjmKqC68ao6KBTUpQmQstyd2Tro8hWwYPy', at: '2026-11-13T05:59:22Z', what: 'Catalog sale' },
];

/**
 * Registrar events in the record, copied from the EVENT_JSON logs (standard share_registry) of each transaction.
 * The doc hashes in this demo are placeholder test values, not hashes of real documents.
 */
export type RegistrarEventKind = 'credential_granted' | 'credential_suspended' | 'credential_reinstated' | 'payer_role_granted' | 'payer_role_revoked' | 'registrar_transfer' | 'dispute_resolved';
export const REGISTRAR_EVENT_KINDS: [RegistrarEventKind, string][] = [
  ['credential_granted', 'Credentials granted'], ['credential_suspended', 'Credentials suspended'], ['credential_reinstated', 'Credentials reinstated'],
  ['payer_role_granted', 'Payer roles granted'], ['payer_role_revoked', 'Payer roles revoked'], ['registrar_transfer', 'Registrar transfers'], ['dispute_resolved', 'Disputes resolved'],
];
export const REGISTRAR_EVENTS: { kind: RegistrarEventKind; subject: string; detail: string; docHash: string; tx: string; at: string }[] = [
  { kind: 'credential_granted', subject: 'writer-b.royltee-dev.testnet', detail: 'Member credential', docHash: 'c'.repeat(64), tx: 'Bv7cDUgGrGxgqaXr5FJnGpauqw1DcLpJmUuTMnrydFa6', at: '2026-11-13T05:48:03Z' },
  { kind: 'credential_granted', subject: 'buyer-pub.royltee-dev.testnet', detail: 'Member credential', docHash: 'c'.repeat(64), tx: '8QffMekUyzGzF6UvPX256tUBBBcXsjhy5M6GTHibUjkB', at: '2026-11-13T05:48:09Z' },
  { kind: 'credential_granted', subject: 'writer-a.royltee-dev.testnet', detail: 'Member credential', docHash: 'c'.repeat(64), tx: '3dqBCz6QkbdbDqkZFSqvmfzHjuZHRkVt45A5kbAvg6Vq', at: '2026-11-13T05:48:37Z' },
  { kind: 'dispute_resolved', subject: 'Dispute 0', detail: 'Uphold existing: s7 void, 0.7 mUSDC released to s5', docHash: 'c'.repeat(64), tx: '7wvLcnuB5L6w3rh3tBL2Jgg7yTMErD9QduC1XLSAYUVi', at: '2026-11-13T07:42:54Z' },
];
