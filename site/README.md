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
| `NEAR_RPC_URL` | support | optional, default `https://test.rpc.fastnear.com` |
| `KV_REST_API_URL`, `KV_REST_API_TOKEN` | both | private Upstash storage for chat state, rate limits, sessions and tickets, as set by the Vercel marketplace integration. Fallback names: `UPSTASH_REDIS_REST_URL`, `UPSTASH_REDIS_REST_TOKEN`. The read only token is not used. |
| `STORAGE_MODE` | local only | `memory` uses in process storage; never for production |
| `CRON_SECRET` | ticket sweep | 32+ random characters; sent by the scheduler as a bearer token |
| `CHAT_SESSION_CAP`, `CHAT_DAY_CAP`, `CHAT_GLOBAL_DAY_CAP` | help chat | optional caps, defaults 12 per session, 40 per client per day, 800 per day overall |

Missing storage or secrets make the API answer 503; it never falls back to unsafe defaults.

## Help chat

- Grounding: `src/content/help-notes.md`, written from the repository docs and the public testnet state. The chat answers only from it and says when something is not built.
- Model: NEAR AI Cloud chat completions, OpenAI compatible. Base URL `https://cloud-api.near.ai/v1` (docs.near.ai quickstart, models, and OpenAI compatibility guide). Default model `z-ai/glm-5.3-flash`, overridable with `NEARAI_MODEL`. Live `GET /v1/model/list` on 2026-11-13 listed it with `ownedBy` nearai, `providerType` vllm (documented as TEE-enabled), and `attestationSupported` true. The JavaScript inference SDK page uses the same id as a supported NEAR TEE model. GLM-5.3 Flash reasons by default, so requests send `reasoning_effort: low` and `chat_template_kwargs.enable_thinking: false` (docs.near.ai reasoning models page). Checked 2026-11-13.
- TEE reporting: the chat completion body has no provider_tee field (OpenAPI ChatCompletionResponse, confirmed on live replies). After each completion the server reads `GET /v1/signature/{id}` and sets `tee` to true only when `signature_kind` is the exact string `provider_tee` (documented as a model-serving TEE). Missing, false, gateway, or malformed values yield `tee` false. The help page shows the label "Inference ran in a TEE (reported by NEAR AI Cloud)" only on those replies. Attestation is not verified by this app, and the contracts still use no TEE.
- Every assistant turn carries an HMAC SHA-256 over (version label, session id, turn index, text). The server also keeps a digest of the whole accepted history per session, so a tampered, unsigned, reordered or replayed history gets a 400 before any model call. Comparison uses `timingSafeEqual`.
- Limits: messages per session, per client per day and per day overall. Origin allowlist and JSON content type are checked on every POST.
- Jailbreak, off topic and personal data messages get a short polite refusal without a model call. Replies that echo the prompt or secret names are withheld, and links outside the app are removed.
- Message bodies are never logged. Errors log only the route and an error class.

## Support tickets

- Sign in: the server issues a one time 32 byte nonce (5 minutes) bound to the site host and the client. The wallet signs it with NEP-413 `signMessage` through NEAR Connect. The server rebuilds the NEP-413 payload (tag 2^31+413, Borsh message, nonce, recipient, no callback URL), checks the ed25519 signature over its SHA-256, consumes the nonce, and calls testnet RPC `view_access_key` to confirm the key is a full access key of that account. The session is an HMAC signed HttpOnly, SameSite Strict cookie that lasts 15 minutes and is also revoked on sign out.
- Tickets: subject up to 120 characters, message up to 2000. Each submit needs a one time submit nonce from the list call. Rate limits apply per wallet and per client. A conservative filter refuses text that looks like an email address, phone number, card number (Luhn), recovery phrase (12 BIP39 words in a row) or private key. Stored only in private server storage, keyed to the wallet account id; never on chain, never logged. Every read filters by the signed in wallet.
- Admin: `/admin` and `/api/admin/*` check on the server that the signed in wallet equals `ADMIN_WALLET`. The owner can list, reply and close. Closing sets a 30 day storage TTL, and a sweep (on admin and user reads, and from the daily cron in `vercel.json`) removes due records and index entries. Open tickets have no expiry.
- No email is sent and no paid service is added beyond the configured free storage tier and the chat model key.

UNVERIFIED: that the `crons` entry in `vercel.json` is picked up for this adapter's build output. Deletion does not depend on it, because the storage TTL deletes closed tickets on time.
