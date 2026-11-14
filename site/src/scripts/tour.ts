/**
 * Guided tour, loaded from shell.ts as an external module (no inline script). It dims the page, rings one
 * element at a time and shows a card beside it. Steps live in src/data/tour.ts. Progress is kept in
 * sessionStorage so the tour carries on across pages; a localStorage flag stops the first-visit auto start
 * once the tour was finished or skipped. The tour only reads: it never connects a wallet or sends a transaction.
 */
import { TOUR, type TourStep } from '../data/tour';

const STATE_KEY = 'royltee-tour';
const DONE_KEY = 'royltee-tour-done';
const WAIT_MS = 15000;
const READ_MS = 8000;
const FIXED = '.app-top, .area-tabs, .side, .menu-btn';
const FOCUSABLE = 'button:not([disabled]), [href], input:not([disabled]), textarea:not([disabled]), select:not([disabled]), [tabindex]:not([tabindex="-1"])';

const store = (k: 'session' | 'local') => { try { return k === 'session' ? sessionStorage : localStorage; } catch { return null; } };
const isWide = () => !matchMedia('(max-width: 767px)').matches;
const steps = () => TOUR.filter((s) => !s.viewport || (s.viewport === 'wide') === isWide());
const trim = (p: string) => p.replace(/\/+$/, '') || '/';
const here = (s: TourStep) => trim(location.pathname) === trim(s.path) && (!s.search || location.search === s.search);
const visible = (el: Element) => {
  const r = el.getBoundingClientRect(); const cs = getComputedStyle(el);
  return r.width > 0 && r.height > 0 && cs.visibility !== 'hidden' && cs.display !== 'none';
};
const findTarget = (s: TourStep) => { for (const sel of s.target) { const el = document.querySelector(sel); if (el && visible(el)) return el as HTMLElement; } return null; };
const typing = (el: EventTarget | null) => el instanceof HTMLElement && (el.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(el.tagName));

type Run = { id: string; target: HTMLElement | null; token: number; dir: 1 | -1; ro: ResizeObserver | null; mo: MutationObserver | null; offClick: (() => void) | null };
let run: Run | null = null;
let token = 0;
let opener: HTMLElement | null = null;
const ui = { dim: null as unknown as HTMLElement, ring: null as unknown as HTMLElement, blocks: [] as HTMLElement[], cover: null as unknown as HTMLElement, card: null as unknown as HTMLElement,
  count: null as unknown as HTMLElement, title: null as unknown as HTMLElement, body: null as unknown as HTMLElement, hint: null as unknown as HTMLElement,
  skip: null as unknown as HTMLButtonElement, back: null as unknown as HTMLButtonElement, next: null as unknown as HTMLButtonElement, root: null as unknown as HTMLElement };

function el<K extends keyof HTMLElementTagNameMap>(tag: K, cls: string, text?: string) { const n = document.createElement(tag); n.className = cls; if (text) n.textContent = text; return n; }
const box = (n: HTMLElement, l: number, t: number, w: number, h: number) => { n.style.setProperty('left', `${l}px`); n.style.setProperty('top', `${t}px`); n.style.setProperty('width', `${Math.max(0, w)}px`); n.style.setProperty('height', `${Math.max(0, h)}px`); };

