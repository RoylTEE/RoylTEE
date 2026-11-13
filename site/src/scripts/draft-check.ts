/**
 * Review 03d M2. The deployed registry treats a register_share that takes a group over 100 percent
 * as a dispute against every active share of the group when no contests list is sent. A draft never
 * means to open a dispute, so the page re-reads each group right before signing and refuses if the
 * draft would take it over 100 percent. Pending (reserved) and claimant basis points count too.
 */
export type GroupTotals = { active_bps: number; reserved_bps: number; claimant_bps?: number } | null;
export const groupTotal = (g: GroupTotals) => (g ? g.active_bps + g.reserved_bps + (g.claimant_bps ?? 0) : 0);
export function overLimit(g: GroupTotals, draftBps: number): number | null {
  const total = groupTotal(g) + draftBps;
  return total > 10000 ? total : null;
}
