import { api, el, signIn, signOut } from './wallet';
type Ticket = { id: string; wallet: string; subject: string; message: string; status: string; reply: string | null; createdAt: number; closedAt: number | null };
const root = document.querySelector<HTMLElement>('[data-admin]')!;
const note = root.querySelector<HTMLElement>('[data-admin-status]')!;
const list = root.querySelector<HTMLElement>('[data-admin-list]')!;
async function load() {
  try {
    const { tickets } = await api<{ tickets: Ticket[] }>('/api/admin/tickets');
    root.dataset.state = 'in';
    list.replaceChildren(...(tickets.length ? tickets.map(row) : [el('li', { className: 'muted', textContent: 'No tickets.' })]));
  } catch (err) { root.dataset.state = 'out'; list.replaceChildren(); note.textContent = (err as Error).message; }
}
function row(t: Ticket) {
  const reply = el('textarea', { maxLength: 2000, rows: 3, placeholder: 'Reply', disabled: t.status === 'closed' });
  const send = el('button', { className: 'btn no-chip dark', textContent: 'Reply', disabled: t.status === 'closed' });
  const close = el('button', { className: 'btn no-chip outline', textContent: 'Close', disabled: t.status === 'closed' });
  send.onclick = () => act({ action: 'reply', id: t.id, reply: reply.value });
  close.onclick = () => act({ action: 'close', id: t.id });
  return el('li', { className: `ticket ${t.status}` },
    el('div', { className: 'ticket-head' }, el('strong', { textContent: t.subject }), el('span', { className: 'tag-mono', textContent: t.status })),
    el('p', { className: 'mono muted', textContent: `${t.wallet} . ${new Date(t.createdAt).toISOString().slice(0, 16)} UTC` }),
    el('p', { textContent: t.message }), ...(t.reply ? [el('p', { className: 'reply', textContent: `Reply: ${t.reply}` })] : []),
    ...(t.closedAt ? [el('p', { className: 'muted', textContent: `Closed. Deleted 30 days after ${new Date(t.closedAt).toISOString().slice(0, 10)}.` })] : []),
    reply, el('div', { className: 'row' }, send, close));
}
async function act(body: Record<string, string>) { try { await api('/api/admin/tickets', { method: 'POST', body }); note.textContent = 'Saved.'; await load(); } catch (err) { note.textContent = (err as Error).message; } }
root.querySelector('[data-sign-in]')?.addEventListener('click', async () => { note.textContent = 'Waiting for your wallet...'; try { await signIn(); note.textContent = ''; await load(); } catch (err) { note.textContent = (err as Error).message; } });
root.querySelector('[data-sign-out]')?.addEventListener('click', async () => { await signOut(); root.dataset.state = 'out'; list.replaceChildren(); note.textContent = 'Signed out.'; });
root.dataset.state = 'out';
if (root.dataset.permitted === 'true') void load();
