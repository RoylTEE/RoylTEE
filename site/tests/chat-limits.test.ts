import { describe, expect, it } from 'vitest';
import { setup } from './helpers';
import { chat, startChat, chatLimits, budgetDay, type Turn } from '../src/server/chat';
const today = () => budgetDay(Date.now());
import { MemoryStore } from '../src/server/store';
import { loadConfig } from '../src/server/config';
import { clientKey } from '../src/server/security';

const Q = 'How do royalty shares work?';
const status = async (p: Promise<unknown>) => { try { await p; return 200; } catch (e) { return (e as { status?: number }).status ?? 500; } };
function direct() {
  const store = new MemoryStore();
  const config = loadConfig({ CHAT_HMAC_SECRET: 'x'.repeat(40), SESSION_SECRET: 'y'.repeat(40), ALLOWED_ORIGINS: 'https://t.example', NEARAI_API_KEY: 'test-only' });
  const model = { calls: 0 };
  const fake = async () => { model.calls++; return 'Royalty shares are split by bps.'; };
  let n = 0;
  const fresh = async () => { const id = (n++).toString(16).padStart(48, 'd'); await startChat(store, id); return id; };
  const ask = async (ip: string, wallet: string | null = null, id?: string, history: Turn[] = []) => status(chat(store, config, id ?? await fresh(), { history, message: Q }, ip, fake, Date.now(), wallet));
  return { store, config, model, fake, fresh, ask };
}

describe('F-16 per-client limits run before shared daily budgets', () => {
  it('orders limits narrowest first and keys none of them on the chat session', () => {
    const { config } = direct();
    expect(chatLimits(config, 'ipk', null).map(l => l.id)).toEqual(['chat:ip:ipk', `chat:global:anon:${today()}`, `chat:global:${today()}`]);
    expect(chatLimits(config, 'ipk', 'w.testnet').map(l => l.id)).toEqual(['chat:ip:ipk', 'chat:wallet:w.testnet', `chat:global:${today()}`]);
  });

  it('PoC: one anonymous client sending 700 requests cannot lock out other anonymous clients', async () => {
    const d = direct();
    let served = 0;
    for (let i = 0; i < 700; i++) if (await d.ask('attacker') === 200) served++;
    expect(served).toBe(d.config.chatDayCap);
    expect(d.model.calls).toBe(d.config.chatDayCap);
    expect(await d.store.get(`rate:chat:global:anon:${today()}`)).toBe(d.config.chatDayCap);
    expect(await d.store.get(`rate:chat:global:${today()}`)).toBe(d.config.chatDayCap);
    expect(await d.store.get('rate:chat:ip:attacker')).toBe(d.config.chatDayCap);
    expect(await d.ask('honest-anon')).toBe(200);
  });

  it('PoC: 560 replayed 429s on one session after the cap cost no shared budget', async () => {
    const d = direct();
    for (let k = 0; k < 4; k++) {
      const id = await d.fresh(); let history: Turn[] = [];
      for (let i = 0; i < 10; i++) {
        const r = await chat(d.store, d.config, id, { history, message: Q }, 'attacker', d.fake);
        history = [...history, { role: 'user', text: Q }, r.assistant];
      }
    }
    expect(d.model.calls).toBe(40);
    const id = await d.fresh(); let refused = 0;
    for (let i = 0; i < 560; i++) if (await d.ask('attacker', null, id) === 429) refused++;
    expect(refused).toBe(560);
    expect(d.model.calls).toBe(40);
    expect(await d.store.get(`rate:chat:global:anon:${today()}`)).toBe(40);
    expect(await d.store.get(`rate:chat:global:${today()}`)).toBe(40);
    expect(await d.ask('honest-anon')).toBe(200);
  });

  it('PoC: one signed-in client cannot drain the signed-in reserve or the global cap', async () => {
    const d = direct();
    for (let i = 0; i < 900; i++) await d.ask('attacker', 'attacker.testnet');
    expect(d.model.calls).toBe(d.config.chatWalletDayCap);
    expect(await d.store.get(`rate:chat:global:${today()}`)).toBe(d.config.chatWalletDayCap);
    expect(await d.ask('honest', 'honest.testnet')).toBe(200);
  });

  it('caps one wallet per day across many addresses, and other wallets still get served', async () => {
    const d = direct();
    let served = 0;
    for (let i = 0; i < 60; i++) if (await d.ask(`addr-${i}`, 'farm.testnet') === 200) served++;
    expect(served).toBe(d.config.chatWalletDayCap);
    for (let i = 0; i < 60; i++) expect(await d.store.get(`rate:chat:ip:addr-${i}`)).toBe(i < d.config.chatWalletDayCap ? 1 : 0);
    expect(await d.ask('addr-0', 'other.testnet')).toBe(200);
  });

  it('a turn refused by a shared budget gives back the per-client charges', async () => {
    const d = direct();
    const config = loadConfig({ CHAT_HMAC_SECRET: 'x'.repeat(40), CHAT_GLOBAL_DAY_CAP: '3', CHAT_SIGNED_RESERVE: '1' });
    const ask = async (ip: string, wallet: string | null = null) => status(chat(d.store, config, await d.fresh(), { history: [], message: Q }, ip, d.fake, Date.now(), wallet));
    expect(await ask('a')).toBe(200);
    expect(await ask('b')).toBe(200);
    expect(await ask('c')).toBe(429);
    expect(await d.store.get('rate:chat:ip:c')).toBe(0);
    expect(await d.store.get(`rate:chat:global:anon:${today()}`)).toBe(2);
    expect(await ask('d', 'w.testnet')).toBe(200);
    expect(await ask('e', 'w.testnet')).toBe(429);
    expect(await d.store.get('rate:chat:ip:e')).toBe(0);
    expect(await d.store.get('rate:chat:wallet:w.testnet')).toBe(1);
    expect(await d.store.get(`rate:chat:global:${today()}`)).toBe(3);
  });

  it('the per-client day cap survives new chat sessions and covers the whole IPv6 /64', async () => {
    const t = setup({ CHAT_DAY_CAP: '3' });
    const send = async (address: string) => {
      const s = await t.call('chat/start', {}, { address });
      return (await t.call('chat', { sessionId: s.body.sessionId, history: [], message: Q }, { address })).status;
    };
    for (let i = 1; i <= 3; i++) expect(await send(`2001:db8:0:7::${i}`)).toBe(200);
    for (let i = 4; i <= 12; i++) expect(await send(`2001:db8:0:7::${i.toString(16)}`)).toBe(429);
    expect(t.modelCalls).toHaveLength(3);
    expect(await t.store.get(`rate:chat:ip:${clientKey('2001:db8:0:7::1', t.config.sessionSecret)}`)).toBe(3);
    expect(await t.store.get(`rate:chat:global:${budgetDay(t.clock.now)}`)).toBe(3);
    expect(await send('2001:db8:0:8::1')).toBe(200);
  });
});
