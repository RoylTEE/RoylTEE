/**
 * Walks the guided tour in a real browser, step by step, on a desktop and a phone viewport.
 * Run with: npm run test:tour   (needs a server: `npm run dev` on 4321, or `npm run preview` with TOUR_BASE)
 * Environment: TOUR_BASE (default http://127.0.0.1:4321), TOUR_SHOTS_DIR (screenshots, optional), CHROME_PATH (optional).
 * The walk only reads: it never opens the wallet and never sends a transaction.
 */
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import { existsSync, mkdirSync } from 'node:fs';
import { chromium } from 'playwright-core';
import { TOUR } from '../../src/data/tour.ts';

const BASE = process.env.TOUR_BASE ?? 'http://127.0.0.1:4321';
const SHOTS = process.env.TOUR_SHOTS_DIR;
const CHROME = process.env.CHROME_PATH ?? ['/usr/bin/google-chrome', '/usr/bin/chromium', '/usr/bin/chromium-browser'].find(existsSync);
const ORIGIN = new URL(BASE).origin;
const VIEWPORTS = [{ name: 'desktop', width: 1280, height: 800 }, { name: 'mobile', width: 390, height: 844 }];
if (SHOTS) mkdirSync(SHOTS, { recursive: true });

/** Chain answers fetched once per run: the same view call gets the same answer, so the walk is repeatable. */
const cache = new Map();
let browser;
before(async () => { browser = await chromium.launch({ executablePath: CHROME, args: ['--no-sandbox'] }); });
after(async () => { await browser?.close(); });

const stepsFor = (vp) => TOUR.filter((s) => !s.viewport || (s.viewport === 'wide') === (vp.width > 767));

/** A fresh page that records console errors, page errors and failed or error responses from our own origin. */
async function open(vp) {
  const ctx = await browser.newContext({ viewport: { width: vp.width, height: vp.height }, deviceScaleFactor: 1 });
  // Both public RPC hosts are fetched from Node, which gives real chain data on every run. The fastnear host refuses a
  // local browser origin and the other one is slow and rate limited, which would make the walk depend on the network mood.
  const cors = { 'access-control-allow-origin': '*', 'access-control-allow-headers': '*', 'access-control-allow-methods': 'POST, OPTIONS' };
  await ctx.route(/^https:\/\/(test\.rpc\.fastnear\.com|rpc\.testnet\.near\.org)\//, async (route) => {
    const req = route.request();
    if (req.method() === 'OPTIONS') return route.fulfill({ status: 204, headers: cors });
    const key = req.postData() ?? '';
    if (cache.has(key)) return route.fulfill({ status: 200, headers: { ...cors, 'content-type': 'application/json' }, body: cache.get(key) });
    let last = 'no attempt';
    for (let k = 0; k < 5; k++) {
      try {
        const r = await fetch('https://test.rpc.fastnear.com', { method: 'POST', headers: { 'content-type': 'application/json' }, body: req.postData() ?? '' });
        if (r.ok) { const body = await r.text(); cache.set(key, body); return route.fulfill({ status: 200, headers: { ...cors, 'content-type': 'application/json' }, body }); }
        last = `status ${r.status}`;
      } catch (err) { last = String(err); }
      await new Promise((res) => setTimeout(res, 300 * (k + 1)));
    }
    return route.fulfill({ status: 502, headers: cors, body: last });
  });
  const page = await ctx.newPage();
  const problems = [];
  page.on('pageerror', (e) => problems.push(`pageerror ${e.message}`));
  // The site tries two public RPC hosts and falls back when the first refuses a local origin. A refusal from another host is not ours.
  page.on('console', (m) => {
    if (m.type() !== 'error') return;
    const fetched = m.text().match(/Access to fetch at '([^']+)'/)?.[1];
    if (fetched && !fetched.startsWith(ORIGIN)) return;
    if (!fetched && m.location().url && !m.location().url.startsWith(ORIGIN)) return;
    problems.push(`console ${m.text()} ${m.location().url ?? ''}`);
  });
  page.on('requestfailed', (r) => { if (r.url().startsWith(ORIGIN) && r.failure()?.errorText !== 'net::ERR_ABORTED') problems.push(`requestfailed ${r.url()} ${r.failure()?.errorText}`); });
  page.on('response', (r) => { if (r.url().startsWith(ORIGIN) && r.status() >= 400) problems.push(`status ${r.status()} ${r.url()}`); });
  return { ctx, page, problems };
}

