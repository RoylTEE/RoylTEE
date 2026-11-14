import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { loadConfig } from '../src/server/config';
import { originCheck } from '../src/server/security';

const root = join(__dirname, '..');
const sites = ['https://royltee.vercel.app', 'https://royltee.app'];
const req = (origin: string, at = origin) => new Request(`${at}/api/chat`, { method: 'POST', headers: { origin, 'content-type': 'application/json' } });

describe('allowed origins for the custom domain', () => {
  it('the example environment file lists both site origins and the setup note names the custom domain', () => {
    const env = readFileSync(join(root, '.env.example'), 'utf8');
    for (const o of sites) expect(env).toContain(o);
    expect(readFileSync(join(root, 'README.md'), 'utf8')).toContain('https://royltee.vercel.app,https://royltee.app');
  });
  it('with both origins set, the custom domain passes and a look-alike is refused', () => {
    const allowed = loadConfig({ ALLOWED_ORIGINS: sites.join(',') }).allowedOrigins;
    expect(allowed).toEqual(sites);
    expect(() => originCheck(req('https://royltee.app'), allowed)).not.toThrow();
    expect(() => originCheck(req('https://royltee.vercel.app'), allowed)).not.toThrow();
    expect(() => originCheck(req('https://royltee.app.evil.example', 'https://royltee.app'), allowed)).toThrow();
    expect(() => originCheck(req('https://evil.example', 'https://royltee.app'), allowed)).toThrow();
  });
  it('with no list set nothing is allowed, so the API stays closed until the setting exists', () => {
    expect(loadConfig({}).allowedOrigins).toEqual([]);
    expect(() => originCheck(req('https://royltee.app'), [])).toThrow();
  });
  it('the page security policy has no host list of its own for the site, it uses self', () => {
    const vercel = JSON.parse(readFileSync(join(root, 'vercel.json'), 'utf8'));
    const csp = JSON.stringify(vercel);
    expect(csp).toContain("default-src 'self'");
    expect(csp).not.toContain('royltee.app');
  });
});
