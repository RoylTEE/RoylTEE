# RoylTEE site

Astro site for the RoylTEE testnet demo. Pages are static. Only `/api/*` and `/admin` run as Vercel serverless functions through `@astrojs/vercel`.

Testnet only. Payments use Mock USDC (testnet). Everything shown is Simulated example data.

## Run

```
npm ci
npm test            # unit and API tests, model and RPC mocked
npm run typecheck   # astro check
npm run build       # output in .vercel/output
npm run scan:wording
```

Local dev with the API: copy `.env.example` to `.env`, set `STORAGE_MODE=memory`, `ALLOWED_ORIGINS=http://127.0.0.1:4321`, and two random 32+ character values for `CHAT_HMAC_SECRET` and `SESSION_SECRET`, then `npm run dev`. Without `NEARAI_API_KEY` the chat says it is not configured.

## Environment variables

Names only; values live in the host's environment settings, never in the repo.

| Name | Needed for | Notes |
|---|---|---|
| `NEARAI_API_KEY` | help chat | NEAR AI Cloud API key |
| `NEARAI_BASE_URL` | help chat | optional, default `https://cloud-api.near.ai/v1` |
| `NEARAI_MODEL` | help chat | optional, default `z-ai/glm-5.3-flash` |
| `CHAT_HMAC_SECRET` | help chat | 32+ random characters; signs assistant turns |
| `SESSION_SECRET` | support | 32+ random characters; signs the sign in cookie and keys hashed client addresses |
| `ALLOWED_ORIGINS` | both | comma separated exact origins of the deployed site |
| `ADMIN_WALLET` | support admin | owner wallet, intended `admin.royltee-dev.testnet` (not created yet) |
| `ADMIN_PUBLIC_KEY` | support admin | optional. If set to an `ed25519:` public key, admin sign-in and admin routes also require that exact key. Not set in the environment in this pass (needs a separate yes). |
| `NEAR_RPC_URL` | support | optional, default `https://test.rpc.fastnear.com` |
| `KV_REST_API_URL`, `KV_REST_API_TOKEN` | both | private Upstash storage for chat state, rate limits, sessions and tickets, as set by the Vercel marketplace integration. Fallback names: `UPSTASH_REDIS_REST_URL`, `UPSTASH_REDIS_REST_TOKEN`. The read only token is not used. |
| `STORAGE_MODE` | local only | `memory` uses in process storage; never for production |
| `CRON_SECRET` | ticket sweep | 32+ random characters; sent by the scheduler as a bearer token |
| `CHAT_SESSION_CAP`, `CHAT_DAY_CAP`, `CHAT_WALLET_DAY_CAP`, `CHAT_GLOBAL_DAY_CAP`, `CHAT_SIGNED_RESERVE` | help chat | optional caps, defaults 12 per session, 40 per client address per day, 20 per signed-in wallet per day, 800 model calls per day overall, of which 200 are reserved for signed-in wallets. Client addresses are grouped by IPv6 /64. The per-client and per-wallet caps are checked first and are not keyed on the chat session, so a new session does not reset them. A chat turn holds a lock with a unique owner token for at most 90 seconds; if another request takes the turn over after that, the first answer is discarded instead of overwriting the history. A turn refused by any limit gives back every charge it made, and refused turns do not count against the daily budgets. |

Missing storage or secrets make the API answer 503; it never falls back to unsafe defaults.

## Help chat

