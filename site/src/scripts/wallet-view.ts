/** What a wallet can read from the standard NFT views for one share. Pure, so it can be tested. */
import type { Tone } from './ui';

type Meta = { title?: string | null; description?: string | null; media?: string | null; reference?: string | null };
export function walletFields(token: Record<string, unknown>, contract: Record<string, unknown>): [string, string, Tone?][] {
  const m = (token.metadata ?? {}) as Meta;
  return [
    ['Collection', `${contract.name ?? 'none'} (${contract.symbol ?? 'no symbol'})`],
    ['Standard', String(contract.spec ?? 'not given')],
    ['Title', m.title ?? 'none'],
    ['Description', m.description ?? 'none'],
    ['Image', m.media ? 'Has media' : 'No media', m.media ? 'warn' : 'plain'],
    ['Collection icon', contract.icon ? 'Has icon' : 'No icon', 'plain'],
    ['Owner', String(token.owner_id ?? 'none')],
    ['Marketplace approvals', token.approved_account_ids == null ? 'Disabled' : 'Present', token.approved_account_ids == null ? 'ok' : 'warn'],
  ];
}
