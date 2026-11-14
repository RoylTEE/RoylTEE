# RoylTEE help notes

Sources: the repository README, DESIGN.md, THREAT_MODEL.md, DEPLOY_PLAN.md, DECISIONS_LOG.md, config/project.json, the contract code, and the public testnet state read on 2026-11-13 (UTC). If a question is not covered here, say it is not covered and point to the closest page.

## What RoylTEE is

- A permissioned registry of music rights shares on NEAR testnet. Each share is an NFT that controls its own payout vault.
- Product line: RoylTEE turns each share of a song into an NFT with its own verified payout address, for human and AI music alike. A writer's share of a song can be sold, split or disputed, but the place royalties are sent should never break. Each share's payout account has no keys, so nobody can sign money out of it. Payers and AI agents can send money in, and only the holder of the share NFT can collect it.
- It is a business to business record among verified industry members. It is not sold to the public, it is not an investment product, and nothing here is legal advice.
- The token is a record of a signed assignment plus a payment instruction among members. It is evidence and a payout address, not legal title. A participation agreement that would give the record legal effect is roadmap.
- Built at NEAR HACKATHON for the track "NFTs That Do Something".
- Testnet only. Payments use Mock USDC (testnet), a mock token with no value.

## Live on NEAR testnet

- Registry contract: reg.royltee-dev.testnet. NFT name RoylTEE, symbol ROYLT, spec nft-1.0.0. Registrar: registrar.royltee-dev.testnet. Token: usdc.royltee-dev.testnet.
- Mock token: usdc.royltee-dev.testnet. Name "Mock USDC (testnet)", symbol mUSDC, 6 decimals. Only the simulated payer account (payer.royltee-dev.testnet) can mint it.
- As of 2026-11-14 (UTC) the contracts are deployed and initialised, and the whole demo is on chain: three verified members (writer-a, writer-b and buyer-pub under royltee-dev.testnet), seven fictional works 0 to 6 (ISRC codes starting ZZRYL), and 15 shares s1 to s15, each with its own vault holding zero keys. Share s7 is void after the dispute.
- On chain: a Simulated payer work payment split 0.5 and 0.5 mUSDC into s1 and s2, a plain transfer into s3 picked up by sync, a claim by writer-a on s1, and the catalog sale: writer-b transferred s2 to buyer-pub, who then claimed the 0.5 mUSDC that moved with it. A claim on s1 by an account without the share failed on chain.
- Also on chain: the scripted payment agent paid work 1 from its allowance, and its second submit of the same statement line was refused. The dispute scene is on chain too: a 20 percent share on work 2 opened dispute 0, the 0.7 mUSDC for s5 was held in escrow, and the registrar resolved it (uphold existing) with a document hash, releasing the escrow to s5.
- Refused before submission, with no transaction: the agent's claim attempt with a 1 yoctoNEAR deposit. A function call key cannot attach a deposit, so the attempt never reached the chain; it was seen in a local CLI log only.
- Later on 2026-11-13 (UTC), on the updated registry: a second Simulated payer payment on work 0 (line-sale-2) paid s1 and s2 0.5 mUSDC each; writer-b, who sold s2, was refused when trying to claim it; buyer-pub claimed it. Works 3 (provenance ai_assisted), 4 (fully_ai, shares on a contract basis) and 5 were registered with shares s8 to s13. The scripted agent paid work 1 with its function call key, was refused when one payment went over its per transaction limit, and paid work 5 correctly even though the work's metadata link and the call itself named outsider as the payee: the contract ignored both and paid s12 and s13 only.
- The deployed registry has the contract review fixes and the audit fix for the payment forward gas (redeployed 2026-11-14 02:43 UTC) and the final contract fixes (redeployed 2026-11-14 09:09 UTC). The split sheet (approve or reject a share) is on the deployed build; all 15 shares are approved. Vaults s1 to s7 keep the vault code they were created with, which cannot receive a token transfer call; s8 to s13 run the newer vault code.
- Split sheet scene on chain (2026-11-14 UTC, work 6): writer-a registered the work and its own 50 percent share s14, which is approved at once. writer-b registered the other 50 percent as s15, which is pending because writer-b did not register the work. A Simulated payer payment of 1.0 mUSDC on work 6 paid 0.5 to s14 and held the 0.5 for s15 in escrow. Then writer-a signed approve_share on s15 (event share_approved) and the held 0.5 mUSDC was released to the s15 vault.
- Share s10 (work 4) was sold in a sealed-bid auction on a separate auction contract on 2026-11-14 (UTC): writer-a listed it, two sealed bids came in, and the settle ran through the OutLayer TEE. buyer-pub won at the second price, 70 mUSDC (Simulated money), so s10 is now owned by buyer-pub. The app has a read only Auction page (/auction) for that sale.
- The registry and token accounts still hold one full access key each, so the code can still be fixed in place. On testnet the owner decided to keep them so late fixes stay possible. On mainnet both keys would be deleted after the final deploy (or upgrades handed to a multisig or DAO) and proven with the same zero keys check the vaults pass. Do not claim the registry is keyless.
- Vault accounts are created by the registry with no keys at all, ever.

## How it works

