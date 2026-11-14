import { api, el, signIn, signOut } from './wallet';
type Turn = { role: 'user' | 'assistant'; text: string; index?: number; signature?: string; tee?: boolean };
const allowed = new Set(['/', '/#how', '/#agents', '/#lifecycle', '/#trust', '/#notice', '/#problem', '/dashboard/', '/catalog/', '/catalog/splits/', '/catalog/lookup/', '/shares/', '/shares/accounts/', '/shares/claims/', '/activity', '/agents', '/disputes', '/registrar', '/verify', '/auction', '/member', '/docs', '/whitepaper', '/help', '/help#chat', '/help#support']);
function render(text: string) {
  const p = el('p'); let last = 0;
  for (const m of text.matchAll(/\[([^\]]+)\]\(([^)]+)\)/g)) {
    p.append(text.slice(last, m.index));
    p.append(allowed.has(m[2]) ? el('a', { href: m[2], textContent: m[1] }) : m[1]);
    last = (m.index ?? 0) + m[0].length;
  }
  p.append(text.slice(last)); return p;
}
const log = document.querySelector<HTMLElement>('[data-chat-log]')!;
const form = document.querySelector<HTMLFormElement>('[data-chat-form]')!;
const input = form.querySelector<HTMLTextAreaElement>('textarea')!;
const status = document.querySelector<HTMLElement>('[data-chat-status]')!;
let sessionId = ''; let history: Turn[] = [];
function show(turn: Turn) {
  const node = el('div', { className: `turn ${turn.role}` }, el('span', { className: 'who', textContent: turn.role === 'user' ? 'You' : 'Guide' }), render(turn.text));
  if (turn.role === 'assistant' && turn.tee === true) node.append(el('p', { className: 'tee muted small', textContent: 'Inference ran in a TEE (reported by NEAR AI Cloud)' }));
  log.append(node); log.scrollTop = log.scrollHeight;
}
document.querySelector('[data-chat-reset]')?.addEventListener('click', () => { sessionId = ''; history = []; log.replaceChildren(); status.textContent = 'New conversation.'; });
form.addEventListener('submit', async e => {
  e.preventDefault(); const message = input.value.trim(); if (!message) return;
  form.querySelector('button')!.disabled = true; status.textContent = 'Thinking...';
  try {
    if (!sessionId) sessionId = (await api<{ sessionId: string }>('/api/chat/start', { method: 'POST', body: {} })).sessionId;
    const r = await api<{ assistant: Turn }>('/api/chat', { method: 'POST', body: { sessionId, history, message } });
    const user: Turn = { role: 'user', text: message }; show(user); show(r.assistant);
    history = [...history, user, r.assistant]; input.value = ''; status.textContent = '';
  } catch (err) { status.textContent = (err as Error).message; if ((err as { status?: number }).status === 400) { sessionId = ''; history = []; } }
  finally { form.querySelector('button')!.disabled = false; }
});
const box = document.querySelector<HTMLElement>('[data-support]')!;
const who = box.querySelector<HTMLElement>('[data-who]')!;
const list = box.querySelector<HTMLElement>('[data-tickets]')!;
const note = box.querySelector<HTMLElement>('[data-support-status]')!;
const ticketForm = box.querySelector<HTMLFormElement>('[data-ticket-form]')!;
let expiry = 0; let submitNonce = ''; let ticketCursor: string | null = null;
function signedIn(account: string | null) {
  box.dataset.state = account ? 'in' : 'out'; who.textContent = account ? `Signed in as ${account}` : '';
  if (!account) list.replaceChildren();
}
function ticketRow(t: { subject: string; message: string; status: string; reply: string | null }) {
  return el('li', { className: `ticket ${t.status}` },
    el('div', { className: 'ticket-head' }, el('strong', { textContent: t.subject }), el('span', { className: `badge ${t.status === 'closed' ? 'plain' : t.status === 'answered' ? 'ok' : 'warn'}`, textContent: t.status })),
    el('p', { textContent: t.message }), ...(t.reply ? [el('p', { className: 'reply', textContent: `Reply: ${t.reply}` })] : []));
}
function renderTickets(tickets: { subject: string; message: string; status: string; reply: string | null }[], append: boolean) {
  list.querySelector('[data-more]')?.remove();
  if (!append) {
    list.replaceChildren(...(tickets.length ? tickets.map(ticketRow) : [el('li', { className: 'muted', textContent: 'No tickets yet.' })]));
  } else if (tickets.length) {
    list.querySelector('.muted')?.remove();
    list.append(...tickets.map(ticketRow));
  }
  if (ticketCursor) {
    const more = el('li', { className: 'row' }, el('button', { type: 'button', className: 'btn sm', textContent: 'Load more tickets' }));
    more.setAttribute('data-more', '');
    more.querySelector('button')!.onclick = () => { void load(true); };
    list.append(more);
  }
}
async function load(more = false) {
  try {
    const q = more && ticketCursor ? `?cursor=${encodeURIComponent(ticketCursor)}` : '';
    const r = await api<{ accountId: string; submitNonce: string; cursor: string | null; tickets: { subject: string; message: string; status: string; reply: string | null; createdAt: number }[] }>(`/api/tickets${q}`);
    signedIn(r.accountId);
    if (!more) submitNonce = r.submitNonce;
    ticketCursor = r.cursor;
    renderTickets(r.tickets, more);
  } catch (err) { if (!more) signedIn(null); if ((err as { status?: number }).status !== 401) note.textContent = (err as Error).message; }
}
box.querySelector('[data-sign-in]')?.addEventListener('click', async () => {
  note.textContent = 'Waiting for your wallet...';
  try { const r = await signIn(); expiry = r.expiresAt; note.textContent = 'Signed in for 15 minutes.'; await load(); }
  catch (err) { note.textContent = (err as Error).message || 'Sign in was cancelled.'; }
});
box.querySelector('[data-sign-out]')?.addEventListener('click', async () => { await signOut(); signedIn(null); note.textContent = 'Signed out.'; });
ticketForm.addEventListener('submit', async e => {
  e.preventDefault(); const data = new FormData(ticketForm);
  if (expiry && Date.now() > expiry) { signedIn(null); note.textContent = 'Your sign in expired. Please sign in again.'; return; }
  try { await api('/api/tickets', { method: 'POST', body: { subject: data.get('subject'), message: data.get('message'), submitNonce } }); ticketForm.reset(); note.textContent = 'Ticket sent.'; await load(); }
  catch (err) { note.textContent = (err as Error).message; if ((err as { status?: number }).status === 401) signedIn(null); else await load(); }
});
signedIn(null);
