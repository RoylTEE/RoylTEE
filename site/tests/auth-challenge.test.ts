import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { setup } from './helpers';

const A = 'writer-a.royltee-dev.testnet';

describe('F-20 sign-in challenge cookie', () => {
  it('still signs in when the client address changes between challenge and verify', async () => {
    const t = setup();
    const w = await t.wallet(A);
    const ch = await t.call('auth/challenge', {}, { address: '198.51.100.10' });
    expect(ch.status).toBe(200);
    expect(ch.cookie).toMatch(/^__Host-royltee_challenge=/);
    expect(ch.cookie).toMatch(/HttpOnly; SameSite=Strict; Path=\/; Max-Age=300; Secure/);
    expect(ch.cookie).not.toMatch(/Domain=/);
    const r = await t.call('auth/verify', await w.sign(ch.body), { address: '2001:db8:aa:bb::7' });
    expect(r.status).toBe(200);
    expect(r.body.accountId).toBe(A);
    expect(r.cookie).toMatch(/^__Host-royltee_support=/);
    expect(r.cookie).toMatch(/__Host-royltee_challenge=; HttpOnly; SameSite=Strict; Path=\/; Max-Age=0; Secure/);
  });

  it('rejects verify when the challenge cookie is missing or wrong, without using up the nonce', async () => {
    const t = setup();
    const w = await t.wallet(A);
    const ch = await t.call('auth/challenge', {});
    const signed = await w.sign(ch.body);
    const missing = await t.call('auth/verify', signed, { cookie: '' });
    expect(missing.status).toBe(400);
    expect(missing.body.error).toBe('That sign in request was used or expired. Please try again.');
    const wrong = await t.call('auth/verify', signed, { cookie: `__Host-royltee_challenge=${'ab'.repeat(24)}` });
    expect(wrong.status).toBe(400);
    expect(wrong.body.error).toBe('That sign in request was used or expired. Please try again.');
    expect((await t.call('auth/verify', signed)).status).toBe(200);
  });

  it('rejects a replay even when the original challenge cookie is sent again', async () => {
    const t = setup();
    const w = await t.wallet(A);
    const ch = await t.call('auth/challenge', {});
    const signed = await w.sign(ch.body);
    const cookie = ch.cookie!.split(';')[0];
    expect((await t.call('auth/verify', signed)).status).toBe(200);
    const replay = await t.call('auth/verify', signed, { cookie });
    expect(replay.status).toBe(400);
    expect(replay.body.error).toBe('That sign in request was used or expired. Please try again.');
  });

  it('sends the challenge cookie automatically because site fetches use same-origin credentials', () => {
    const src = readFileSync(new URL('../src/scripts/wallet.ts', import.meta.url), 'utf8');
    expect(src).toMatch(/fetch\(path, \{ method: init\.method \?\? 'GET', credentials: 'same-origin'/);
  });
});