- Grounding: `src/content/help-notes.md`, written from the repository docs and the public testnet state. The chat answers only from it and says when something is not built.
- Model: NEAR AI Cloud chat completions, OpenAI compatible. Base URL `https://cloud-api.near.ai/v1` (docs.near.ai quickstart, models, and OpenAI compatibility guide). Default model `z-ai/glm-5.3-flash`, overridable with `NEARAI_MODEL`. Live `GET /v1/model/list` on 2026-11-13 listed it with `ownedBy` nearai, `providerType` vllm (documented as TEE-enabled), and `attestationSupported` true. The JavaScript inference SDK page uses the same id as a supported NEAR TEE model. GLM-5.3 Flash reasons by default, so requests send `reasoning_effort: low` and `chat_template_kwargs.enable_thinking: false` (docs.near.ai reasoning models page). Checked 2026-11-13.
- TEE reporting: the chat completion body has no provider_tee field (OpenAPI ChatCompletionResponse, confirmed on live replies). After each completion the server reads `GET /v1/signature/{id}` and sets `tee` to true only when `signature_kind` is the exact string `provider_tee` (documented as a model-serving TEE). Missing, false, gateway, or malformed values yield `tee` false. The help page shows the label "Inference ran in a TEE (reported by NEAR AI Cloud)" only on those replies. Attestation is not verified by this app, and the contracts still use no TEE.
- Every assistant turn carries an HMAC SHA-256 over (version label, session id, turn index, text, tee flag). The server also keeps a digest of the whole accepted history per session, so a tampered, unsigned, reordered or replayed history gets a 400 before any model call. Comparison uses `timingSafeEqual`.
- A model reply is accepted only when `finish_reason` is `stop`. A cut-off or other finish is a 503, with nothing signed and no TEE label.
- Limits: messages per session, model calls per client address per day, and model calls per day overall, with a reserve for signed-in wallets. Refused turns do not count against those daily budgets. Origin allowlist and JSON content type are checked on every POST.
- Jailbreak, off topic and personal data messages get a short polite refusal without a model call. Replies that echo the prompt or secret names are withheld, and links outside the app are removed.
- Message bodies are never logged. Errors log only the route and an error class.

## Support tickets

- Sign in: the server issues a one time 32 byte nonce (5 minutes) bound to the site host and the client. The wallet signs it with NEP-413 `signMessage` through NEAR Connect. The server rebuilds the NEP-413 payload (tag 2^31+413, Borsh message, nonce, recipient, no callback URL), checks the ed25519 signature over its SHA-256, consumes the nonce, and calls testnet RPC `view_access_key` to confirm the key is a full access key of that account. The session is an HMAC signed HttpOnly, SameSite Strict cookie that lasts 15 minutes and is also revoked on sign out. On Vercel, and on any https request, the cookie is marked Secure and uses the `__Host-` prefix.
- Tickets: subject up to 120 characters, message up to 2000. Each submit needs a one time submit nonce from the list call. Rate limits apply per wallet and per client. A wallet may have at most 10 tickets that are not closed. The count check and the write run under a per wallet slot reservation taken with one atomic counter increment, so concurrent submits cannot pass the cap together; a second submit from the same wallet in the same moment is asked to try again (review 03b L3). Lists are newest first and accept a cursor for older pages. Paging uses an exclusive (creation time, id) boundary that does not need the cursor ticket to exist, and keeps reading through tickets on the same millisecond and expired entries until the page is full or the index ends; `tests/review-03b.test.ts` covers an expired cursor and 104 tickets on one millisecond. A conservative filter refuses text that looks like an email address, phone number, card number (Luhn, checked before any number format exception; a dotted amount such as 1.000.000 is let through only next to a currency), recovery phrase (12 BIP39 words in a row) or private key. Stored only in private server storage, keyed to the wallet account id; never on chain, never logged. Every read filters by the signed in wallet.
- Admin: `/admin` and `/api/admin/*` check on the server that the signed in wallet equals `ADMIN_WALLET`. If `ADMIN_PUBLIC_KEY` is set, the signing public key must also match that value. The owner can list, reply and close. Closing sets a 30 day storage TTL, and a sweep (on admin and user reads, and from the daily cron in `vercel.json`) removes due records and index entries. Open tickets expire 30 days after the last update. Exception until the backfill below is run: tickets opened before this rule shipped were stored with no expiry and are not in the per wallet open index, so they do not expire and do not count toward the cap. Whether any such tickets exist in production storage is UNVERIFIED. The admin only route `POST /api/admin/backfill` runs an idempotent backfill: by default it is a dry run that reports how many open tickets it would index, give a TTL (30 days from their last update) or delete as already expired; with the body `{"apply": true}` it makes those changes. It has not been run against any deployed storage (review 03b L1).
- Admin status: `admin.royltee-dev.testnet` is not created yet and `ADMIN_PUBLIC_KEY` is not set, so the admin console is not live on the hosted site. Until both are set, any full access key of `ADMIN_WALLET` would open it. The admin flows are tested in Vitest only.
- No email is sent and no paid service is added beyond the configured free storage tier and the chat model key.

## Security headers

