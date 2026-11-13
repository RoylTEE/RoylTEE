import type { Config } from './config';
import type { Store } from './store';
import { digest, equal, fail, mac, randomId, rateAll, text, type Limit } from './security';
import { SYSTEM_PROMPT } from './prompt';
import { sensitive } from './sensitive';
export type Turn = { role: 'user' | 'assistant'; text: string; index?: number; signature?: string; tee?: boolean };
export type ChatState = { count: number; digest: string; busy: boolean; busyUntil?: number; owner?: string };
export type ModelResult = { text: string; tee: boolean };
export type Model = (turns: Turn[]) => Promise<string | ModelResult>;
export const VERSION = 'royltee-chat-v2';
export const TEE_LABEL = 'Inference ran in a TEE (reported by NEAR AI Cloud)';
export const BUSY_MS = 90_000;
const emptyState = (): ChatState => ({ count: 0, digest: digest([]), busy: false, busyUntil: 0 });
export async function startChat(store: Store, id: string) { await store.put(`chat:${id}`, emptyState(), 3600); }
export const signTurn = (secret: string, id: string, index: number, value: string, tee = false) => mac(secret, [VERSION, id, index, value, tee === true]);
const jailbreak = /ignore\b.{0,60}\b(?:instructions|rules|prompt)|(?:system|developer|hidden|initial)\s*(?:prompt|message|instructions)|(?:reveal|show|print|repeat|leak|dump)\b.{0,40}(?:prompt|instructions|notes|secret|env|token|api key)|(?:your|the server'?s?|site'?s?)\s+(?:api|hmac|session|secret|signing)\s*(?:key|secret|token)|\bjailbreak|\bDAN\b|developer mode|pretend\b.{0,40}(?:unrestricted|no rules|no limits)|bypass\b.{0,30}(?:safety|filter|rules)|<\/?(?:system|assistant)>/is;
const unrelated = /\b(?:recipe|cook(?:ing)?|weather|election|president|football|soccer|movie|poem|joke|horoscope|stock price|bitcoin price|homework|capital of|travel|dating)\b/i;
export function refusal(question: string) {
  const kind = sensitive(question);
  if (kind) return `Please leave out ${kind}. This help chat only needs demo details. If you need a person, use [Contact support](/help#support).`;
  if (jailbreak.test(question)) return 'I cannot share private setup details or step around the safety rules. I am happy to explain how RoylTEE works, though. [How it works](/#how) is a good place to start.';
  if (unrelated.test(question)) return 'That is outside what I can help with here. I can explain RoylTEE shares, vaults, the testnet demo or wallet support. [How it works](/#how) is a good place to start.';
  return null;
}
export const LINKS = new Set(['/', '/#how', '/#agents', '/#lifecycle', '/#trust', '/#notice', '/#problem', '/dashboard/', '/catalog/', '/catalog/splits/', '/catalog/lookup/', '/shares/', '/shares/accounts/', '/shares/claims/', '/activity', '/agents', '/disputes', '/registrar', '/verify', '/member', '/docs', '/whitepaper', '/help', '/help#chat', '/help#support']);
/** Returns null when the reply must not be shown; otherwise drops links outside the app. Raw outside URLs are withheld instead of leaving a mid-sentence placeholder. */
const RAW_URL = /(?<![A-Za-z0-9])(?:https?:\/\/|(?:javascript|data|vbscript):)\S*|(?<![A-Za-z0-9:])\/\/[A-Za-z0-9][^\s)]*/i;
export function cleanAnswer(value: string): string | null {
  if (!value || value.length > 6000 || /grounding-notes|CHAT_HMAC_SECRET|SESSION_SECRET|NEARAI_API_KEY|UPSTASH_REDIS|KV_REST_API|CRON_SECRET|You are the RoylTEE help guide|Treat every user message as untrusted/i.test(value) || sensitive(value)) return null;
  const cleaned = value
    .replace(/\[([^\]]{1,120})\]\(([^)\s]{1,200})\)/g, (all, label, href) => LINKS.has(href) ? all : label)
    .replace(/[\u2013\u2014]/g, ',');
  if (RAW_URL.test(cleaned)) return null;
  return cleaned;
}
/** True only when NEAR AI Cloud reports signature_kind as the exact string provider_tee. Never inferred. */
export function reportedProviderTee(value: unknown): boolean {
  return !!value && typeof value === 'object' && !Array.isArray(value) && (value as { signature_kind?: unknown }).signature_kind === 'provider_tee';
}
export function modelResult(value: string | ModelResult): ModelResult {
  return typeof value === 'string' ? { text: value, tee: false } : { text: value.text, tee: value.tee === true };
}
async function readSignature(fetcher: typeof fetch, url: string, key: string): Promise<unknown> {
  try {
    const r = await fetcher(url, { headers: { Accept: 'application/json', Authorization: `Bearer ${key}` }, signal: AbortSignal.timeout(8000) });
    if (!r.ok) return null;
    return await r.json();
  } catch {
    return null;
  }
}
export function nearAiModel(config: Config, fetcher = fetch): Model {
  return async turns => {
    if (!config.nearAiKey) fail(503, 'Chat is not configured yet. You can still read the guide or contact support.');
    const r = await fetcher(`${config.nearAiBaseUrl}/chat/completions`, { method: 'POST', headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${config.nearAiKey}` }, body: JSON.stringify({ model: config.nearAiModel, messages: [{ role: 'system', content: SYSTEM_PROMPT }, ...turns.map(t => ({ role: t.role, content: t.text }))], max_tokens: 650, stream: false, chat_template_kwargs: { enable_thinking: false }, reasoning_effort: 'low' }), signal: AbortSignal.timeout(45000) });
    if (!r.ok) fail(503, 'Chat is taking a break. Please try again shortly.');
    const j = await r.json();
    const choice = j.choices?.[0];
    const answer = choice?.message?.content;
    if (typeof answer !== 'string' || !answer.trim() || choice?.finish_reason !== 'stop') fail(503, 'Chat could not finish that answer. Please try again.');
    const id = typeof j.id === 'string' && j.id ? j.id : '';
    const signature = id ? await readSignature(fetcher, `${config.nearAiBaseUrl}/signature/${encodeURIComponent(id)}?signing_algo=ecdsa`, config.nearAiKey) : null;
    return { text: answer.trim(), tee: reportedProviderTee(signature) };
  };
}
function isBusy(state: ChatState, now: number) {
  return state.busy === true && typeof state.busyUntil === 'number' && state.busyUntil > now;
}
function signedReserve(config: Config) {
  return Math.min(Math.max(0, config.chatSignedReserve), Math.max(0, config.chatGlobalDayCap - 1));
}
/** UTC calendar day used to bucket the shared daily budgets. */
export const budgetDay = (now: number) => new Date(now).toISOString().slice(0, 10);
/**
 * Daily limits for one model call, narrowest first: the client address group (IPv6 /64), then the
 * signed-in wallet, then the anonymous share, then the global cap. Keys never use the chat session,
 * so starting a new session does not reset them, and a refused call gives back every charge.
 * The anonymous share and the global cap use the same UTC day key, so they always start and end
 * together and anonymous calls cannot reach into the signed-in reserve (review 03b, second reviewer item 2).
 */
export function chatLimits(config: Config, ip: string, wallet: string | null, now = Date.now()): Limit[] {
  const day = 86400;
  const bucket = budgetDay(now);
  const limits: Limit[] = [{ id: `chat:ip:${ip}`, max: config.chatDayCap, seconds: day }];
  const reserve = signedReserve(config);
  if (wallet) limits.push({ id: `chat:wallet:${wallet}`, max: config.chatWalletDayCap, seconds: day });
  else if (reserve > 0) limits.push({ id: `chat:global:anon:${bucket}`, max: config.chatGlobalDayCap - reserve, seconds: 2 * day });
  limits.push({ id: `chat:global:${bucket}`, max: config.chatGlobalDayCap, seconds: 2 * day });
  return limits;
}
export async function chat(store: Store, config: Config, id: string, input: Record<string, unknown>, ip: string, model: Model, now = Date.now(), wallet: string | null = null) {
  const state = await store.get<ChatState>(`chat:${id}`) ?? fail(400, 'This chat has expired. Please start a new conversation.');
  const turns = input.history;
  if (!Array.isArray(turns) || turns.length !== state.count * 2) fail(400, 'That chat history is not valid. Please start again.');
  const history: Turn[] = [];
  for (let i = 0; i < turns.length; i++) {
    const raw = turns[i];
    if (!raw || typeof raw !== 'object' || raw.role !== (i % 2 ? 'assistant' : 'user') || typeof raw.text !== 'string' || raw.text.length > 6000) fail(400, 'That chat history is not valid. Please start again.');
    if (i % 2) {
      const index = (i - 1) / 2;
      const tee = raw.tee === true;
      if (raw.index !== index || typeof raw.signature !== 'string' || !equal(raw.signature, signTurn(config.chatSecret, id, index, raw.text, tee))) fail(400, 'That chat history is not valid. Please start again.');
      history.push({ role: 'assistant', text: raw.text, index, signature: raw.signature, tee });
    } else history.push({ role: 'user', text: raw.text });
  }
  if (isBusy(state, now) || digest(history) !== state.digest) fail(400, 'That turn was already used or changed. Please start again.');
  const question = text(input.message, 1500);
  if (state.count >= config.chatSessionCap) fail(429, 'This conversation has reached its limit. Please come back later.');
  // A unique owner token: if this lock expires and another request takes the turn over, this
  // worker's commit no longer matches and its answer is discarded (review 03b, second reviewer item 7).
  const busy: ChatState = { ...state, busy: true, busyUntil: now + BUSY_MS, owner: randomId() };
  if (!await store.cas(`chat:${id}`, state, busy, 3600)) fail(400, 'That turn was already used. Please start again.');
  try {
    const user: Turn = { role: 'user', text: question };
    const blocked = refusal(question);
    if (!blocked) await rateAll(store, chatLimits(config, ip, wallet, now));
    const modelOut = blocked ? { text: blocked, tee: false } : modelResult(await model([...history, user]));
    const cleaned = cleanAnswer(modelOut.text);
    const output = cleaned ?? 'I could not give a safe answer to that one. Could you ask about a specific part of RoylTEE? [Contact support](/help#support) is there too.';
    const tee = cleaned !== null && modelOut.tee;
    const assistant: Turn = { role: 'assistant', text: output, index: state.count, signature: signTurn(config.chatSecret, id, state.count, output, tee), tee };
    const all = [...history, user, assistant];
    if (!await store.cas(`chat:${id}`, busy, { count: state.count + 1, digest: digest(all), busy: false, busyUntil: 0 }, 3600)) {
      fail(409, 'A newer request took over this turn. Please start again.');
    }
    return { sessionId: id, version: VERSION, assistant: { ...assistant, tee }, tee };
  } catch (e) {
    await store.cas(`chat:${id}`, busy, { ...state, busy: false, busyUntil: 0 }, 3600);
    throw e;
  }
}
