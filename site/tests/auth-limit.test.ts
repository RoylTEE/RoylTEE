import { describe, expect, it } from 'vitest';
import { setup } from './helpers';

describe('sign-in challenge limit', () => {
  it('allows 30 challenges per client address in 10 minutes, then 429, then resets', async () => {
    const t = setup();
    for (let i = 0; i < 30; i++) expect((await t.call('auth/challenge', {}, { address: '198.51.100.77' })).status).toBe(200);
    expect((await t.call('auth/challenge', {}, { address: '198.51.100.77' })).status).toBe(429);
    expect((await t.call('auth/challenge', {}, { address: '198.51.100.78' })).status).toBe(200);
    t.clock.now += 600_000 + 1;
    expect((await t.call('auth/challenge', {}, { address: '198.51.100.77' })).status).toBe(200);
  });

  it('counts one IPv6 /64 as one client', async () => {
    const t = setup();
    for (let i = 1; i <= 30; i++) expect((await t.call('auth/challenge', {}, { address: `2001:db8:0:9::${i.toString(16)}` })).status).toBe(200);
    expect((await t.call('auth/challenge', {}, { address: '2001:db8:0:9::ffff' })).status).toBe(429);
  });

  it('when the platform gives no client address, every caller shares one bucket and is limited', async () => {
    const t = setup();
    const deps = t.deps as { address?: string };
    delete deps.address;
    for (let i = 0; i < 30; i++) expect((await t.call('auth/challenge', {}, { address: undefined as unknown as string })).status).toBe(200);
    expect((await t.call('auth/challenge', {}, { address: undefined as unknown as string })).status).toBe(429);
  });
});
