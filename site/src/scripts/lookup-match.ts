/** Catalog lookup, pure: what a query matches across works and shares. Runs in the browser, there is no on-chain code index. */
import type { Token, Work } from './chain';

export type QueryKind = 'code' | 'share' | 'account' | 'text';
const norm = (s: string) => s.toUpperCase().replace(/[\s.-]/g, '');

export function kindOf(q: string): QueryKind {
  const t = q.trim();
  if (/^s\d{1,6}$/i.test(t)) return 'share';
  if (/\.(testnet|near)$/i.test(t) || /^[a-z0-9_-]+(\.[a-z0-9_-]+)+$/.test(t)) return 'account';
  if (/^[A-Z]{2}[A-Z0-9]{3}\d{7}$/.test(norm(t)) || /^T\d{10}$/.test(norm(t))) return 'code';
  return 'text';
}

export type Hit = { work: Work; tokens: Token[]; why: string };

export function lookup(q: string, works: Work[], tokens: Token[]): Hit[] {
  const t = q.trim();
  if (!t) return [];
  const kind = kindOf(t);
  const nq = norm(t), lq = t.toLowerCase();
  const of = (w: Work) => tokens.filter((x) => x.extra.work_id === w.id);
  const hits: Hit[] = [];
  for (const w of works) {
    const ts = of(w);
    if (kind === 'share') { const m = ts.filter((x) => x.token_id === lq); if (m.length) hits.push({ work: w, tokens: m, why: `Share ${lq}` }); continue; }
    if (kind === 'account') {
      const m = ts.filter((x) => x.owner_id === lq);
      if (m.length || w.registered_by === lq) hits.push({ work: w, tokens: m, why: m.length ? `Holds ${m.map((x) => x.token_id).join(', ')}` : 'Registered the work' });
      continue;
    }
    if (norm(w.code).includes(nq) || (kind === 'text' && /^\d+$/.test(t) && w.id === Number(t))) hits.push({ work: w, tokens: ts, why: norm(w.code) === nq ? `${w.code_type.toUpperCase()} match` : `${w.code_type.toUpperCase()} contains ${t}` });
  }
  return hits;
}

/** Who gets paid now for one right of a work: the current owner of each active share, in basis points. */
export function payees(shareIds: string[], tokens: Token[]) {
  return shareIds.map((id) => tokens.find((t) => t.token_id === id)).filter((t): t is Token => !!t)
    .map((t) => ({ token: t.token_id, payee: t.owner_id, bps: t.extra.percent_bps, status: t.extra.status, vault: t.extra.vault, paid: t.extra.status === 'active' && !t.extra.dispute }));
}
