/**
 * The guided tour of the testnet app: one list, read by src/scripts/tour.ts and by the Playwright walk
 * (tests/e2e/tour.e2e.mjs). Copy describes what the page shows and never states a figure. The tour only reads:
 * it never connects a wallet and never sends a transaction.
 */
export type TourStep = {
  id: string;
  /** Page the step lives on. The tour navigates here when the step is shown on another page. */
  path: string;
  /** Query string the page needs for the step, for example "?id=s2". */
  search?: string;
  /** Tab hash to show before the step, on pages that switch panels by hash. */
  hash?: string;
  /** Candidate selectors. The first one that is visible is highlighted. */
  target: string[];
  title: string;
  body: string;
  /** Present on action steps: the person clicks the highlighted element instead of pressing Next. */
  action?: { label: string; prefill?: { selector: string; value: string } };
  /** Only for wide or narrow screens (breakpoint 767 px, the one the ledger uses). */
  viewport?: 'wide' | 'narrow';
};

export const TOUR: TourStep[] = [
  { id: 'menu', path: '/dashboard/', target: ['.menu-btn', '.side'], title: 'Welcome to the app',
    body: 'This is the registry app. Every page is in this menu, grouped as Registry, Oversight, You and Support. On a phone the menu opens from the menu button.' },
  { id: 'totals', path: '/dashboard/', target: ['section[aria-labelledby="stmt-h"]'], title: 'The statement',
    body: 'These totals add up every share account together since mint, read live from NEAR.' },
  { id: 'catalog', path: '/catalog/', target: ['.area-tabs a[href="/catalog/splits/"]'], title: 'The catalog',
    body: 'The catalog lists every registered work and how its rights are split, composition against recording. Tabs on this page lead to the split sheets and a lookup.',
    action: { label: 'Split sheets' } },
  { id: 'sheet', path: '/catalog/splits/', target: ['[data-splits] section.sheet'], title: 'A split sheet',
    body: 'One sheet per work. It lists each share, who holds it, its role and its percent. Every right has to add up to 100 percent, and the total row shows whether it does.' },
  { id: 'lookup', path: '/catalog/lookup/', target: ['.tries .try[href="?q=ZZRYL2600001"]'], title: 'Look up a work',
    body: 'You can search by ISRC, ISWC, share id or account. This example is the code of one registered work.',
    action: { label: 'ZZRYL2600001' } },
  { id: 'lookup-result', path: '/catalog/lookup/', search: '?q=ZZRYL2600001', target: ['[data-lookup] section.panel .panel-head'], title: 'Who gets paid',
    body: 'The search found the work. Below it, each right lists the account that is paid now and whether its money is paid on each payment or held in escrow.' },
  { id: 'ledger', path: '/shares/', target: ['tr[data-row="s2"] a.row-btn'], title: 'The share ledger',
    body: 'Every share is an NFT and has one row here. Share s2 was sold in the recorded demo, so it is a good one to inspect.',
    action: { label: 'share s2' } },
  { id: 'inspector', path: '/shares/', target: ['[data-inspector]'], viewport: 'wide', title: 'The inspector',
    body: 'The panel changed to the share you picked. It shows the holder, the share account, the live key list and the statement for that share.' },
  { id: 'share', path: '/share', search: '?id=s2', hash: 'summary', target: ['.sub-tabs a[data-tab-link="account"]'], title: 'One share',
    body: 'This page is for one share NFT: who holds it and every recorded move. Open Account and keys to see the money account the share controls.',
    action: { label: 'Account and keys' } },
  { id: 'vault', path: '/share', search: '?id=s2', hash: 'account', target: ['#account'], title: 'A vault with no keys',
    body: 'Each share controls its own payout account. The key list is read live from NEAR, and a share account has no access key, so nobody can sign money out of it. Only the holder of the share NFT can claim what has built up.' },
  { id: 'claims', path: '/shares/claims/', target: ['section[aria-labelledby="cl-h"] .panel-head'], title: 'Claims',
    body: 'Money waits in share accounts until the holder claims it. Only the current holder can claim, and the money goes to that holder. Claiming happens on Your shares with a connected wallet.' },
  { id: 'activity', path: '/activity', hash: 'all', target: ['.area-tabs a[data-tab-link="payments"]'], title: 'Recorded activity',
    body: 'Every recorded step of the demo is listed here, newest first. Tabs filter the list by type.',
    action: { label: 'Payments' } },
  { id: 'activity-rows', path: '/activity', hash: 'payments', target: ['#acts'], title: 'Payments only',
    body: 'The list now shows only payments. Each row says what happened and what NEAR does, and links to its transaction on the explorer.' },
  { id: 'auction', path: '/auction', target: ['section[aria-labelledby="what-h"]'], title: 'The sealed-bid auction',
    body: 'A share NFT was sold in a sealed-bid auction. Bids stay encrypted, a program inside a TEE opens them, and the winner pays the second highest bid.' },
  { id: 'attestation', path: '/auction', target: ['#attestation > .panel-head'], title: 'The attestation proof',
    body: 'The TEE gives a signed proof of what ran. The checks below this header show whether each part of the proof passed, so anyone can check that the answer came from the approved code.' },
  { id: 'to-verify', path: '/auction', target: ['.actions a[href="/verify"]'], title: 'Check it yourself',
    body: 'Next is the page where you check the deployment yourself.',
    action: { label: 'Verify the deployment' } },
  { id: 'verify-claims', path: '/verify', target: ['#claims > .panel-head'], title: 'Every claim, checked',
    body: 'Each claim the project makes is listed with its code, its test and its transaction. Claims marked live are read from the chain now with view calls only.' },
  { id: 'verify-code', path: '/verify', target: ['[data-code]'], title: 'Code hashes',
    body: 'This table compares the code hash on each contract now against the deploy record, so you can see that the code running is the code that was recorded.' },
  { id: 'verify-vaults', path: '/verify', target: ['section[aria-labelledby="sa-h"] .panel-head'], title: 'Keyless vaults',
    body: 'The list below this header covers every share account with its vault code and its access keys, read live. A keyless vault means no key can move its money.' },
  { id: 'agents', path: '/agents', target: ['[data-agents]'], title: 'Agents',
    body: 'An agent is software that pays for music. Each one has a role from an organisation, with limits on each payment and on each day. It names the work, and the contract picks who is paid.' },
  { id: 'disputes', path: '/disputes', target: ['section[aria-labelledby="d-h"]'], title: 'Disputes',
    body: 'When claims on one right add up to more than 100 percent, only the contested money is frozen until the registrar decides. Everything else keeps being paid.' },
  { id: 'registrar', path: '/registrar', hash: 'log', target: ['#log'], title: 'The registrar log',
    body: 'The registrar is a trusted role in this demo. Each action that moves a share or money is logged here with a document hash and its transaction. The hashes are placeholders.' },
  { id: 'member', path: '/member', target: ['form[data-lookup] button[type="submit"]'], title: 'Your shares',
    body: 'You can look up any account without a wallet. An example account is filled in for you.',
    action: { label: 'Look up', prefill: { selector: '#acct-in', value: 'writer-a.royltee-dev.testnet' } } },
  { id: 'member-wallet', path: '/member', target: ['.who-grid .field + .field'], title: 'Connecting a wallet',
    body: 'The shares that account holds are listed below. To claim or transfer, you would connect your own wallet here. That is optional, and this tour never connects a wallet.' },
  { id: 'help', path: '/help', hash: 'chat', target: ['#chat .panel-head'], title: 'Help',
    body: 'Ask the help guide about shares, payments and the demo. Answers can be wrong, so the repository docs are the source of truth. Support tickets need a wallet sign in that signs a message, never a transaction.' },
  { id: 'docs', path: '/docs', target: ['section[aria-labelledby="c-h"]'], title: 'Check it without a wallet',
    body: 'The docs list the accounts and the contract methods these pages read, and show how to ask a public node for a share account\'s keys. You can run this tour again any time with Take the tour.' },
];
