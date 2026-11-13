import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { createTicket, listTickets } from '../src/server/tickets';
import { MemoryStore } from '../src/server/store';
import { SCENES } from '../src/data/testnet';

describe('review 03b L2: ticket paging survives a cursor ticket that disappears', () => {
  it('keeps walking older tickets when the cursor ticket expired between pages', async () => {
    const clock = { now: 2_000_000 };
    const store = new MemoryStore(() => clock.now);
    for (let i = 0; i < 10; i++) {
      await createTicket(store, `w${i}.royltee-dev.testnet`, { subject: `S${i}`, message: 'demo ticket text' }, clock.now + i);
    }
    const first = await listTickets(store, null, clock.now + 100, null, 3);
    expect(first.tickets.map(t => t.subject)).toEqual(['S9', 'S8', 'S7']);
    await store.del(`ticket:${first.tickets[2].id}`);
    const second = await listTickets(store, null, clock.now + 100, first.cursor, 3);
    expect(second.tickets.map(t => t.subject)).toEqual(['S6', 'S5', 'S4']);
    expect(second.cursor).toBeTruthy();
  });

  it('orders ties on the same millisecond by id and never repeats or drops one', async () => {
    const clock = { now: 3_000_000 };
    const store = new MemoryStore(() => clock.now);
    for (let i = 0; i < 7; i++) await createTicket(store, `t${i}.royltee-dev.testnet`, { subject: `T${i}`, message: 'demo ticket text' }, clock.now);
    const seen: string[] = [];
    let cursor: string | null = null;
    do {
      const page = await listTickets(store, null, clock.now + 1, cursor, 2);
      seen.push(...page.tickets.map(t => t.id));
      cursor = page.cursor;
    } while (cursor);
    expect(new Set(seen).size).toBe(7);
    expect(seen.length).toBe(7);
  });
});

describe('review 03c L3: the agent payment is not labelled Simulated payer', () => {
  it('marks the agent scene as a scripted agent', () => {
    const agent = SCENES.filter(s => s.step === 'Agent' && s.what.startsWith('Agent pays'));
    expect(agent.length).toBe(1);
    for (const s of agent) { expect(s.simulated).toBeFalsy(); expect(s.agent).toBe(true); }
  });
});

describe('review 03c M2: power claims match the deployed build', () => {
  const read = (p: string) => readFileSync(new URL(`../src/${p}`, import.meta.url), 'utf8');
  it('does not say every registrar action has a document hash', () => {
    for (const f of ['pages/registrar.astro', 'pages/index.astro', 'content/help-notes.md']) {
      expect(read(f)).not.toMatch(/[Ee]very registrar action (is an event with|carries) a document hash/);
    }
  });
  it('marks branch-only split sheet approval and the registrar credential refusal', () => {
    expect(read('pages/registrar.astro')).toMatch(/Hold a credential \(after the planned registry update/);
    expect(read('pages/member.astro')).toMatch(/Not on the deployed build yet/);
    expect(read('pages/index.astro')).toMatch(/Change a split alone \(except the registrar/);
  });
});
