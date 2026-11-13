/**
 * Review 03d L1: say what happened on chain, not only that the wallet returned. A transaction whose
 * status or any receipt is a Failure is reported as failed; a wallet that returns no outcome is
 * reported as unconfirmed. Only a status that carries SuccessValue or SuccessReceiptId, on the
 * transaction and on every receipt, counts as confirmed. A pending or unknown status (for example
 * "NotStarted", "Started" or a receipt "Unknown") is reported as not confirmed (audit F2).
 */
type Status = { SuccessValue?: string; SuccessReceiptId?: string; Failure?: unknown } | string | undefined;
type Outcome = { status?: Status; receipts_outcome?: { outcome?: { status?: Status } }[] } | null | undefined;

const failed = (s: Status) => !!s && typeof s === 'object' && 'Failure' in s;
const succeeded = (s: Status) => !!s && typeof s === 'object' && ('SuccessValue' in s || 'SuccessReceiptId' in s);
const shown = (s: Status) => String(typeof s === 'string' ? s : JSON.stringify(s)).slice(0, 40);
function reason(o: Outcome): string {
  const all = [o?.status, ...(o?.receipts_outcome ?? []).map((r) => r.outcome?.status)];
  const f = all.find(failed) as { Failure: unknown } | undefined;
  const text = JSON.stringify(f?.Failure ?? '');
  const m = text.match(/Smart contract panicked: ([^"\\]+)/);
  return m ? m[1] : text.match(/"([A-Za-z]+)"/)?.[1] ?? 'see the explorer';
}
export function outcomeText(out: Outcome[] | Outcome | void, label: string, okNote = ''): string {
  const list = (Array.isArray(out) ? out : [out]).filter((o) => o != null) as Exclude<Outcome, null | undefined>[];
  if (!list.length) return `${label} sent, but the wallet returned no result, so success is not confirmed. Refresh to check.`;
  const bad = list.filter((o) => failed(o.status) || (o.receipts_outcome ?? []).some((r) => failed(r.outcome?.status)));
  if (bad.length) return `${label} failed on chain (${bad.length} of ${list.length}): ${reason(bad[0])}.`;
  if (list.some((o) => !o.status)) return `${label} sent, but the wallet returned no status, so success is not confirmed. Refresh to check.`;
  const all = list.flatMap((o) => [o.status, ...(o.receipts_outcome ?? []).map((r) => r.outcome?.status)]);
  const i = all.findIndex((st) => !succeeded(st));
  if (i >= 0) return `${label} sent, but the chain reports it as not final (${all[i] === undefined ? 'no receipt status' : shown(all[i])}), so success is not confirmed. Refresh to check.`;
  return `${label} confirmed on chain.${okNote ? ' ' + okNote : ''}`;
}
