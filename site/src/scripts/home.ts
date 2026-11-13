import { gsap } from 'gsap';
import { ScrollTrigger } from 'gsap/ScrollTrigger';
import { SplitText } from 'gsap/SplitText';

gsap.registerPlugin(ScrollTrigger, SplitText);
// ?static renders every section in its final state (used for full page screenshots).
const isStatic = new URLSearchParams(location.search).has('static');
const reduce = isStatic || window.matchMedia('(prefers-reduced-motion: reduce)').matches;
const $ = <T extends Element = HTMLElement>(s: string, r: ParentNode = document) => r.querySelector<T>(s);
const $$ = <T extends Element = HTMLElement>(s: string, r: ParentNode = document) => Array.from(r.querySelectorAll<T>(s));

function initMotion() {
  // 1. Hero: lines rise out of a mask on load, then the sub copy and buttons fade up.
  const heroH = $('[data-hero-split]');
  if (heroH) {
    const split = SplitText.create(heroH, { type: 'lines', mask: 'lines', linesClass: 'ln' });
    gsap.timeline({ delay: 0.15 })
      .from(split.lines, { yPercent: 110, duration: 0.9, stagger: 0.08, ease: 'power3.out' })
      .from('[data-hero-fade]', { opacity: 0, y: 16, duration: 0.6, stagger: 0.08, ease: 'power2.out' }, '-=0.5');
  }
  // Hero art drifts slower than the page.
  gsap.to('[data-parallax]', { yPercent: -10, ease: 'none', scrollTrigger: { trigger: '.hero', start: 'top top', end: 'bottom top', scrub: true } });

  // 2. Section headings: lines rise when the heading reaches 80 percent of the viewport.
  $$('[data-split-lines]').forEach((el) => {
    const s = SplitText.create(el, { type: 'lines', mask: 'lines' });
    gsap.from(s.lines, { yPercent: 105, duration: 0.8, stagger: 0.08, ease: 'power3.out', scrollTrigger: { trigger: el, start: 'top 85%', toggleActions: 'play none none none' } });
  });

  // 3. Generic stagger reveal: fade and lift 30px, 0.5s, power2.out, at top 80%.
  ScrollTrigger.batch('[data-stagger]', {
    start: 'top 80%',
    onEnter: (els) => gsap.fromTo(els, { opacity: 0, y: 30 }, { opacity: 1, y: 0, duration: 0.5, stagger: 0.1, ease: 'power2.out', overwrite: true }),
  });
  gsap.set('[data-stagger]', { opacity: 0 });

  // 4. Diagram: one sequenced timeline, played once when the diagram reaches 70 percent.
  const dg = $('[data-diagram]');
  if (dg) {
    const q = (k: string) => $$(`[data-dg="${k}"]`, dg);
    gsap.set(q('logo'), { opacity: 0, scale: 0.6 });
    gsap.set(q('vline'), { scaleY: 0 });
    gsap.set(q('connector'), { clipPath: 'inset(0 50% 0 50%)' });
    gsap.set(q('pill'), { opacity: 0, y: 8 });
    gsap.set(q('dot'), { opacity: 0, scale: 0 });
    gsap.set([...q('engine'), ...q('vaults'), ...q('foot')], { opacity: 0, y: 24 });
    gsap.set(q('card'), { opacity: 0, y: 16 });
    gsap.set(q('orb'), { opacity: 0, scale: 0.7 });
    gsap.set(q('link'), { scaleY: 0, transformOrigin: 'top' });
    const tl = gsap.timeline({ paused: true })
      .to(q('logo'), { opacity: 1, scale: 1, duration: 0.45, ease: 'back.out(1.7)' })
      .to(q('vline'), { scaleY: 1, duration: 0.35, ease: 'power2.out' }, '-=0.05')
      .to(q('connector'), { clipPath: 'inset(0 0% 0 0%)', duration: 0.7, ease: 'power2.inOut' }, '-=0.1')
      .to(q('pill'), { opacity: 1, y: 0, duration: 0.3, stagger: 0.08 }, '-=0.2')
      .to(q('dot'), { opacity: 1, scale: 1, duration: 0.25, stagger: 0.06, ease: 'back.out(2.5)' }, '-=0.15')
      .to(q('engine'), { opacity: 1, y: 0, duration: 0.45, ease: 'power2.out' })
      .to(q('card'), { opacity: 1, y: 0, duration: 0.45, stagger: 0.09, ease: 'power2.out' }, '-=0.2')
      .to(q('link')[0], { scaleY: 1, duration: 0.35, ease: 'power2.out' })
      .to(q('vaults'), { opacity: 1, y: 0, duration: 0.4, ease: 'power2.out' })
      .to(q('orb'), { opacity: 1, scale: 1, duration: 0.35, stagger: 0.08, ease: 'back.out(1.4)' }, '-=0.15')
      .to(q('link')[1], { scaleY: 1, duration: 0.35, ease: 'power2.out' })
      .to(q('foot'), { opacity: 1, y: 0, duration: 0.45, ease: 'power2.out' });
    ScrollTrigger.create({ trigger: dg, start: 'top 70%', once: true, onEnter: () => tl.play() });
  }

  // 5. Lifecycle: pin the section on desktop and scrub the card track sideways.
  const mm = gsap.matchMedia();
  mm.add('(min-width: 768px)', () => {
    const sec = $('[data-life]');
    const track = $('[data-life-track]');
    const fill = $('[data-life-fill]');
    if (!sec || !track || !fill) return;
    const dist = () => Math.max(0, track.scrollWidth - window.innerWidth);
    const tl = gsap.timeline({ scrollTrigger: { trigger: sec, start: 'top top', end: () => `+=${dist() + window.innerHeight * 0.5}`, pin: true, scrub: 1, invalidateOnRefresh: true } });
    tl.to(track, { x: () => -dist(), ease: 'none' }, 0).to(fill, { width: '100%', ease: 'none' }, 0);
  });

  // 6. Resource cards: scrubbed fade in, each finishing a little later (top 45/50/55/60 percent).
  const ends = ['top 45%', 'top 50%', 'top 55%', 'top 60%'];
  $$('[data-card-fade]').forEach((card, i) => {
    gsap.fromTo(card, { opacity: 0, y: 60 }, { opacity: 1, y: 0, ease: 'none', scrollTrigger: { trigger: card, start: 'top 100%', end: ends[i % 4], scrub: 1 } });
  });
}

