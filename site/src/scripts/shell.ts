export {};
/**
 * App shell behaviour, loaded as an external module (no inline scripts):
 * 1. the mobile menu drawer, 2. hash tabs inside an area, 3. "Show all" toggles.
 * Without this script every panel and row stays visible, so nothing is lost.
 */
const body = document.body;

// 1. Drawer
const btn = document.querySelector<HTMLButtonElement>('[data-drawer-btn]');
const drawer = document.querySelector<HTMLElement>('[data-drawer]');
const backdrop = document.querySelector<HTMLElement>('.side-backdrop');
const narrow = matchMedia('(max-width: 991px)');
function setMenu(open: boolean, refocus = true) {
  if (!btn || !drawer) return;
  body.dataset.menu = open ? 'open' : 'closed';
  btn.setAttribute('aria-expanded', String(open));
  if (backdrop) backdrop.hidden = !open;
  if (open) (drawer.querySelector<HTMLElement>('.side-link[aria-current="page"]') ?? drawer.querySelector<HTMLElement>('a'))?.focus();
  else if (refocus) btn.focus();
}
btn?.addEventListener('click', () => setMenu(body.dataset.menu !== 'open'));
document.querySelectorAll('[data-drawer-close]').forEach((el) => el.addEventListener('click', () => setMenu(false)));
drawer?.querySelectorAll('a').forEach((a) => a.addEventListener('click', () => { if (narrow.matches) setMenu(false, false); }));
document.addEventListener('keydown', (e) => { if (e.key === 'Escape' && body.dataset.menu === 'open') setMenu(false); });
narrow.addEventListener('change', () => { if (!narrow.matches && body.dataset.menu === 'open') setMenu(false, false); });

// 2. Hash tabs. Keys come from the area tab links that carry a hash.
const links = [...document.querySelectorAll<HTMLAnchorElement>('[data-tab-link]')];
const keys = [...new Set(links.map((a) => a.dataset.tabLink!))];
const panels = [...document.querySelectorAll<HTMLElement>('[data-tab-panel]')];
function show(key: string) {
  body.dataset.tab = key;
  for (const a of links) { if (a.dataset.tabLink === key) a.setAttribute('aria-current', 'page'); else a.removeAttribute('aria-current'); }
  for (const p of panels) p.hidden = p.id !== key;
}
const fromHash = () => { const k = decodeURIComponent(location.hash.slice(1)); return keys.includes(k) ? k : keys[0]; };
if (keys.length) {
  const deep = keys.includes(location.hash.slice(1));
  show(fromHash());
  if (deep) requestAnimationFrame(() => window.scrollTo(0, 0));
  for (const a of links) a.addEventListener('click', (e) => {
    const url = new URL(a.href);
    if (url.pathname !== location.pathname) return;
    e.preventDefault();
    history.replaceState(null, '', `${location.pathname}${location.search}#${a.dataset.tabLink}`);
    show(a.dataset.tabLink!);
    const top = document.querySelector<HTMLElement>('[data-area-tabs]');
    if (top && window.scrollY > top.offsetTop) window.scrollTo(0, top.offsetTop);
  });
  window.addEventListener('hashchange', () => show(fromHash()));
}

// 3. "Show all" toggles: <button data-expand aria-controls="id" aria-expanded="false">
for (const b of document.querySelectorAll<HTMLButtonElement>('[data-expand]')) {
  const target = document.getElementById(b.getAttribute('aria-controls') ?? '');
  if (!target) continue;
  const label = b.textContent ?? 'Show all';
  b.hidden = false;
  b.addEventListener('click', () => {
    const open = b.getAttribute('aria-expanded') !== 'true';
    b.setAttribute('aria-expanded', String(open));
    if (open) target.setAttribute('data-expanded', ''); else target.removeAttribute('data-expanded');
    b.textContent = open ? 'Show latest 5' : label;
  });
}