`site/vercel.json` sets `X-Frame-Options: DENY`, `X-Content-Type-Options: nosniff`, `Referrer-Policy`, `Permissions-Policy`, and `Strict-Transport-Security` on all routes, and the strict Content-Security-Policy (`script-src 'self'`, `frame-ancestors 'none'`, `object-src 'none'`) on every route except the two wallet sign in pages, `/admin` and `/help`. `style-src` allows `'unsafe-inline'` because Astro pages emit inline `<style>` blocks. `npm run build` also copies these headers into `.vercel/output/config.json` so they apply to CLI deploys that use the Build Output API. Google Fonts are not used on our own pages; fonts are self-hosted through `@fontsource-variable` on the main pages and through `/fonts` on the dashboard mockup. Both policies are built in `src/server/csp.ts`, and `tests/csp.test.ts` checks that `vercel.json` matches it.

Wallet sign in pages. `@hot-labs/near-connect` runs each wallet in a sandboxed `srcdoc` iframe with inline scripts, and that iframe inherits the page CSP, so `script-src 'self'` stops every wallet from opening. `/admin` and `/help` are rendered per request; `src/middleware.ts` gives each response a fresh random nonce and sends the wallet policy instead:

- `script-src 'self' 'nonce-<per response>' 'wasm-unsafe-eval'`. Inline script runs only if it carries this response's nonce, which the page passes to the selector through its `cspNonce` option. No `'unsafe-inline'`, no `'unsafe-eval'`. `'wasm-unsafe-eval'` allows WebAssembly compile for wallet signing code, not JS eval.
- `style-src`, `img-src`, `font-src` and `connect-src` accept any `https:` host (and `data:` fonts, `wss:` connections), because wallets load their own styles, images, fonts and backends from hosts that change without notice. `frame-src` keeps the wallet host list. `frame-ancestors`, `object-src`, `base-uri` and `form-action` are unchanged.
- These responses are `Cache-Control: private, no-store`, so a nonce is never reused.

What the wallet code can and cannot do (review 03b M1). Checked in `@hot-labs/near-connect` 0.12.0 (`build/SandboxedWallet/iframe.js`): each wallet iframe gets `sandbox="allow-scripts"` and nothing else, so it has an opaque origin. It cannot read this page's DOM or storage, and it cannot send requests as this site with the admin's cookie (the cookie is HttpOnly and SameSite Strict, and POSTs check the origin allowlist). The wallet executor code is fetched at run time from the hosts named in the wallet manifest, and the default manifest is itself fetched from `raw.githubusercontent.com` (hot-dao/near-selector, `main` branch) with a `cdn.jsdelivr.net` fallback. So whoever controls that manifest or an executor host controls which wallet code runs inside the sandbox and what it asks the user to sign. Residual risks: a hostile wallet could show a misleading signing prompt, and with `https:` open in `connect-src`, `img-src`, `style-src` and `font-src` it can reach any https host. Tightening those four to fixed hosts was not done because the hosts each wallet uses are not fixed or published (for example Meteor loads from its own app host and third party font hosts), so a fixed list cannot be verified. Possible further steps, not done: pass a pinned `manifest` (the `NearConnector` option accepts a URL or an object) at a fixed commit, set `ADMIN_PUBLIC_KEY`, and move admin sign in to a page that offers one wallet only.

Checked in headless Chrome against a local production build on 2026-11-13: with the testnet wallet list (Meteor, Intear, Ledger, Nightly, NEAR CLI) every wallet opened with no CSP error; Meteor Web and Intear Web opened their login windows. UNVERIFIED: a full signed sign in through each wallet, and the policy on the live preview.

The `frame-src` wallet hosts and the strict policy `connect-src` hosts are taken from `@hot-labs/near-connect` 0.12.0 (default manifest URLs in the package, plus executor, icon, and `allowsOpen` hosts in the default manifest retrieved on 2026-11-13) together with the default `NEAR_RPC_URL` host `https://test.rpc.fastnear.com` and `https://rpc.testnet.near.org`.

Chat budgets (review 03b L4). The anonymous share and the overall cap are counted per UTC calendar day under the same day key, so they reset together and anonymous calls cannot use the signed in reserve. The daily caps bound cost (800 model calls a day overall). They do not guarantee availability: about 15 IPv4 client addresses at 40 calls a day can use the 600 anonymous calls, and testnet accounts with a full access key are free to create, so about 10 wallets at 20 a day can use the 200 calls reserved for signed in wallets. The tests show that one client cannot drain a budget, not that several cannot. When a budget is used up, chat refuses with "You have reached the limit. Please try again later." until the window resets.

UNVERIFIED: that the `crons` entry in `vercel.json` is picked up for this adapter's build output. Deletion does not depend on it, because the storage TTL deletes closed and expired tickets on time.