// Auto advancing tabs with a progress bar, 8.5s per tab, running only while in view.
function initTabs() {
  const root = $('[data-tabs]');
  if (!root) return;
  const tabs = $$('[data-tab]', root);
  const DURATION = 8500;
  let idx = 0; let timer: number | undefined; let running = false;
  const show = (i: number) => {
    idx = i;
    tabs.forEach((t, k) => {
      t.classList.toggle('is-active', k === i);
      const p = $('[data-prog]', t)!;
      p.style.transition = 'none'; p.style.width = '0%';
    });
    if (!running || reduce) return;
    const p = $('[data-prog]', tabs[i])!;
    void p.offsetWidth;
    p.style.transition = `width ${DURATION}ms linear`; p.style.width = '100%';
    clearTimeout(timer);
    timer = window.setTimeout(() => show((idx + 1) % tabs.length), DURATION);
  };
  tabs.forEach((t, k) => t.addEventListener('click', () => show(k)));
  new IntersectionObserver(([e]) => {
    running = e.isIntersecting;
    if (running) show(idx); else { clearTimeout(timer); show(idx); }
  }, { threshold: 0.3 }).observe(root);
}

// Updates strip: arrow buttons scroll by one card, autoplay every 4s, pause on hover.
function initUpdates() {
  const list = $('[data-up-list]');
  if (!list) return;
  const step = () => (list.firstElementChild as HTMLElement).offsetWidth + 14;
  const next = () => { const atEnd = list.scrollLeft + list.clientWidth >= list.scrollWidth - 4; list.scrollTo({ left: atEnd ? 0 : list.scrollLeft + step() }); };
  const prev = () => list.scrollTo({ left: Math.max(0, list.scrollLeft - step()) });
  $('[data-up-next]')?.addEventListener('click', next);
  $('[data-up-prev]')?.addEventListener('click', prev);
  let t = window.setInterval(next, 4000);
  list.addEventListener('mouseenter', () => clearInterval(t));
  list.addEventListener('mouseleave', () => { t = window.setInterval(next, 4000); });
}

// Placeholder quote tabs.
function initVoices() {
  const tabs = $$('[data-vtab]');
  const slides = $$('[data-vslide]');
  tabs.forEach((t, i) => t.addEventListener('click', () => {
    tabs.forEach((x, k) => x.classList.toggle('is-active', k === i));
    slides.forEach((x, k) => x.classList.toggle('is-active', k === i));
  }));
}

const start = async () => {
  await document.fonts.ready;
  if (!reduce) initMotion();
  initTabs(); initUpdates(); initVoices();
};
if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', start); else start();
