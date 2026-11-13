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
| L1 legacy tickets never expire | REAL (count in storage UNVERIFIED) | Idempotent backfill added (`backfillOpenTickets`, admin route `POST /api/admin/backfill`, dry run by default). Not run on any deployed storage; until it runs, site/README.md says old tickets are outside the policy. |
| L2 paging stops when the cursor ticket disappears | REAL | Fixed: exclusive (createdAt, id) boundary, window grows until the page is full or the index ends. Tests: expired cursor, 7 tied tickets, 104 tied tickets with an expired cursor. |
| L3 open ticket cap race | REAL | Fixed: per wallet slot reservation with one atomic counter increment around the count and write. Test: 9 open, 5 concurrent submits, exactly 1 created. |
| L4 chat budgets | REAL | Documented. The anonymous share and the global cap now share one UTC day key (second reviewer item 2). |
| L5 dashboard mockup | Superseded on `ui/pass` | `site/public/dashboard.js` is dead code; deleting it needs the owner's yes. |
| I1 to I8 | Noted | I3 is closed by the merge. I7 facts stay UNVERIFIED. |

## Second reviewer, part 03b

| # | Item | Verdict | Status |
|---|---|---|---|
| 1 | Dotted number exception before card and phone checks | REAL | Card check runs first; a dotted amount is skipped only next to a currency. Tests: `555.123.4567` is a phone number, `411.111.111.111.111.1` is a card number, `1.000.000 mUSDC` passes. |
| 2 | Anonymous calls reach the signed-in reserve | REAL | Anonymous and global counters are keyed by the same UTC day (`chat:global:anon:<day>`, `chat:global:<day>`). Test: cap 3, reserve 1: late anonymous call refused, signed call served, next day resets. |
| 3 | Paging hides tickets (expired cursor, ties) | REAL | As L2 above. |
| 4 | Legacy open tickets outside TTL and open index | REAL | As L1 above. Test: 12 legacy records, dry run reports 10 to index and 2 expired, apply makes the cap count them, a second run changes nothing. Running it on preview or production storage needs the owner's yes. |
| 5 | Grounding tests assert stale facts | FALSE on `ui/pass` and this branch | The notes already say seven shares and that the agent and dispute scenes are on chain; a test now asserts the stale phrases are absent. |
| 6 | Open ticket cap race | REAL | As L3 above. |
| 7 | Expired chat lock worker overwrites history | REAL (timing in production UNVERIFIED) | The lock carries a unique owner token; the commit is a compare and set against it, and a superseded answer gets 409. Test: slow worker loses after a takeover, history count stays 1. |

Tests on this branch: site `npm test` 200 passed (17 files), `npm run typecheck` 0 errors, 0 warnings, 0 hints, `npm run scan:wording` clean, `npm run build` exit 0.
