/** Mobile menu toggle. Closes on Escape and when a link is chosen. */
const btn = document.querySelector<HTMLButtonElement>('[data-menu-btn]');
const menu = document.querySelector<HTMLElement>('[data-menu]');
if (btn && menu) {
  const set = (open: boolean) => { btn.setAttribute('aria-expanded', String(open)); menu.hidden = !open; };
  btn.addEventListener('click', () => set(menu.hidden === true));
  document.addEventListener('keydown', (e) => { if (e.key === 'Escape' && !menu.hidden) { set(false); btn.focus(); } });
  menu.querySelectorAll('a').forEach((a) => a.addEventListener('click', () => set(false)));
}
