/**
 * The registry app map: areas grouped by purpose, each with its own URL and its tabs.
 * The sidebar, the mobile menu, the breadcrumb and the area tabs all read this one list.
 * Tab links with a hash switch panels on the same page (src/scripts/shell.ts).
 * Not in the menu: the share detail page (/share?id=, reached from any share row) and /admin (support owner only).
 */
export type Tab = { key: string; label: string; href: string };
export type Area = { key: string; label: string; group: string; purpose: string; href: string; tabs: Tab[] };

export const GROUPS = ['Registry', 'Oversight', 'You', 'Support'] as const;

export const AREAS: Area[] = [
  { key: 'overview', label: 'Overview', group: 'Registry', purpose: 'Statement and totals', href: '/dashboard/', tabs: [] },
  { key: 'catalog', label: 'Catalog', group: 'Registry', purpose: 'Works, split sheets, lookup', href: '/catalog/', tabs: [
    { key: 'works', label: 'Works and rights', href: '/catalog/' },
    { key: 'splits', label: 'Split sheets', href: '/catalog/splits/' },
    { key: 'lookup', label: 'Lookup', href: '/catalog/lookup/' },
  ] },
  { key: 'shares', label: 'Shares', group: 'Registry', purpose: 'Ledger, share accounts and claims', href: '/shares/', tabs: [
    { key: 'ledger', label: 'Ledger', href: '/shares/' },
    { key: 'accounts', label: 'Share accounts', href: '/shares/accounts/' },
    { key: 'claims', label: 'Claims', href: '/shares/claims/' },
  ] },
  { key: 'activity', label: 'Activity', group: 'Registry', purpose: 'Recorded on-chain history', href: '/activity', tabs: [
    { key: 'all', label: 'All', href: '/activity#all' },
    { key: 'payments', label: 'Payments', href: '/activity#payments' },
    { key: 'claims', label: 'Claims', href: '/activity#claims' },
    { key: 'sale', label: 'Sale', href: '/activity#sale' },
    { key: 'agent', label: 'Agent', href: '/activity#agent' },
    { key: 'dispute', label: 'Dispute', href: '/activity#dispute' },
    { key: 'refused', label: 'Refused', href: '/activity#refused' },
  ] },
  { key: 'auction', label: 'Auction', group: 'Registry', purpose: 'Sealed-bid sale run in a TEE', href: '/auction', tabs: [] },
  { key: 'verify', label: 'Verify', group: 'Registry', purpose: 'Code hashes, keyless vaults, keys left', href: '/verify', tabs: [] },
  { key: 'agents', label: 'Agents', group: 'Oversight', purpose: 'Roles, limits and keys', href: '/agents', tabs: [] },
  { key: 'disputes', label: 'Disputes', group: 'Oversight', purpose: 'Frozen money and outcomes', href: '/disputes', tabs: [] },
  { key: 'registrar', label: 'Registrar log', group: 'Oversight', purpose: 'Members, powers and recorded actions', href: '/registrar', tabs: [
    { key: 'log', label: 'Log', href: '/registrar#log' },
    { key: 'members', label: 'Members', href: '/registrar#members' },
    { key: 'role', label: 'Role and powers', href: '/registrar#role' },
  ] },
  { key: 'member', label: 'Your shares', group: 'You', purpose: 'Claim, transfer, agents, approve, draft', href: '/member', tabs: [
    { key: 'holdings', label: 'Holdings', href: '/member#holdings' },
    { key: 'transfer', label: 'Transfer', href: '/member#transfer' },
    { key: 'org', label: 'Organisation agents', href: '/member#org' },
    { key: 'approvals', label: 'Approvals', href: '/member#approvals' },
    { key: 'draft', label: 'Draft a split sheet', href: '/member#draft' },
  ] },
  { key: 'help', label: 'Help', group: 'Support', purpose: 'Help chat and support tickets', href: '/help', tabs: [
    { key: 'chat', label: 'Help chat', href: '/help#chat' },
    { key: 'support', label: 'Support', href: '/help#support' },
  ] },
  { key: 'docs', label: 'Docs', group: 'Support', purpose: 'Accounts, methods, whitepaper', href: '/docs', tabs: [
    { key: 'docs', label: 'Docs', href: '/docs' },
    { key: 'whitepaper', label: 'Whitepaper', href: '/whitepaper' },
  ] },
  { key: 'how', label: 'How it works', group: 'Support', purpose: 'The project site', href: '/#how', tabs: [] },
];

export const area = (key: string) => AREAS.find((a) => a.key === key)!;
