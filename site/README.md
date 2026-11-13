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
| `CHAT_SESSION_CAP`, `CHAT_DAY_CAP`, `CHAT_GLOBAL_DAY_CAP`, `CHAT_SIGNED_RESERVE` | help chat | optional caps, defaults 12 per session, 40 per client address per day, 800 model calls per day overall, of which 200 are reserved for signed-in wallets. Client addresses are grouped by IPv6 /64. Refused turns do not count against the daily budgets. |

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
- Tickets: subject up to 120 characters, message up to 2000. Each submit needs a one time submit nonce from the list call. Rate limits apply per wallet and per client. A wallet may have at most 10 tickets that are not closed. Lists are newest first and accept a cursor for older pages. A conservative filter refuses text that looks like an email address, phone number, card number (Luhn), recovery phrase (12 BIP39 words in a row) or private key. Stored only in private server storage, keyed to the wallet account id; never on chain, never logged. Every read filters by the signed in wallet.
- Admin: `/admin` and `/api/admin/*` check on the server that the signed in wallet equals `ADMIN_WALLET`. If `ADMIN_PUBLIC_KEY` is set, the signing public key must also match that value. The owner can list, reply and close. Closing sets a 30 day storage TTL, and a sweep (on admin and user reads, and from the daily cron in `vercel.json`) removes due records and index entries. Open tickets expire 30 days after the last update.
- No email is sent and no paid service is added beyond the configured free storage tier and the chat model key.

## Security headers

`site/vercel.json` sets Content-Security-Policy, `frame-ancestors 'none'`, `X-Frame-Options: DENY`, `X-Content-Type-Options: nosniff`, `Referrer-Policy`, `Permissions-Policy`, and `Strict-Transport-Security` on all routes. `style-src` allows `'unsafe-inline'` because Astro pages emit inline `<style>` blocks. `npm run build` also copies these headers into `.vercel/output/config.json` so they apply to CLI deploys that use the Build Output API. Google Fonts are not used; fonts are self-hosted through `@fontsource-variable` on the main pages and through `/fonts` on the dashboard mockup.

The CSP `connect-src` and `frame-src` wallet hosts are taken from `@hot-labs/near-connect` 0.12.0 (default manifest URLs in the package, plus executor, icon, and `allowsOpen` hosts in the default manifest retrieved on 2026-11-13) together with the default `NEAR_RPC_URL` host `https://test.rpc.fastnear.com` and `https://rpc.testnet.near.org`. Those wallet hosts are UNVERIFIED until a live wallet sign-in is tested against this CSP. The live preview header set is also UNVERIFIED until a preview is probed.

UNVERIFIED: that the `crons` entry in `vercel.json` is picked up for this adapter's build output. Deletion does not depend on it, because the storage TTL deletes closed and expired tickets on time.