- Works: a member registers a song by ISRC (12 characters) or ISWC (T plus 10 digits). Only the shape of the code is checked. The title is stored as a hash; personal data stays off chain.
- Shares: a member registers a share of a work for one right type (master, publishing, performance or mechanical) and a territory ("WW" or a two letter country code), in basis points (1 to 10000). A group (work, right, territory) holds at most 16 shares.
- Mint is two phase: the share stays pending until its vault account exists and is registered with the token. If either step fails, no NFT is minted and the unused deposit is refunded.
- Each share NFT owns a vault account s<N>.reg.royltee-dev.testnet. The vault has no keys. Its only method is withdraw, and only the registry can call it.
- Payments: a payer pays a work once with ft_transfer_call to the registry, naming the work group and a statement. The registry splits the amount by basis points and forwards each part to the share vaults in the same receipt chain. Splits use integer math; the remainder goes to the share with the largest basis points, ties to the lowest token number, so the parts add up to the payment exactly.
- A plain transfer straight into a vault also works and is picked up by sync, which only the holder or the registrar can call. A plain transfer to the registry itself is not attributed and stays stuck by design; there is no sweep.
- Claims: only the current holder can claim, and the money goes to that holder. A claim locks the share until it settles, so the share cannot be sold mid payout.
- Catalog sale: the holder transfers the NFT to another verified member. Unclaimed money in the vault moves with the token, and the seller cannot claim after the sale. The sale price itself is settled off chain in this demo.
- Transfers only go to accounts with an active credential. Marketplace approvals are disabled.

## Members and the registrar

- Only verified members (accounts with an active credential) can register works, hold shares and receive them.
- The registrar grants and suspends credentials, grants the payer role, and resolves disputes. Every registrar action that moves a share or money carries a document hash and emits an event; sync and revoke_role take none. The deployed registry refuses a credential for the registrar account.
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

KYB and identity, the fiat rail and payer statements, payer attestations, provenance evidence (human, AI assisted, fully AI is a declared field), arbitration documents, license legal terms, the agent (scripted, not a model), and USDC (a mock token).

## The registry pages

- The registry is an app with a menu by purpose. Registry: Overview (statement since mint and what needs attention), Catalog (works and rights, split sheets), Shares (ledger, share accounts with their live keys and NEAR against the storage minimum, claims), Activity (the recorded demo, filterable), Verify (code hashes against the deploy record, keyless share accounts, the full access keys still on the registry and Mock USDC accounts), Auction (a read only page for the one sealed-bid sale that ran in a TEE, with its timeline, transactions and attestation proof). Oversight: Governance (agents, disputes, registrar). You: Your shares. Support: Help (chat, support, docs, whitepaper). The whitepaper (version 1.0, 14 November 2026) is a page of its own at /whitepaper, with a PDF download. Balances, owners, keys and code hashes are read live from NEAR testnet with view calls; Activity, the Registrar log, share holders and history, and the landing split sheet are the recorded demo.
- Your shares: look up any account read only, or connect a testnet wallet. A connected holder can claim, a work registrant can approve or reject a pending share (on the deployed build), and a member can draft a split sheet with a review step before signing. Sending these from the site has not been run on testnet yet, so it is UNVERIFIED.
- Statements on these pages use Opening balance, activity and Closing balance since mint. A view by statement period needs an event indexer, which is roadmap.

## Not built

- TEE: the registry build does not use a trusted execution environment. TEE attestation of statements is roadmap. The help chat can report a provider TEE flag from NEAR AI Cloud when a reply's signature has signature_kind provider_tee. Attestation is not verified by this app.
- TEE auction: the app has a read only Auction page (/auction) for one real sale. A sealed bid share auction ran on testnet as a separate contract (share s10 was sold in it, the settle ran in OutLayer's TEE and outlayer-verify passed all 5 checks), but this app does not list, bid or settle, and the registry itself does not use a TEE.
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
- Do I need a wallet to read the site? No. A wallet is needed to open support tickets, and to claim or sign split sheets on the Your shares page.
- Where is the code? On GitHub at github.com/RoylTEE/RoylTEE, under the MIT license.

## In app links

- [Home](/)
- [How it works](/#how)
- [How it works in detail](/how-it-works)
- [Holder powers and agent limits](/security#agents)
- [Lifecycle and catalog sale](/how-it-works#lifecycle)
- [Trust and privacy](/security#trust)
- [Security](/security)
- [FAQ and glossary](/faq)
- [Testnet notice](/#notice)
- [Registry overview](/dashboard/)
- [Works and rights](/catalog/)
- [Split sheets](/catalog/splits/)
- [Share ledger](/shares/)
- [Share accounts and keys](/shares/accounts/)
- [Claims](/shares/claims/)
- [Recorded activity](/activity)
- [Agents](/agents)
- [Disputes](/disputes)
- [Registrar](/registrar)
- [Your shares](/member)
- [Look up a code, share or account](/catalog/lookup/)
- [Verify the deployment](/verify)
- [Sealed-bid auction](/auction)
- [Docs](/docs)
- [Whitepaper, version 1.0](/whitepaper)
- [Help chat](/help#chat)
- [Contact support](/help#support)
