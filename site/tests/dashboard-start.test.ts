import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { SCENES, MINTS } from '../src/data/testnet';
import { nearDoes, allLabelled, SPLIT_SCENE } from '../src/data/scene-labels';
import { forbidden } from '../scripts/wording.mjs';

const root = join(__dirname, '..');
const read = (f: string) => readFileSync(join(root, f), 'utf8');

describe('dashboard Start here and scene labels', () => {
  it('every recorded scene has a NEAR does line, with no dash and in plain words', () => {
    expect(allLabelled()).toBe(true);
    for (const s of SCENES) {
      const t = nearDoes(s);
      expect(t.length, s.what).toBeGreaterThan(20);
      expect(t, s.what).not.toMatch(/[\u2013\u2014]/);
      expect(t.endsWith('.'), s.what).toBe(true);
    }
  });
  it('the special scenes say what is specific to them', () => {
    const by = (hash: string) => SCENES.find((s) => s.txs[0] === hash)!;
    expect(nearDoes(by('5dgncSm1CyDuiEKRzeUpZJTWrBnHzNuuEP6k51RNseGa'))).toContain('releases the held payment');
    expect(nearDoes(by('8y7g9RfonZSfhBHKvRVAiYytyK1zEeo2zp5kgJV4cokL'))).toContain('escrow');
    expect(nearDoes(by('CATqXVEauHtQ1mpFHQo7VxQdj9fCwnEQQUFGh9NK5esW'))).toContain('TEE');
  });
  it('the split sheet scene is work 6, shares s14 and s15, and its transactions are the recorded ones', () => {
    expect(SPLIT_SCENE.workId).toBe(6);
    expect([...SPLIT_SCENE.shares]).toEqual(['s14', 's15']);
    const recorded = new Set(SCENES.flatMap((s) => s.txs));
    const evidence = JSON.parse(read('src/data/evidence.json'));
    for (const st of SPLIT_SCENE.steps) for (const h of st.txs) {
      expect(recorded.has(h), h).toBe(true);
      expect(evidence.txs[h], h).toBeTruthy();
    }
    expect(SPLIT_SCENE.steps.flatMap((s) => s.txs)).toContain(MINTS.s14.tx);
    expect(SPLIT_SCENE.steps.flatMap((s) => s.txs)).toContain(MINTS.s15.tx);
    expect(SCENES.find((s) => s.txs[0] === SPLIT_SCENE.steps[3].txs[0])!.step).toBe('Approve');
  });
  it('the Overview page has the three line Start here strip, the live split sheet table and explorer links', () => {
    const page = read('src/pages/dashboard.astro');
    expect(page).toContain('Start here');
    expect(page).toContain('What this is.');
    expect(page).toContain('The one click to try.');
    expect(page).toContain('What is Simulated.');
    expect(page).toContain('href="/share?id=s15"');
    expect(page).toContain('data-split-rows');
    expect(page).toContain('txUrl(h)');
    expect(forbidden(page)).toEqual([]);
  });
  it('the Overview script reads both shares live and shows their approval, vault balance and escrow', () => {
    const js = read('src/scripts/overview.ts');
    expect(js).toContain('SPLIT_SCENE.shares.map');
    expect(js).toContain('oneToken(id)');
    expect(js).toContain('approvalOf(t)');
    expect(js).toContain('t.extra.escrowed');
  });
  it('the Activity table shows the NEAR does line for each scene', () => {
    expect(read('src/components/ActivityTable.astro')).toContain('NEAR does:');
  });
});