async function expectStep(page, list, id) {
  const s = list.find((x) => x.id === id);
  await page.waitForFunction((t) => document.querySelector('[data-tour-card]') && !document.querySelector('[data-tour-card]').hidden && document.querySelector('#tour-title')?.textContent === t && !document.querySelector('[data-tour]').hasAttribute('data-waiting'), s.title, { timeout: 30000 });
  // Wait until the ring stops moving: pages fill in after their reads and the ring follows.
  let last = '', same = 0;
  for (let k = 0; k < 100 && same < 3; k++) {
    const now = await page.evaluate(() => { const r = document.querySelector('[data-tour-ring]').getBoundingClientRect(); const c = document.querySelector('[data-tour-card]').getBoundingClientRect(); return [r.left, r.top, r.width, r.height, c.left, c.top].map(Math.round).join(','); });
    same = now === last ? same + 1 : 0; last = now; await page.waitForTimeout(100);
  }
}
const startButton = async (page) => { for (const b of await page.locator('[data-tour-start]').all()) if (await b.isVisible()) return b; return null; };

/** Geometry and accessibility checks for the step on screen. Returns the target rectangle. */
async function checkStep(page, s, list, vp) {
  const n = list.findIndex((x) => x.id === s.id);
  const info = await page.evaluate((sels) => {
    const vw = document.documentElement.clientWidth, vh = innerHeight;
    const vis = (el) => { const r = el.getBoundingClientRect(); const cs = getComputedStyle(el); return r.width > 0 && r.height > 0 && cs.visibility !== 'hidden' && cs.display !== 'none'; };
    let target = null; for (const sel of sels) { const el = document.querySelector(sel); if (el && vis(el)) { target = el; break; } }
    const rect = (el) => { const r = el.getBoundingClientRect(); return { l: r.left, t: r.top, r: r.right, b: r.bottom }; };
    const cardEl = document.querySelector('[data-tour-card]'); const ring = document.querySelector('[data-tour-ring]');
    const tr = target ? rect(target) : null;
    const clipped = tr ? { l: Math.max(tr.l, 0), t: Math.max(tr.t, 0), r: Math.min(tr.r, vw), b: Math.min(tr.b, vh) } : null;
    let hit = null;
    if (clipped && clipped.r > clipped.l && clipped.b > clipped.t) {
      const el = document.elementFromPoint((clipped.l + clipped.r) / 2, (clipped.t + clipped.b) / 2);
      hit = el ? { inTarget: target.contains(el) || el.contains(target), cls: el.className || el.outerHTML.slice(0, 120) } : null;
    }
    const inView = (el) => { const r = el.getBoundingClientRect(); return r.width > 0 && r.height > 0 && r.bottom > 0 && r.top < vh && r.right > 0 && r.left < vw; };
    const loadingText = /Reading |Not read yet/;
    // Your shares reads nothing until an account is chosen, so its stamp legitimately says Not read yet.
    const stillLoading = [...document.querySelectorAll(document.querySelector('[data-member]') ? 'main .loading' : 'main .loading, [data-stamp]')].filter((e) => inView(e) && loadingText.test(e.textContent)).map((e) => e.textContent.trim());
    const targetText = target ? target.textContent : '';
    const active = document.activeElement;
    return { stillLoading, targetLoading: loadingText.test(targetText), vw, vh, found: !!target, tr, clipped, card: rect(cardEl), ring: rect(ring), hit, count: document.querySelector('[data-tour-count]').textContent,
      role: cardEl.getAttribute('role'), modal: cardEl.getAttribute('aria-modal'), labelled: cardEl.getAttribute('aria-labelledby'), focusInCard: cardEl.contains(active), placement: document.querySelector('[data-tour]').dataset.placement,
      hint: document.querySelector('[data-tour-hint]').hidden ? '' : document.querySelector('[data-tour-hint]').textContent, nextHidden: document.querySelector('[data-tour-next]').hidden,
      nextLabel: document.querySelector('[data-tour-next]').textContent, backDisabled: document.querySelector('[data-tour-back]').disabled, skipLabel: document.querySelector('[data-tour-skip]').textContent, body: document.querySelector('#tour-body').textContent };
  }, s.target);
  const w = `${vp.name} step ${n + 1} (${s.id})`;
  assert.ok(info.found, `${w}: target exists and is visible`);
  assert.ok(!info.targetLoading, `${w}: the highlighted target has finished loading (no Reading or Not read yet text)`);
  assert.deepEqual(info.stillLoading, [], `${w}: nothing on screen still says it is reading`);
  const c = info.card;
  assert.ok(c.l >= 0 && c.t >= 0 && c.r <= info.vw && c.b <= info.vh, `${w}: popout inside the viewport ${JSON.stringify(c)}`);
  const k = info.clipped;
  assert.ok(k.r - k.l >= 8 && k.b - k.t >= 8, `${w}: target is in the viewport ${JSON.stringify(info.tr)}`);
  const g = info.ring;
  if (info.placement === 'dock') {
    // A target taller than the room beside the popout is rung from its top down to the popout.
    assert.ok(g.l <= k.l + 1 && g.t <= k.t + 1 && g.r >= k.r - 1 && g.b <= k.b + 8 && g.b - g.t >= 100, `${w}: ring marks the top of a tall target ${JSON.stringify({ ring: g, k })}`);
  } else assert.ok(g.l <= k.l + 1 && g.t <= k.t + 1 && g.r >= k.r - 1 && g.b >= k.b - 1, `${w}: ring covers the visible target ${JSON.stringify({ ring: g, k })}`);
  const dx = Math.max(g.l - c.r, c.l - g.r, 0), dy = Math.max(g.t - c.b, c.t - g.b, 0);
  assert.ok(dx > 0 || dy > 0, `${w}: popout does not cover the highlight ${JSON.stringify({ card: c, ring: g })}`);
  assert.ok(Math.hypot(dx, dy) <= 48, `${w}: popout is next to the highlight (gap ${Math.round(Math.hypot(dx, dy))}px)`);
  assert.equal(info.count, `Step ${n + 1} of ${list.length}`, `${w}: counter`);
  assert.equal(info.role, 'dialog'); assert.equal(info.modal, 'true'); assert.equal(info.labelled, 'tour-title');
  assert.equal(info.skipLabel, 'Skip tutorial');
  assert.equal(info.backDisabled, n === 0, `${w}: Back state`);
  assert.equal(info.nextHidden, !!s.action, `${w}: Next is replaced by the click on action steps`);
  if (s.action) assert.equal(info.hint, `Click ${s.action.label} to continue.`);
  else assert.equal(info.nextLabel, n === list.length - 1 ? 'Finish' : 'Next');
  assert.ok(info.focusInCard, `${w}: focus is inside the popout`);
  assert.ok(info.hit, `${w}: something sits at the middle of the target`);
  if (s.action) assert.ok(info.hit.inTarget, `${w}: action target can be clicked (nothing blocks it, got ${info.hit.cls})`);
  else assert.ok(!info.hit.inTarget && /tour-block/.test(info.hit.cls), `${w}: page is blocked behind a non-action step`);
  return info;
}

