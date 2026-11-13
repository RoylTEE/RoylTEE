/**
 * Split sheet totals per work, right and territory (second reviewer 03d item 4). The registry caps
 * each (work, right, territory) group at 100 percent, so totals are kept per group, never per right only.
 */
export type GroupRow = { right_type: string; territory: string; percent_bps: number; status: string };
export function groupTotals(rows: GroupRow[]): { right: string; territory: string; bps: number }[] {
  const m = new Map<string, { right: string; territory: string; bps: number }>();
  for (const r of rows) {
    if (r.status !== 'active') continue;
    const k = `${r.right_type}|${r.territory}`;
    const g = m.get(k) ?? { right: r.right_type, territory: r.territory, bps: 0 };
    g.bps += r.percent_bps; m.set(k, g);
  }
  return [...m.values()];
}