function build() {
  if (ui.root) return;
  ui.root = el('div', 'tour'); ui.root.setAttribute('data-tour', '');
  ui.dim = el('div', 'tour-dim'); ui.ring = el('div', 'tour-ring'); ui.ring.setAttribute('data-tour-ring', ''); ui.cover = el('div', 'tour-block');
  ui.blocks = [el('div', 'tour-block'), el('div', 'tour-block'), el('div', 'tour-block'), el('div', 'tour-block')];
  const card = el('div', 'tour-card'); ui.card = card;
  card.setAttribute('role', 'dialog'); card.setAttribute('aria-modal', 'true'); card.setAttribute('aria-labelledby', 'tour-title'); card.setAttribute('aria-describedby', 'tour-body'); card.setAttribute('aria-label', 'Guided tour'); card.tabIndex = -1; card.setAttribute('data-tour-card', '');
  ui.count = el('p', 'tour-count'); ui.count.setAttribute('data-tour-count', '');
  ui.title = el('h2', 'tour-title'); ui.title.id = 'tour-title';
  ui.body = el('p', 'tour-body'); ui.body.id = 'tour-body';
  ui.hint = el('p', 'tour-hint'); ui.hint.setAttribute('data-tour-hint', '');
  const actions = el('div', 'tour-actions');
  ui.skip = el('button', 'btn sm quiet tour-skip', 'Skip tutorial') as HTMLButtonElement; ui.skip.type = 'button'; ui.skip.setAttribute('data-tour-skip', '');
  ui.back = el('button', 'btn sm', 'Back') as HTMLButtonElement; ui.back.type = 'button'; ui.back.setAttribute('data-tour-back', ''); ui.back.setAttribute('aria-label', 'Back to the previous step');
  ui.next = el('button', 'btn sm primary', 'Next') as HTMLButtonElement; ui.next.type = 'button'; ui.next.setAttribute('data-tour-next', '');
  actions.append(ui.skip, ui.back, ui.next);
  card.append(ui.count, ui.title, ui.body, ui.hint, actions);
  ui.root.append(ui.dim, ...ui.blocks, ui.cover, ui.ring, card);
  document.body.append(ui.root);
  ui.skip.addEventListener('click', () => finish());
  ui.back.addEventListener('click', () => go(-1));
  ui.next.addEventListener('click', () => go(1));
}

function readState(): string | null { try { return JSON.parse(store('session')?.getItem(STATE_KEY) ?? 'null')?.id ?? null; } catch { return null; } }
const writeState = (id: string) => { try { store('session')?.setItem(STATE_KEY, JSON.stringify({ id })); } catch { /* storage blocked: the tour still runs on this page */ } };
const clearState = () => { try { store('session')?.removeItem(STATE_KEY); } catch { /* ignore */ } };
const markDone = () => { try { store('local')?.setItem(DONE_KEY, '1'); } catch { /* ignore */ } };

function teardown() {
  token++;
  run?.ro?.disconnect(); run?.mo?.disconnect(); run?.offClick?.();
  run = null;
  ui.root?.remove(); ui.root = null as unknown as HTMLElement;
  document.removeEventListener('keydown', onKey, true);
  window.removeEventListener('resize', onViewport); window.removeEventListener('scroll', onViewport, true);
}
/** Close the tour for good: Skip, Esc and the last step all end up here. */
function finish() {
  clearState(); markDone(); teardown();
  if (opener && document.contains(opener)) opener.focus();
  opener = null;
}

function setHash(h?: string) {
  if (!h || location.hash.slice(1) === h) return;
  history.replaceState(null, '', `${location.pathname}${location.search}#${h}`);
  window.dispatchEvent(new HashChangeEvent('hashchange'));
}

function closeDrawer() { if (document.body.dataset.menu === 'open') document.querySelector<HTMLElement>('[data-drawer-close]')?.click(); }