for (const vp of VIEWPORTS) {
  test(`${vp.name}: walk every step, click every action`, { timeout: 600000 }, async () => {
    const list = stepsFor(vp);
    const { ctx, page, problems } = await open(vp);
    await page.goto(`${BASE}/dashboard/`);
    await expectStep(page, list, list[0].id);   // auto start on the first visit
    for (let i = 0; i < list.length; i++) {
      const s = list[i];
      await expectStep(page, list, s.id);
      const here = new URL(page.url());
      assert.equal(here.pathname.replace(/\/+$/, '') || '/', s.path.replace(/\/+$/, '') || '/', `${vp.name} ${s.id}: on the page the step names`);
      if (s.search) assert.equal(here.search, s.search);
      try { await checkStep(page, s, list, vp); }
      catch (err) { if (SHOTS) await page.screenshot({ path: `${SHOTS}/FAIL-${vp.name}-${s.id}.png` }); throw err; }
      if (SHOTS) await page.screenshot({ path: `${SHOTS}/${vp.name}-${String(i + 1).padStart(2, '0')}.png` });
      if (s.action?.prefill) assert.equal(await page.inputValue(s.action.prefill.selector), s.action.prefill.value, `${s.id}: example is filled in`);

      // Back checks: across a page change, and inside one page with a tab change.
      if (s.id === 'sheet' || s.id === 'activity-rows') {
        const prev = list[i - 1];
        await page.locator('[data-tour-back]').click();
        await expectStep(page, list, prev.id);
        assert.equal(new URL(page.url()).pathname.replace(/\/+$/, ''), prev.path.replace(/\/+$/, ''), `${s.id}: Back lands on the previous page`);
        if (s.id === 'activity-rows') assert.equal(await page.evaluate(() => document.body.dataset.tab), 'all', 'Back resets the tab the click had changed');
        await checkStep(page, prev, list, vp);
        await clickAction(page, prev);
        await expectStep(page, list, s.id);
        await checkStep(page, s, list, vp);
      }

      if (s.action) {
        await clickAction(page, s);
        if (i + 1 < list.length) {
          try { await expectStep(page, list, list[i + 1].id); }
          catch (err) { if (SHOTS) await page.screenshot({ path: `${SHOTS}/FAIL-after-click-${s.id}-${vp.name}.png` }); console.log('stuck after click on', s.id, page.url(), await page.evaluate(() => document.querySelector('#tour-title')?.textContent)); throw err; }
        }
        // Expected change after the click
        if (s.id === 'catalog') assert.match(page.url(), /\/catalog\/splits\/?$/);
        if (s.id === 'lookup') assert.match(page.url(), /\/catalog\/lookup\/\?q=ZZRYL2600001$/);
        if (s.id === 'ledger') {
          if (vp.width > 767) assert.equal(await page.locator('tr[data-row="s2"]').getAttribute('aria-selected'), 'true', 'ledger row s2 is selected');
          else assert.match(page.url(), /\/share\?id=s2(#summary)?$/);
        }
        if (s.id === 'share') assert.equal(await page.locator('#account').isVisible(), true, 'Account and keys panel opened');
        if (s.id === 'activity') assert.equal(await page.evaluate(() => document.body.dataset.tab), 'payments');
        if (s.id === 'to-verify') assert.match(page.url(), /\/verify\/?$/);
        if (s.id === 'member') assert.equal(await page.locator('[data-who-badge]').textContent().then((t) => /Not chosen/.test(t ?? '')), false, 'the account lookup ran');
      } else if (i + 1 < list.length) {
        // Alternate between the button and the keyboard so both are exercised.
        if (i % 2) await page.keyboard.press('ArrowRight'); else await page.locator('[data-tour-next]').click();
        try { await expectStep(page, list, list[i + 1].id); }
        catch (err) { if (SHOTS) await page.screenshot({ path: `${SHOTS}/FAIL-after-${s.id}-${vp.name}.png` }); console.log('stuck after', s.id, page.url(), await page.evaluate(() => sessionStorage.getItem('royltee-tour'))); throw err; }
      } else {
        await page.locator('[data-tour-next]').click();   // Finish
        await page.waitForFunction(() => !document.querySelector('[data-tour]'));
      }
    }
    assert.equal(await page.locator('[data-tour]').count(), 0, 'the tour closed after the last step');
    assert.equal(await page.evaluate(() => localStorage.getItem('royltee-tour-done')), '1');
    await page.reload(); await page.waitForTimeout(800);
    assert.equal(await page.locator('[data-tour]').count(), 0, 'a finished tour does not restart on reload');
    assert.deepEqual(problems, [], 'no console errors and no failed requests to our own origin');
    await ctx.close();
  });

  test(`${vp.name}: skip, Esc, reload, restart and keyboard`, { timeout: 180000 }, async () => {
    const list = stepsFor(vp);
    const { ctx, page, problems } = await open(vp);
    await page.goto(`${BASE}/dashboard/`);
    await expectStep(page, list, list[0].id);
    // Keyboard: Right, Left, Enter
    await page.keyboard.press('ArrowRight'); await expectStep(page, list, list[1].id);
    await page.keyboard.press('ArrowLeft'); await expectStep(page, list, list[0].id);
    await page.keyboard.press('Enter'); await expectStep(page, list, list[1].id);
    // Tab stays inside the popout
    for (let k = 0; k < 6; k++) await page.keyboard.press('Tab');
    assert.equal(await page.evaluate(() => document.querySelector('[data-tour-card]').contains(document.activeElement)), true, 'Tab does not leave the popout');
    // Resume after a reload in the middle
    await page.reload(); await expectStep(page, list, list[1].id);
    // Skip closes and does not restart
    await page.locator('[data-tour-skip]').click();
    assert.equal(await page.locator('[data-tour]').count(), 0);
    assert.equal(await page.evaluate(() => localStorage.getItem('royltee-tour-done')), '1');
    await page.reload(); await page.waitForTimeout(1500);
    assert.equal(await page.locator('[data-tour]').count(), 0, 'Skip does not auto restart on reload');
    // Restart from the button on another page, then Esc
    await page.goto(`${BASE}/docs`);
    let btn = await startButton(page);
    if (!btn) { await page.locator('[data-drawer-btn]').click(); btn = await startButton(page); }
    assert.ok(btn, 'a visible Take the tour button');
    await btn.click();
    await expectStep(page, list, list[0].id);
    assert.match(page.url(), /\/dashboard\/?$/, 'Take the tour starts at the first page');
    await page.keyboard.press('Escape');
    assert.equal(await page.locator('[data-tour]').count(), 0, 'Esc closes the tour');
    await page.reload(); await page.waitForTimeout(1500);
    assert.equal(await page.locator('[data-tour]').count(), 0, 'Esc does not auto restart');
    assert.deepEqual(problems, []);
    await ctx.close();
  });
}

test('a missing target is skipped instead of breaking the tour', { timeout: 120000 }, async () => {
  const vp = VIEWPORTS[0]; const list = stepsFor(vp);
  const { ctx, page } = await open(vp);
  await page.goto(`${BASE}/dashboard/`);
  await expectStep(page, list, 'menu');
  await page.evaluate(() => document.querySelector('section[aria-labelledby="stmt-h"]')?.remove());
  await page.locator('[data-tour-next]').click();
  await expectStep(page, list, 'catalog');
  await ctx.close();
});

async function clickAction(page, s) {
  for (const sel of s.target) { const loc = page.locator(sel).first(); if (await loc.isVisible()) { await loc.click(); return; } }
  throw new Error(`no visible target for ${s.id}`);
}
