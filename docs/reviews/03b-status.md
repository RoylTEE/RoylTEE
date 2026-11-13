# Review 03b status

Branch: `fix/review-03b` (off `ui/pass`). Local only, not pushed, not merged. Report: review 03b of the site and server diff (`86e071b..613c116`).

| Item | Verdict | Status |
|---|---|---|
| Correction: 03a L8(b) withdrawn | Accepted | `astro dev --host 127.0.0.1 --port 4321` in site/package.json. |
| Correction: 03a M4 narrowed | Accepted | provider_tee check is on `fix/server-review`; DECISIONS_LOG row 73 updated on `fix/review-03a`. |
| C1 (carried) | REAL | Merge plan in the final report; needs the owner's yes. |
| M1 wallet code on /admin and /help | PARTLY | Sandbox checked: near-connect 0.12.0 sets `sandbox="allow-scripts"` only (`build/SandboxedWallet/iframe.js`), so no same origin access. Manifest control and any `https:` hosts remain; documented in site/README.md "What the wallet code can and cannot do". Host lists not tightened (not verifiable). Manifest pin not done. |
| M2 admin not set up | REAL | Stated in site/README.md. Creating `admin.royltee-dev.testnet` and setting `ADMIN_WALLET` and `ADMIN_PUBLIC_KEY` needs the owner's yes. |
| M3 hosted site unverified | REAL | Pre-submission item; steps in the final report. UNVERIFIED until run. |
| L1 legacy tickets never expire | REAL (count in storage UNVERIFIED) | Documented; no migration. |
| L2 paging stops when the cursor ticket disappears | REAL | Fixed in `site/src/server/tickets.ts`; tests in `site/tests/review-03b.test.ts`. |
| L3 open ticket cap race | REAL | Accepted and documented. |
| L4 chat budgets | REAL | Documented in site/README.md. |
| L5 dashboard mockup | Superseded on `ui/pass` | `site/public/dashboard.js` is dead code; deleting it needs the owner's yes. |
| I1 to I8 | Noted | I3 is closed by the merge. I7 facts stay UNVERIFIED. |

Tests on this branch: site `npm test` 187 passed (17 files), `npm run typecheck` 0 errors, `npm run scan:wording` clean.