/** Lay out the ring, the click blockers and the card. With scroll=true the target is first scrolled below the sticky bars. */
function layout(scroll: boolean) {
  if (!run?.target) return;
  const s = steps().find((x) => x.id === run!.id)!; const t = run.target;
  const vw = document.documentElement.clientWidth; const vh = window.innerHeight;
  const card = ui.card; // Width is fixed in CSS, so the size can be read where it is. Hiding it would drop focus.
  const cw = card.offsetWidth; const ch = card.offsetHeight;
  let r = t.getBoundingClientRect();
  if (scroll && !t.closest(FIXED)) { window.scrollBy({ top: r.top - 120, behavior: 'instant' as ScrollBehavior }); r = t.getBoundingClientRect(); }
  const PAD = 4, GAP = 12, M = 12;
  let l = Math.max(r.left, 0), tp = Math.max(r.top, 0), rt = Math.min(r.right, vw), b = Math.min(r.bottom, vh);
  const clampX = (x: number) => Math.min(Math.max(x, M), vw - cw - M); const clampY = (y: number) => Math.min(Math.max(y, M), vh - ch - M);
  const spots: Record<string, { x: number; y: number; ok: boolean }> = {
    below: { x: clampX(l), y: b + PAD + GAP, ok: b + PAD + GAP + ch <= vh - M },
    above: { x: clampX(l), y: tp - PAD - GAP - ch, ok: tp - PAD - GAP - ch >= M },
    right: { x: rt + PAD + GAP, y: clampY(tp), ok: rt + PAD + GAP + cw <= vw - M },
    left: { x: l - PAD - GAP - cw, y: clampY(tp), ok: l - PAD - GAP - cw >= M },
  };
  const order = (b - tp) > vh * 0.5 ? ['right', 'left', 'below', 'above'] : ['below', 'above', 'right', 'left'];
  const pick = order.find((k) => spots[k].ok);
  let x: number, y: number;
  if (pick) ({ x, y } = spots[pick]);
  else { x = clampX((vw - cw) / 2); y = vh - ch - M; b = Math.min(b, y - PAD - GAP); b = Math.max(b, tp + 40); }
  card.style.setProperty('left', `${Math.round(x)}px`); card.style.setProperty('top', `${Math.round(y)}px`);
  box(ui.ring, l - PAD, tp - PAD, rt - l + 2 * PAD, b - tp + 2 * PAD);
  box(ui.blocks[0], 0, 0, vw, tp); box(ui.blocks[1], 0, b, vw, vh - b); box(ui.blocks[2], 0, tp, l, b - tp); box(ui.blocks[3], rt, tp, vw - rt, b - tp);
  if (s.action) ui.cover.hidden = true; else { ui.cover.hidden = false; box(ui.cover, l, tp, rt - l, b - tp); }
  ui.root.dataset.placement = pick ?? 'dock';
}
let raf = 0;
/** Re-place the ring. When the page content moved (reads finished) and the target ended up under the bars or off screen, scroll to it again. */
function relayout(rescroll = false) {
  cancelAnimationFrame(raf);
  raf = requestAnimationFrame(() => {
    const t = run?.target;
    if (!t) return;
    const r = t.getBoundingClientRect();
    const lost = !t.closest(FIXED) && (r.top < 100 || r.top > window.innerHeight - 40 || r.bottom < 100);
    layout(rescroll && lost);
    if (document.activeElement === document.body) focusStep();
  });
}
const onViewport = () => relayout(false);
const onContent = () => relayout(true);

function render(s: TourStep, list: TourStep[]) {
  const n = list.findIndex((x) => x.id === s.id);
  ui.count.textContent = `Step ${n + 1} of ${list.length}`;
  ui.title.textContent = s.title; ui.body.textContent = s.body;
  ui.hint.textContent = s.action ? `Click ${s.action.label} to continue.` : ''; ui.hint.hidden = !s.action;
  ui.next.hidden = !!s.action; ui.next.textContent = n === list.length - 1 ? 'Finish' : 'Next';
  ui.back.disabled = n === 0;
  ui.card.setAttribute('aria-label', `Guided tour, step ${n + 1} of ${list.length}: ${s.title}`);
}

async function show(id: string, dir: 1 | -1) {
  const list = steps(); let i = list.findIndex((x) => x.id === id);
  if (i < 0) { finish(); return; }
  build();
  const mine = ++token;
  run?.ro?.disconnect(); run?.mo?.disconnect(); run?.offClick?.();
  run = { id, target: null, token: mine, dir, ro: null, mo: null, offClick: null };
  const s = list[i];
  writeState(s.id); closeDrawer(); setHash(s.hash);
  if (s.action?.prefill) { const inp = document.querySelector<HTMLInputElement>(s.action.prefill.selector); if (inp && !inp.value) inp.value = s.action.prefill.value; }
  ui.root.dataset.waiting = ''; ui.dim.hidden = false; ui.ring.hidden = true; ui.cover.hidden = true; ui.blocks.forEach((x) => { x.hidden = true; });
  ui.card.hidden = true;
  document.addEventListener('keydown', onKey, true); window.addEventListener('resize', onViewport); window.addEventListener('scroll', onViewport, true);
  const start = Date.now(); let target: HTMLElement | null = null;
  // Wait for the target, then for the reads inside it to finish, so the popout describes what is on screen.
  const reading = (t: HTMLElement | null) => !!t && (t.matches('.loading') || !!t.querySelector('.loading'));
  while ((!(target = findTarget(s)) || (reading(target) && Date.now() - start < READ_MS)) && Date.now() - start < WAIT_MS) { await new Promise((r) => setTimeout(r, 120)); if (mine !== token) return; }
  if (mine !== token) return;
  if (!target) { // Missing target: skip this step instead of breaking the tour.
    const j = i + dir; if (j < 0 || j >= list.length) { if (dir === 1) finish(); else open(list[Math.min(i + 1, list.length - 1)], 1); return; }
    open(list[j], dir); return;
  }
  run.target = target;
  delete ui.root.dataset.waiting; ui.dim.hidden = true; ui.ring.hidden = false; ui.blocks.forEach((x) => { x.hidden = false; }); ui.card.hidden = false;
  render(s, list);
  layout(true);
  run.ro = new ResizeObserver(onContent); run.ro.observe(target); run.ro.observe(document.body);
  // Pages fill in after their testnet reads: re-place the ring whenever the page content changes.
  run.mo = new MutationObserver((ms) => { if (ms.some((m) => !ui.root.contains(m.target))) onContent(); }); run.mo.observe(document.querySelector('main') ?? document.body, { childList: true, subtree: true, attributes: true, attributeFilter: ['hidden', 'class'] });
  if (s.action) {
    const onClick = () => {
      const nxt = list[i + 1];
      if (!nxt) { finish(); return; }
      writeState(nxt.id);
      // A link leaves the page and the next page resumes the tour. In-page changes show the next step right away.
      setTimeout(() => { if (run && run.id === s.id) void show(nxt.id, 1); }, 60);
    };
    target.addEventListener('click', onClick, { capture: true, once: true });
    run.offClick = () => target!.removeEventListener('click', onClick, { capture: true } as EventListenerOptions);
  }
  focusStep();
}
function focusStep() {
  const s = run && steps().find((x) => x.id === run!.id);
  (s?.action ? ui.card : ui.next).focus({ preventScroll: true });
}

