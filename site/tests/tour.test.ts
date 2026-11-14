import { describe, it, expect } from 'vitest';
import { existsSync, readFileSync } from 'node:fs';
import { TOUR } from '../src/data/tour';
import { forbidden } from '../scripts/wording.mjs';

const src = (p: string) => readFileSync(new URL(`../src/${p}`, import.meta.url), 'utf8');
const pageFor = (path: string) => {
  const p = path.replace(/\/+$/, '');
  if (p === '/dashboard') return 'pages/dashboard.astro';
  for (const f of [`pages${p}.astro`, `pages${p}/index.astro`]) if (existsSync(new URL(`../src/${f}`, import.meta.url))) return f;
  return null;
};

describe('guided tour steps', () => {
  it('has unique ids and a sensible length', () => {
    expect(new Set(TOUR.map((s) => s.id)).size).toBe(TOUR.length);
    expect(TOUR.length).toBeGreaterThanOrEqual(15);
    expect(TOUR.length).toBeLessThanOrEqual(30);
  });
  it('every step names a real page and has at least one target', () => {
    for (const s of TOUR) { expect(pageFor(s.path), s.id).not.toBeNull(); expect(s.target.length, s.id).toBeGreaterThan(0); }
  });
  it('plain copy: 1 to 3 sentences, no long dashes, no banned wording, no testnet or mock repeats, no figures', () => {
    for (const s of TOUR) {
      const sentences = s.body.split(/(?<=[.!?])\s+/).filter(Boolean);
      expect(sentences.length, s.id).toBeGreaterThanOrEqual(1);
      expect(sentences.length, s.id).toBeLessThanOrEqual(3);
      expect(forbidden(`${s.title} ${s.body}`), s.id).toEqual([]);
      expect(`${s.title} ${s.body}`, s.id).not.toMatch(/testnet|mock usdc|simulated/i);
      expect(s.body, s.id).not.toMatch(/\b\d+(\.\d+)?\s*(mUSDC|NEAR|%)/);
    }
  });
  it('action steps name what to click; only the member lookup is prefilled', () => {
    for (const s of TOUR.filter((x) => x.action)) expect(s.action!.label.length, s.id).toBeGreaterThan(0);
    expect(TOUR.filter((s) => s.action?.prefill).map((s) => s.id)).toEqual(['member']);
  });
  it('covers every area of the app menu except How it works', () => {
    const covered = new Set(TOUR.map((s) => s.path.replace(/\/+$/, '')));
    for (const p of ['/dashboard', '/catalog', '/shares', '/activity', '/auction', '/verify', '/agents', '/disputes', '/registrar', '/member', '/help', '/docs']) expect(covered.has(p), p).toBe(true);
  });
  it('never touches the wallet or sends anything', () => {
    const js = src('scripts/tour.ts');
    expect(js).not.toMatch(/innerHTML|outerHTML|insertAdjacentHTML|eval\(|signAndSend|\bconnect\(|\[data-connect\]|\[data-sign-in\]|fetch\(|XMLHttpRequest/);
    expect(TOUR.flatMap((s) => s.target).join(' ')).not.toMatch(/data-connect|data-sign-in/);
  });
  it('is loaded by the app shell from an external module, with a Take the tour button in the layout', () => {
    expect(src('scripts/shell.ts')).toMatch(/import \{ initTour \} from '\.\/tour\.ts'/);
    expect(src('layouts/App.astro')).toMatch(/data-tour-start/);
  });
});
