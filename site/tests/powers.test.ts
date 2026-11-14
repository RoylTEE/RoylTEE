import { describe, expect, it } from 'vitest';
import { readFileSync, existsSync } from 'node:fs';
import { POWERS, CLI_ONLY } from '../src/data/powers';

const src = (rel: string) => readFileSync(new URL(`../src/${rel}`, import.meta.url), 'utf8');

describe('every power has a screen or says it is CLI only', () => {
  it('a power with a screen links to a real page and tab panel', () => {
    for (const p of POWERS.filter((x) => x.ui)) {
      const [path, hash] = p.ui!.split('#');
      expect(existsSync(new URL(`../src/pages${path}.astro`, import.meta.url)), p.method).toBe(true);
      if (hash) expect(src(`pages${path}.astro`), p.method).toMatch(new RegExp(`id="${hash}"`));
    }
  });
  it('the table renders the CLI only label for every power without a screen', () => {
    expect(CLI_ONLY).toBe('CLI only in this demo');
    expect(src('components/PowerTable.astro')).toMatch(/p\.ui \? <a class="link" href=\{p\.ui\}>Open<\/a> : <span class="badge cli">\{CLI_ONLY\}<\/span>/);
    expect(POWERS.filter((x) => !x.ui).length).toBeGreaterThan(5);
  });
  it('holder, organisation and registrar pages show their powers', () => {
    expect(src('pages/share.astro')).toMatch(/<PowerTable who="holder" \/>/);
    expect(src('pages/agents.astro')).toMatch(/<PowerTable who="organisation" \/>/);
    expect(src('pages/registrar.astro')).toMatch(/<PowerTable who="registrar" \/>/);
    expect(src('pages/docs.astro')).toMatch(/<PowerTable who="payer" \/>/);
  });
  it('payer powers carry no repeated Simulated label and the draft stays UNVERIFIED', () => {
    for (const p of POWERS.filter((x) => x.who === 'payer')) expect(p.note).toBeUndefined();
    expect(POWERS.find((x) => x.method === 'register_share')!.note).toBe('UNVERIFIED');
  });
});