/** Move one step. A step on another page is reached by loading that page; the tour resumes there. */
function go(dir: 1 | -1) {
  if (!run) return;
  const list = steps(); const i = list.findIndex((x) => x.id === run!.id);
  const j = i + dir;
  if (j >= list.length) { finish(); return; }
  if (j < 0) return;
  open(list[j], dir);
}
function open(s: TourStep, dir: 1 | -1) {
  writeState(s.id);
  if (here(s)) { void show(s.id, dir); return; }
  teardown();
  location.assign(`${s.path}${s.search ?? ''}`);
}

function onKey(e: KeyboardEvent) {
  if (!run) return;
  if (e.key === 'Escape') { e.preventDefault(); e.stopPropagation(); finish(); return; }
  const s = steps().find((x) => x.id === run!.id);
  if (e.key === 'Tab') {
    const items = [...ui.card.querySelectorAll<HTMLElement>(FOCUSABLE)].filter((x) => !x.hidden && visible(x));
    if (s?.action && run.target) items.push(run.target);
    if (!items.length) return;
    const k = items.indexOf(document.activeElement as HTMLElement);
    const nxt = e.shiftKey ? (k <= 0 ? items.length - 1 : k - 1) : (k < 0 || k === items.length - 1 ? 0 : k + 1);
    e.preventDefault(); items[nxt].focus(); return;
  }
  if (typing(e.target) || e.altKey || e.ctrlKey || e.metaKey) return;
  const onButton = e.target instanceof HTMLElement && /^(BUTTON|A)$/.test(e.target.tagName);
  if (e.key === 'ArrowLeft') { e.preventDefault(); go(-1); }
  else if (e.key === 'ArrowRight' && !s?.action) { e.preventDefault(); go(1); }
  else if (e.key === 'Enter' && !s?.action && !onButton) { e.preventDefault(); go(1); }
}

/** Start from the first step. From any page other than the first step's page, this loads that page. */
function begin(from?: HTMLElement | null) {
  opener = from ?? null;
  closeDrawer();
  const first = steps()[0];
  if (run) teardown();
  open(first, 1);
}

/** Called by shell.ts after the menu and tabs are wired, so a tab hash set by the tour is handled by them. */
export function initTour() {
  document.querySelectorAll<HTMLElement>('[data-tour-start]').forEach((b) => b.addEventListener('click', () => begin(b)));
  const saved = readState();
  if (saved) {
    const list = steps();
    const k = TOUR.findIndex((x) => x.id === saved);
    const s = k < 0 ? null : (list.find((x) => x.id === saved) ?? TOUR.slice(k).find((x) => list.includes(x)) ?? null);
    if (s && here(s)) void show(s.id, 1);
    else { clearState(); markDone(); } // Left the tour by some other route: do not drag the person back into it.
    return;
  }
  const dash = trim(location.pathname) === '/dashboard';
  let done = false; try { done = !!store('local')?.getItem(DONE_KEY); } catch { /* ignore */ }
  if (dash && !done) { writeState(steps()[0].id); void show(steps()[0].id, 1); }
}
