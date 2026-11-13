# RoylTEE help notes

Sources: the repository README, DESIGN.md, THREAT_MODEL.md, DEPLOY_PLAN.md, DECISIONS_LOG.md, config/project.json, the contract code, and the public testnet state read on 2026-11-13 (UTC). If a question is not covered here, say it is not covered and point to the closest page.

## What RoylTEE is

- A permissioned registry of music rights shares on NEAR testnet. Each share is an NFT that controls its own payout vault.
- Product line: the verified payout address for every share of every song, human or AI. The payout address never changes, only who controls it. An agent can pay it, but it cannot steal from it.
- It is a business to business record among verified industry members. It is not sold to the public, it is not an investment product, and nothing here is legal advice.
- The token is a record of a signed assignment plus a payment instruction among members. It is evidence and a payout address, not legal title. A participation agreement that would give the record legal effect is roadmap.
- Built at NEAR HACKATHON for the track "NFTs That Do Something".
- Testnet only. Payments use Mock USDC (testnet), a mock token with no value.

## Live on NEAR testnet

- Registry contract: reg.royltee-dev.testnet. NFT name RoylTEE, symbol ROYLT, spec nft-1.0.0. Registrar: registrar.royltee-dev.testnet. Token: usdc.royltee-dev.testnet.
- Mock token: usdc.royltee-dev.testnet. Name "Mock USDC (testnet)", symbol mUSDC, 6 decimals. Only the simulated payer account (payer.royltee-dev.testnet) can mint it.
- As of 2026-11-13 (UTC) the contracts are deployed and initialised, and part of the demo is on chain: three verified members (writer-a, writer-b and buyer-pub under royltee-dev.testnet), three fictional works (ISRC codes starting ZZRYL), and six shares s1 to s6, each with its own vault holding zero keys.
- Also live: a Simulated payer work payment split 0.5 and 0.5 mUSDC into s1 and s2, a plain transfer into s3 picked up by sync, a claim by writer-a on s1, and the catalog sale: writer-b transferred s2 to buyer-pub, who then claimed the 0.5 mUSDC that moved with it. A claim on s1 by an account without the share failed on chain.
- Not on chain yet: the scripted agent and the dispute scene. They are later deploy steps.
- The registry and token accounts still hold one full access key each, so the code can still be fixed in place. Deleting those keys (the zero keys proof for the registry) is planned once the code is final. Until then, do not claim the registry is keyless.
- Vault accounts are created by the registry with no keys at all, ever.

## How it works

- Works: a member registers a song by ISRC (12 characters) or ISWC (T plus 10 digits). Only the shape of the code is checked. The title is stored as a hash; personal data stays off chain.
- Shares: a member registers a share of a work for one right type (master, publishing, performance or mechanical) and a territory ("WW" or a two letter country code), in basis points (1 to 10000). A group (work, right, territory) holds at most 16 shares.
- Mint is two phase: the share stays pending until its vault account exists and is registered with the token. If either step fails, no NFT is minted and the unused deposit is refunded.
- Each share NFT owns a vault account s<N>.reg.royltee-dev.testnet. The vault has no keys. Its only method is withdraw, and only the registry can call it.
- Payments: a payer pays a work once with ft_transfer_call to the registry, naming the work group and a statement. The registry splits the amount by basis points and forwards each part to the share vaults in the same receipt chain. Splits use integer math; the remainder goes to the share with the largest basis points, ties to the lowest token number, so the parts add up to the payment exactly.
- A plain transfer straight into a vault also works and is picked up by sync, which anyone can call. A plain transfer to the registry itself is not attributed and stays stuck by design; there is no sweep.
- Claims: only the current holder can claim, and the money goes to that holder. A claim locks the share until it settles, so the share cannot be sold mid payout.
- Catalog sale: the holder transfers the NFT to another verified member. Unclaimed money in the vault moves with the token, and the seller cannot claim after the sale. The sale price itself is settled off chain in this demo.
- Transfers only go to accounts with an active credential. Marketplace approvals are disabled.

