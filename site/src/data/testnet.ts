/**
 * Public facts about the NEAR testnet deployment, copied from the repository README and
 * deployments/testnet.json. Live values on the registry pages come from view calls, not from here.
 * The scene list is the recorded on-chain demo (2026-11-13 UTC, plus the split sheet and auction scenes of 2026-11-14 UTC). The rows from 'Pay' with line-sale-2 on
 * ran on the registry build with the contract review fixes (deployed 2026-11-13 18:54 UTC).
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
export const txUrl = (hash: string) => `${EXPLORER}/txns/${encodeURIComponent(hash)}`;
export const accountUrl = (account: string) => `${EXPLORER}/address/${encodeURIComponent(account)}`;
/** Display names for the demo accounts. All fictional. */
export const ROLE_OF: Record<string, string> = {
  'writer-a.royltee-dev.testnet': 'Writer A',
  'writer-b.royltee-dev.testnet': 'Writer B',
  'buyer-pub.royltee-dev.testnet': 'Buyer publisher',
  'pay-agent.buyer-pub.royltee-dev.testnet': 'Payment agent',
  'payer.royltee-dev.testnet': 'Payer (test account)',
  'outsider.royltee-dev.testnet': 'Outsider, no credential',
  'registrar.royltee-dev.testnet': 'Registrar',
};
export type Scene = { step: string; what: string; shows: string; txs: string[]; tokens: string[]; result: 'ok' | 'refused' | 'info'; simulated?: boolean; agent?: boolean };
export const SCENES: Scene[] = [
  { step: 'Pay', what: 'Payer pays work 0 once, 1.0 mUSDC, split 0.5 and 0.5 into vaults s1 and s2', shows: 'One payment, split by basis points', txs: ['HBzKi5ptr5sGvPSgwWQSUNMKBRJksNCp3tAeT4xByo8u'], tokens: ['s1', 's2'], result: 'ok', simulated: true },
  { step: 'Deposit', what: 'Plain 0.2 mUSDC straight into vault s3, then sync', shows: 'A direct deposit is picked up', txs: ['5wiQVMdfYtuFvUsYcaTeM31sZPizytAaT8etsDx9dQZo', '2Jsht1EatGqZfeVrZePKyVMQPyqyeqTCpEcoaHWK44LY'], tokens: ['s3'], result: 'ok' },
  { step: 'Claim', what: 'writer-a claims s1', shows: 'The holder claims', txs: ['FLzUW7hfLkY1waMuwRDoLCdhAxKsXyxv2FmJ4UoRnsNH'], tokens: ['s1'], result: 'ok' },
  { step: 'Sale', what: 'Catalog sale: writer-b transfers s2 to buyer-pub', shows: 'Control moves with the NFT. Vault s2 keeps 0 keys', txs: ['HZ9pD2kLbqbjmKqC68ao6KBTUpQmQstyd2Tro8hWwYPy'], tokens: ['s2'], result: 'ok' },
  { step: 'Claim', what: 'buyer-pub claims s2, the 0.5 paid before the sale', shows: 'Unclaimed money moved with the token', txs: ['M4pWDbEgnUfz4TiJA8ZZdNVCnwV9vzsBnnEqRgALDrp'], tokens: ['s2'], result: 'ok' },
  { step: 'Refusal', what: 'outsider tries to claim s1', shows: 'Refused: only the current holder can claim', txs: ['GvcHLtG7vLQXihNgbH6cmGSnG8QWhsNgzYGFQ2URwAVB'], tokens: ['s1'], result: 'refused' },
  { step: 'Agent', what: 'Agent pays work 1 from its allowance, line agent-line-1', shows: 'The contract picks the payees. Signed with the agent full access key before that key was deleted', txs: ['7DJThzgCgnZYf9uNhPgH5ZsuJKx8mJqHkiLcqszdRSTu'], tokens: ['s3', 's4'], result: 'ok', agent: true },
  { step: 'Agent', what: 'Agent full access key deleted', shows: 'From here the agent holds a function call key only', txs: ['7Bwi9rmQk4R4iApdVTXMWLkGjMgDnh2qZiVEfGPY5NAH'], tokens: [], result: 'info' },
  { step: 'Agent', what: 'Agent report, signed with the function call key', shows: 'The scoped key works', txs: ['CmRvd9ZNcfsR5EnMsc3VJ6YYncE3mQgLWqF21YhS1w5G'], tokens: [], result: 'ok' },
  { step: 'Agent', what: 'Agent submits agent-line-1 again', shows: 'Refused: a statement line pays once', txs: ['2547uRUDcHokGMabzgLXdHZVfuG1VzDyxBc2X1rxmFb3'], tokens: [], result: 'refused' },
  { step: 'Dispute', what: 'writer-b registers a 20 percent share on work 2, contesting s5', shows: 'Over 100 percent: s7 minted as claimant, dispute 0 opened', txs: ['3uKby5aP5pN6sMEp1EJSWn7xDEzyH2nASNzS7ccstJba'], tokens: ['s5', 's7'], result: 'info' },
  { step: 'Pay', what: 'Payer pays work 2, 1.0 mUSDC', shows: 's6 paid 0.3 as usual. The 0.7 for s5 held in dispute escrow', txs: ['C8H28Nx1MZ5Pjb13uzkvheAh7S6Jf5mdqqHqmM1n7Mu'], tokens: ['s5', 's6'], result: 'ok', simulated: true },
  { step: 'Resolve', what: 'Registrar resolves dispute 0, uphold existing, with a document hash', shows: 's7 void. The 0.7 in escrow released to s5 only', txs: ['7wvLcnuB5L6w3rh3tBL2Jgg7yTMErD9QduC1XLSAYUVi'], tokens: ['s5', 's7'], result: 'ok' },
  { step: 'Pay', what: 'Payer pays work 0 again, 1.0 mUSDC, line line-sale-2, after the sale of s2', shows: 'The new holder of s2 now receives its 0.5', txs: ['2aAgWtcaEC1RinbQvVS1AoLpZn7zQeEmzAGW3FshXHQA'], tokens: ['s1', 's2'], result: 'ok', simulated: true },
  { step: 'Refusal', what: 'writer-b, who sold s2, tries to claim it', shows: 'Refused: only the current holder can claim', txs: ['EYDV3Tsv7Q1ZFPnJzmVdk6D4Rtosqpn8fYNY71ofVWJU'], tokens: ['s2'], result: 'refused' },
  { step: 'Claim', what: 'buyer-pub claims s2, 0.5 mUSDC', shows: 'The buyer collects the new payment', txs: ['ASynFkHYZ5CxZdSi6paosMVuZiQf66mvEusNn4mnxhiw'], tokens: ['s2'], result: 'ok' },
  { step: 'Register', what: 'writer-a registers work 3 (ZZRYL2600005, provenance ai_assisted) and shares s8 and s9, 50 percent each', shows: 'Provenance is a declared field; its evidence is Simulated, not checked', txs: ['3er9FiUfRj1msBQkDbM2WhxazMou8hdRw95xPPs1eyKA', 'FGrwBa58Empz2zW1Yj2BTHPvaSw9ZjsrVrDT6PrbN4gf', 'BDE7EtebNBcfNX5PXCi2aniUCaLwQivKzQwpvSxq7Cft'], tokens: ['s8', 's9'], result: 'ok' },
  { step: 'Register', what: 'writer-a registers work 4 (ZZRYL2600006, provenance fully_ai) and shares s10 and s11 on the basis of a contract', shows: 'A fully AI track can record a contract based entitlement instead of claiming copyright', txs: ['BRt579jXQx8bhwbMHhtaykeik7D86TWLBVgEX3VNEp1K', '46nERgHy9ydk3x58zwqMCmrrVSnvMJQNMKd26BhpPebe', 'vhe6Vgoc9btbQia4ArgcsbygYiECAmbSm58hqTy1qph'], tokens: ['s10', 's11'], result: 'ok' },
  { step: 'Agent', what: 'Agent pays work 1 from its allowance, 0.1 mUSDC, line agent-line-2, signed with its function call key', shows: 'The contract picks the payees: s3 0.06, s4 0.04', txs: ['2XjtVThM4g7tRkt79XCj1inLj1bBk3vNZiKAbUmQsBaW'], tokens: ['s3', 's4'], result: 'ok', agent: true },
  { step: 'Agent', what: 'Agent tries to pay 0.100001 mUSDC, over its per transaction limit of 0.1', shows: 'Refused: over per-transaction limit', txs: ['7Vu5wRDqx7dj487LAk3e69wFt6FmH8bMNfzpL8E2LuaW'], tokens: [], result: 'refused', agent: true },
  { step: 'Register', what: 'writer-a registers work 5 (ZZRYL2600007) with a metadata link that tells agents to pay outsider instead, and shares s12 and s13', shows: 'Metadata text is stored, never obeyed', txs: ['ryAcUUtRGdxyzsraWZjJE3SnytCbZzpsmZNNCmNFq3y', 'GJopxb2zVGHNEgSUq9RKFHA3wmBYjcyM4wVmhZH5NcKd', 'FR5SZZWyr9dfQdpkxDTQamQXXoRKG6SPTPwK7NU21W1P'], tokens: ['s12', 's13'], result: 'ok' },
  { step: 'Agent', what: 'Agent pays work 5, 0.1 mUSDC, line agent-line-3, with extra payee and receiver_id arguments naming outsider', shows: 'The extra arguments are ignored: s12 0.05, s13 0.05, outsider 0', txs: ['CUW8qssJVCJZuUgzzWHf9bdEyNCkFXpQjUKrJeWXeVPS'], tokens: ['s12', 's13'], result: 'ok', agent: true },
  { step: 'Register', what: 'writer-a registers work 6 (ZZRYL2600004) and its own 50 percent share s14 (split sheet scene)', shows: 'The registrant of the work registers its own share, so it is approved at once', txs: ['EMdw5gBJg8LAeuZ96n767iYiV6zyYhAgqmQ1Gf89W2sy', '9aUzpdgBPrdJ3XaSn3YdKu19yjaBjKZj1S9wzowL9Nwt'], tokens: ['s14'], result: 'ok' },
  { step: 'Register', what: 'writer-b registers the other 50 percent of work 6 as share s15 (split sheet scene)', shows: 'writer-b did not register the work, so s15 is pending: nothing is paid to it until writer-a signs', txs: ['5xYY2sWQQrZL7XCiQ8CVKhw27pUWDRhnN5k3doqjsgjB'], tokens: ['s15'], result: 'ok' },
  { step: 'Pay', what: 'Payer pays work 6, 1.0 mUSDC, line line-4. s14 gets 0.5 at once, the 0.5 for pending s15 goes to escrow', shows: 'The pending half is held, not paid: s14 vault 0.5, s15 vault 0, s15 held 0.5 awaiting approval', txs: ['8y7g9RfonZSfhBHKvRVAiYytyK1zEeo2zp5kgJV4cokL'], tokens: ['s14', 's15'], result: 'ok', simulated: true },
  { step: 'Approve', what: 'writer-a signs approve_share on s15', shows: 'share_approved event: the held 0.5 mUSDC is released to the s15 vault, which now holds 0.5', txs: ['5dgncSm1CyDuiEKRzeUpZJTWrBnHzNuuEP6k51RNseGa'], tokens: ['s15'], result: 'ok' },
  { step: 'Auction', what: 'writer-a lists share s10 in a sealed-bid auction on a separate auction contract (auction.royltee-dev.testnet), reserve 20 mUSDC', shows: 'nft_transfer_call moves s10 to the auction contract, which holds it for the sale', txs: ['Ae6ZccUJNUoRZRSPesWyJGCnpSPZ8hs11Ch5WjFANHiR'], tokens: ['s10'], result: 'ok' },
  { step: 'Auction', what: 'The auction settles through the OutLayer TEE: buyer-pub wins s10 at the second price, 70 mUSDC', shows: 's10 moves from the auction contract to buyer-pub; the vault of s10 keeps 0 keys', txs: ['CATqXVEauHtQ1mpFHQo7VxQdj9fCwnEQQUFGh9NK5esW'], tokens: ['s10'], result: 'ok' },
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

/** Code hashes from deployments/testnet.json in the contracts repository (updated 2026-11-14 09:30 UTC, after the final contract fixes redeploy). */
export const DEPLOY_RECORD = {
  registry: { account: 'reg.royltee-dev.testnet', codeHash: '5YNk7mvmiq4SuSckR1gULXVvgcRdUjaioxmDE8niguAG', sha256: '4378568f0459560130cdb56fa05152f9946116d3332d6c3f7f53e939005dc271' },
  mockUsdc: { account: 'usdc.royltee-dev.testnet', codeHash: '47ybTZxhaJ9ZW3FC9jwaDdLfwgkJ59cCrnkSn5oop67t', sha256: '2e5c896b2ead7d1bd12a2944971d6a393902084cf47616c54d18e7a577efc0bb' },
  vault: { codeHash: 'BgC1pQYrvm91uXYeWFhs6ab1prgVAWMFTSfuj8KQkMXP', sha256: '9e9ef97840a3a3e6967565b43a9c2a467c1925153ea8c5577d666c9b16966cda' },
  /**
   * Vault code hashes from earlier deploys. Existing share accounts keep the vault code they were
   * created with, so s1 to s7 still run the vault build of the first registry deploy (no ft_on_transfer),
   * while s8 to s13 run the current one. When the registry is redeployed with a new vault build, move the old vault code hash here and update the three entries above from
   * deployments/testnet.json (tests/review-03b.test.ts checks that they agree when that file is present).
   */
  earlierVaultCodeHashes: ['BBLu1w41doKx3yLRHm1tKS47wQQeB8qev9B5P3iEHKKU'] as readonly string[],
} as const;

/** register_share transactions that minted each share (deployments/testnet.json, steps 9, 13 and 16 and demo writes W5 to W14). */
export const MINTS: Record<string, { tx: string; at: string }> = {
  s1: { tx: '7wxZ7Bmstzu9qTUayrvzbGP97tcD6GRLMaZo7pTVAqzh', at: '2026-11-13T05:49:49Z' },
  s2: { tx: '5w2dvQS54div1eWYPrfFBteYK541cQiCxfvfKhEYw51c', at: '2026-11-13T05:49:56Z' },
  s3: { tx: '6TM8ZRPrH88GXN9AcgBCF7WGkyLLnn64PnkRnqNEg3Fz', at: '2026-11-13T05:50:03Z' },
  s4: { tx: 'DPSu7aR4HCsTp4mvvAKgbTbwcAzDnvADYBmvsoSpANuH', at: '2026-11-13T05:50:10Z' },
  s5: { tx: '79TqB2ZMs8DcTGpsvYidKvffe4ytf7gWngWDveJnVhDf', at: '2026-11-13T05:50:17Z' },
  s6: { tx: '4VFK3fPFo5Amk9B6r2KzBqpkVEmYNtN53vpcHyLC9q5J', at: '2026-11-13T05:50:23Z' },
  s7: { tx: '3uKby5aP5pN6sMEp1EJSWn7xDEzyH2nASNzS7ccstJba', at: '2026-11-13T07:42:25Z' },
  s8: { tx: 'FGrwBa58Empz2zW1Yj2BTHPvaSw9ZjsrVrDT6PrbN4gf', at: '2026-11-13T21:26:44Z' },
  s9: { tx: 'BDE7EtebNBcfNX5PXCi2aniUCaLwQivKzQwpvSxq7Cft', at: '2026-11-13T21:27:29Z' },
  s10: { tx: '46nERgHy9ydk3x58zwqMCmrrVSnvMJQNMKd26BhpPebe', at: '2026-11-13T21:29:08Z' },
  s11: { tx: 'vhe6Vgoc9btbQia4ArgcsbygYiECAmbSm58hqTy1qph', at: '2026-11-13T21:30:18Z' },
  s12: { tx: 'GJopxb2zVGHNEgSUq9RKFHA3wmBYjcyM4wVmhZH5NcKd', at: '2026-11-13T21:35:29Z' },
  s13: { tx: 'FR5SZZWyr9dfQdpkxDTQamQXXoRKG6SPTPwK7NU21W1P', at: '2026-11-13T21:36:11Z' },
  s14: { tx: '9aUzpdgBPrdJ3XaSn3YdKu19yjaBjKZj1S9wzowL9Nwt', at: '2026-11-14T10:55:21Z' },
  s15: { tx: '5xYY2sWQQrZL7XCiQ8CVKhw27pUWDRhnN5k3doqjsgjB', at: '2026-11-14T10:56:13Z' },
};
/** nft_transfer transactions in the record. Each emits an nft_transfer event. */
export const TRANSFERS: { token: string; from: string; to: string; tx: string; at: string; what: string }[] = [
  { token: 's2', from: 'writer-b.royltee-dev.testnet', to: 'buyer-pub.royltee-dev.testnet', tx: 'HZ9pD2kLbqbjmKqC68ao6KBTUpQmQstyd2Tro8hWwYPy', at: '2026-11-13T05:59:22Z', what: 'Catalog sale' },
  { token: 's10', from: 'writer-a.royltee-dev.testnet', to: 'auction.royltee-dev.testnet', tx: 'Ae6ZccUJNUoRZRSPesWyJGCnpSPZ8hs11Ch5WjFANHiR', at: '2026-11-14T10:18:51Z', what: 'Listed in the sealed-bid auction' },
  { token: 's10', from: 'auction.royltee-dev.testnet', to: 'buyer-pub.royltee-dev.testnet', tx: 'CATqXVEauHtQ1mpFHQo7VxQdj9fCwnEQQUFGh9NK5esW', at: '2026-11-14T10:39:05Z', what: 'Sealed-bid auction sale' },
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