## Members and the registrar

- Only verified members (accounts with an active credential) can register works, hold shares and receive them.
- The registrar grants and suspends credentials, grants the payer role, and resolves disputes. Every registrar action carries a document hash and emits an event.
- The registrar cannot move vault money to itself, cannot transfer shares it does not hold and cannot claim. It can move a share only while its holder is suspended (registrar_transfer). A multisig council with a time delay is roadmap.
- KYB and organisation identity are Simulated: the credential hashes are hashes of dummy documents.

## Agents

- An agent is a sub-account of a member organisation that holds only a function call key for the registry, limited to pay_from_allowance and report.
- It spends from an allowance held inside the registry, with a per transaction limit, a rolling 24 hour limit and a maximum number of payments per day. It names a target (a work group or a share), never a payee, so injected text cannot redirect money.
- The same statement line pays once. Owner and admin methods need a deposit, which a function call key cannot attach.
- The demo agent is a scripted Node agent, not an AI model.

## Disputes

- If a new share would push a group above 100 percent, it becomes a claimant and a dispute opens. Only the contested shares are frozen, and only new money for them goes to escrow in the registry. Other shares keep getting paid, and money already in vaults stays claimable.
- A duplicate ISRC or ISWC opens a duplicate dispute; payments to the duplicate work are refused and the original is untouched.
- The registrar resolves with a document hash: uphold existing, uphold claimant with new basis points, void duplicate, keep both, or swap payable.

## What is Simulated

KYB and identity, the fiat rail and payer statements, payer attestations, provenance evidence (human, AI assisted, fully AI is a declared field), arbitration documents, license legal terms, the agent (scripted, not a model), and USDC (a mock token). The dashboard page is a mockup with example data and is not yet wired to testnet. The claim view on that page is a mockup too. It does not show a live claimable balance, and live vault balances are not shown in the app today. Claims happen on chain by the current holder.

## Not built

- TEE: the registry build does not use a trusted execution environment. TEE attestation of statements is roadmap. The help chat can report a provider TEE flag from NEAR AI Cloud when a reply's signature has signature_kind provider_tee. Attestation is not verified by this app.
- TEE auction: not built. A sealed bid share auction may come later as a TEE version, only after the core is done.
- Licenses: not built in the current code. Sync licenses were planned and cut first.
- The amend_split method is not built; a split only changes through a dispute ruling.
- No mainnet deploy, no real money, no public sale.

## This help chat and support

- The help chat answers from these notes only. When a NEAR AI Cloud key is set, questions go to NEAR AI Cloud (an OpenAI compatible API). A TEE label is shown only when NEAR AI Cloud reports signature_kind provider_tee for that reply. Attestation is not verified by this app. The site does not store chat text.
- Support tickets: sign in with a NEAR testnet wallet by signing a NEP-413 message. No email and no personal data. Tickets hold a subject, a message, a status (open, answered or closed) and the owner's reply. They live in private server storage tied only to the wallet account id, never on chain and never in logs. Open tickets expire 30 days after the last update. Closed tickets are deleted 30 days after they close. A sign in lasts 15 minutes.
- Please never put a seed phrase, private key, email address, phone number or card number in a ticket. The form refuses things that look like them.

## Common first questions

- Is this real money? No. Testnet only, Mock USDC (testnet).
- Can I buy a share? No. It is a permissioned registry among verified members, not a public sale.
- Do I need a wallet to read the site? No. A wallet is only needed to open support tickets.
- Where is the code? The source link on the site is a placeholder until the repository is public.

## In app links

- [Home](/)
- [How it works](/#how)
- [Agent allowances](/#agents)
- [Lifecycle and catalog sale](/#lifecycle)
- [Trust and privacy](/#trust)
- [Testnet notice](/#notice)
- [Testnet dashboard mockup](/dashboard/)
- [Whitepaper placeholder](/whitepaper)
- [Help chat](/help#chat)
- [Contact support](/help#support)
