# RoylTEE: design whitepaper

**Version 1.0, 14 November 2026.** A verified payout address for every share of every song, human or AI. Built at NEAR HACKATHON, track NFTs That Do Something.

**Testnet only.** The build uses Mock USDC on NEAR testnet, a token with no value. Every payer, statement, identity check and dispute document in the demo is Simulated. This paper separates what is built and checked from what is Simulated and from what is UNVERIFIED (Section 10).

## Abstract

Ownership of a song is split among writers, publishers, labels and others, and each share can be sold. The records of who owns which share, and where its money should go, sit in separate databases that do not agree, so a payer often cannot tell whom to pay. RoylTEE is a permissioned registry on the NEAR blockchain. Each rights share of a work is a non-fungible token, and each token controls its own on-chain account, called a vault, that receives the money of that share. A vault has no access keys. Only the registry contract can move money out of it, and only to whoever holds the token at that moment, so the payout address of a share never changes when the share is sold. Only its controller does.

Payers, including software agents, pay once per work. The registry splits the payment by basis points into the share vaults with a public rounding rule. Agents spend only from capped allowances and never choose the payee. A share registered by someone other than the work registrant waits for the registrant to sign, and its money waits in escrow until then. Disputes, suspensions and unassigned shares are handled by a registrar whose powers are listed, bounded and logged. A share can also be sold in a sealed-bid second-price auction whose bids are opened inside a trusted execution environment (TEE).

The system is a record and payment instruction among verified industry members. It is not sold to the public and is not an investment product. It exists only on NEAR testnet.

**Status of this paper (version 1.0).** The final registry is deployed on testnet. Every feature described here is built and covered by tests: keyless vaults, minting, claims, transfers with their locks, payment routing and splits, suspension, the unallocated escrow, agent allowances, the over-allocation dispute and its resolution, and split sheet approval with escrow (`approve_share`). The on-chain demo shows a catalog sale, an agent that pays within limits and is refused outside them, a dispute opened and resolved, and a split sheet scene in which a pending share is approved and its held money is released. The 15 share vaults on testnet have zero access keys. A sealed-bid auction ran once end to end on testnet and settled through the real OutLayer TEE, and the `outlayer-verify` tool passed all 5 of its checks on the settle transaction. The registry, vault and Mock USDC contracts do not use a TEE. Only the auction settle step does.

## Contents

1. The problem
2. Design goals
3. System overview
4. Lifecycle
5. On-chain design on NEAR
6. The sealed-bid auction and the TEE
7. Privacy design
8. Trust model, keys and the path to mainnet
9. Why NEAR
10. What is Simulated, what is UNVERIFIED
11. Limitations and open questions
12. Roadmap
13. References

## 1 The problem

### 1.1 Ownership data is fragmented

A commercially released song usually has at least two kinds of rights. The composition (melody and lyrics) is identified by an International Standard Musical Work Code, or ISWC, a reference number that identifies "a musical work as a unique intangible creation" [1]. The recording is identified by an International Standard Recording Code, or ISRC, which identifies sound recordings and music videos and "is not used to identify compositions/musical works" [2]. Each of these rights is divided into shares held by writers, publishers, labels and others, and the division can differ by right type and by territory.

The codes say what a work is, not who owns each share today or where its money should go. That information lives in split sheets, contracts, society registrations and publisher and distributor databases, each kept by a different party. When they disagree, money is held or misdirected until someone reconciles them.

The industry has tried to build one authoritative database before. The Global Repertoire Database was reported scrapped in July 2014 after collection societies fell out over its funding, with about £8m of publisher and songwriter money spent since 2008 [3].

The scale of unmatched money is visible in public data from the United States. Under the Music Modernization Act, digital services transferred their "historical unmatched royalties" to the Mechanical Licensing Collective (The MLC): United States digital audio mechanical royalties they had not been able to match and pay to the owners. The MLC reports that 21 services made these transfers, with an initial value of approximately $427 million, later adjusted to approximately $397 million [4]. The MLC also separates unmatched royalties, where a use cannot be linked to a registered work, from unclaimed royalties, where the work is known but less than 100 percent of its shares have been claimed [5]. These figures come from the cited sources and not from this project.

### 1.2 Payers do not know who owns a share or where to pay

A payer, such as a service, a licensee or a distributor, needs two answers for each payment: who owns each share now, and which account receives it. Today these come from the payer's own copy of the ownership data, which can be stale and which the payer cannot easily check. Software agents have the same problem in a sharper form. An AI agent that pays or licenses music for a business has no authoritative source to read, and anything it reads, including text in metadata, can be written by an attacker.

### 1.3 Payouts take several hops

Money for a share usually passes through several intermediaries, such as a society or distributor and then a publisher or label, before it reaches an artist or writer. Each hop applies its own records, timing and reporting. We state no typical delay or cost, because we have found no source we can cite (UNVERIFIED). The structural point needs no number: every hop is another copy of the ownership data that can be wrong.

### 1.4 Catalog sales need payout redirection

Shares are sold, often in bulk as part of a catalog. After a sale, every payer has to be told to pay the new owner, and until each one updates its records, money keeps arriving at the old owner and has to be passed on. How long this takes in practice is UNVERIFIED. We make no claim about typical lags.

### 1.5 Music made with AI

A registry built now has to cover music that is human-made, AI-assisted or fully AI-generated, and the law treats these differently. The United States Copyright Office concluded in January 2025 that AI outputs are protected only where a human author has determined sufficient expressive elements, and that using AI to assist human creativity does not bar protection [6]. So a registry cannot assume every share is a copyright interest. Some payment entitlements rest on contract instead. Each share records its basis: copyright, contract or none.

## 2 Design goals

- **G1. One payout address per share.** Selling a share changes who controls its account, never the address payers use.
- **G2. Complete hand-over.** After a sale the seller has nothing left: no key, no approval, no pending payout that can land in the wrong place.
- **G3. Pay once per work.** One payment per work and right, split exactly by the registered shares.
- **G4. Agents can pay but cannot steal.** Agents pay within limits set by their organisation, and the contract decides who is paid.
- **G5. Bounded powers.** Holder, agent and registrar powers are listed and enforced by the contract. Every registrar action that moves a share or money carries a document hash and emits an event.
- **G6. Exact accounting.** Every unit of a payment is credited to a share, held in a named escrow, or sent back. Rounding creates and loses nothing.
- **G7. Minimal disclosure.** Private documents stay off chain and appear on chain only as hashes.
- **G8. Consent before money.** A share that the work registrant has not approved is paid nothing until the registrant signs.
- **G9. Honest scope.** The token is evidence of a registered share and a binding payment instruction among members. It is not legal title, not sold to the public and not an investment product.

## 3 System overview

### 3.1 Actors

- **Registrar.** The operator of the registry. It grants and suspends member credentials, grants the payer role, resolves disputes and performs the small set of corrective actions listed in Section 8.3. In the testnet build the registrar is a single account. A multisig council with a time delay is planned.
- **Credentialed members.** Organisations and individuals in the music business, such as labels, publishers, distributors, societies, artists, writers and AI music companies, who have passed verification and hold an active credential. Only members can register works, hold shares and receive share transfers. In the testnet build verification is Simulated.
- **Payers.** Any account that sends money to a work or a share. A payer can also hold a payer role, granted by the registrar, which lets it attest that it has seen a share in its own records.
- **AI agents.** Software that pays on behalf of a member organisation. Each agent is a named sub-account of its organisation, holds only a narrowly scoped key, and spends only from an allowance its organisation has funded inside the registry.
- **Auction contract.** A separate contract that is itself a credentialed member. It holds a share while the share is for sale in a sealed-bid auction (Section 6).

Judges, auditors and readers are observers, not actors. The system gives them no role.

### 3.2 Objects

- **Work.** A composition (keyed by ISWC) or a recording (keyed by ISRC), with a declared provenance of human, AI-assisted or fully AI, a title hash and a pointer to off-chain metadata. The account that registers the work first is its registrant and signs its split sheet.
- **Share.** A fraction of one right (master, publishing, performance or mechanical) in one work for one territory, in basis points (1 basis point is 0.01 percent, and 10,000 is the whole). Each share records its basis.
- **Share token.** One non-fungible token per share. Whoever holds the token holds the share in the registry.
- **Vault.** A NEAR account created for one share at mint, named under the registry's account, with zero access keys. It holds that share's stablecoin.
- **Group.** All shares of the same work, right type and territory. Payments are split within a group.
- **Allowance and escrow.** Balances the registry holds itself: an agent's spending allowance, the unallocated escrow for parts of a group no share covers, and the escrow for contested fractions, which also holds the part of a payment that belongs to a share still waiting for approval.

### 3.3 How money moves

1. A payer or an agent pays a work once, through the registry. A payer sends one stablecoin transfer with a message. An agent spends from its allowance and never names a payee.
2. The registry splits the payment by basis points and forwards each part into the vault of its share. Parts for contested shares, unassigned parts and parts for shares waiting for approval go to a named escrow instead.
3. The holder of a share asks the registry to pay out its vault. The registry pays the account that holds the token at that moment, and nobody else.
4. When a share is sold, the vault address stays the same and the next payment lands in it. Only the new holder can claim.

### 3.4 What a token does

A share token is useful only because of what it controls. Holding it gives exactly three powers: claim the vault balance to the holder's own account, transfer the token to another credentialed member, and set a short public note on the share. A holder cannot change the split of a work, its provenance, or any other share. Marketplace approvals are switched off, so no third party can be authorised to move a token.

## 4 Lifecycle

This section follows one share from registration to sale, then shows how disputes, approvals and suspensions interrupt the cycle.

### 4.1 Register a work

A credentialed member registers a work with its code (ISRC or ISWC), a title hash, its provenance and a metadata pointer. Malformed codes are refused. A code already registered is recorded as a duplicate, which opens a dispute at once. Payments to the duplicate are refused until the registrar resolves it, while the original entry keeps being paid (Section 4.7).

### 4.2 Register shares, mint, and split sheet approval

A member registers a share by naming the work, right type, territory, basis points, basis, holder (also a member) and the hash of an evidence document such as a signed split sheet. The registry reserves the basis points, so two registrations in progress cannot together pass 100 percent, and mints in two phases. First it creates the vault under its own name, funds its code storage and deploys the vault code, adding no key. Then it registers the vault with the stablecoin contract. Only when both succeed is the token minted to the holder. If either fails, no token exists, the reservation is released and the unused deposit is sent back.

Each share carries an evidence tier: self-asserted at registration, counterparty-confirmed once every other holder in the same work and right has confirmed it, and payer-confirmed once an account with the payer role has attested to it (and only while that role holds). Payers can choose which tiers they pay.

**Approval.** A share registered by the work registrant is approved at once. A share registered by anyone else is pending until the work registrant, or the registrar, signs `approve_share`. A pending share is paid nothing: the part of each payment that belongs to it is held in escrow with the reason `awaiting_approval`, and its vault stays at 0. When the registrant approves, the registry emits a `share_approved` event and releases the held money to the vault. If the registrant (or the registrar) rejects the share, it becomes void, its basis points are freed and its held money moves to the group's unallocated escrow. Consent therefore comes before the money moves.

On testnet (work 6, Simulated money), share s14 was registered by the registrant and approved at once. Share s15 was registered by the other writer and stayed pending. A payment of 1.0 mUSDC put 0.5 into the s14 vault and held 0.5 for s15 in escrow. After the registrant signed `approve_share`, the held 0.5 mUSDC was released into the s15 vault and the escrow read 0.

### 4.3 Pay

A payer sends one stablecoin transfer to the registry with a message naming the work, right type and territory, the period and a hash of the payer's statement. The token standard lets such a transfer deliver its message to the receiving contract and notify it [7]. A plain transfer notifies no one, which is why payments go through the registry rather than to a per-song account.

An agent pays from its allowance instead. It names a work or share, an amount, a statement line identifier, the period and the statement hash, but never a payee. Before any money moves, the registry checks the agent's role, expiry and organisation credential, the per-payment, rolling 24-hour and count limits, the allowance, and that the statement line has not been paid before. A second submission of the same line pays nothing.

### 4.4 Split

In the same flow, the registry splits the payment across the group's payable shares and forwards each part straight into its vault. No balance waits for anyone to trigger a split. The rule is:

- **Rounding rule.** Each share is credited the payment times its basis points divided by 10,000, rounded down to a whole unit. If the shares total exactly 10,000 basis points, the few units left over go to the share with the most basis points, and if two shares tie, to the one with the lower token number. The credits then add up to the payment exactly.
- **Under 100 percent.** If the registered shares total less than 10,000 basis points, each share is still credited exactly its own fraction, rounded down. Everything not credited, the unassigned part plus rounding units, goes to the group's unallocated escrow. Only the registrar can release that escrow, and only to an active share in the same group, with a document hash and an event.
- **Over 100 percent.** If the shares to be paid would total more than 10,000 basis points, the payment is refused and goes back to the payer. A claimant share in an open dispute is not among the shares paid (Section 4.7).
- **Waiting for approval.** The part for a pending share goes to escrow with the reason `awaiting_approval` (Section 4.2).

For example, a payment of 10 units to shares of 3,333, 3,333 and 3,334 basis points credits 3, 3 and 3 units after rounding down. The one unit left goes to the 3,334 share, which receives 4. The credits sum to 10. Each payment's event lists every credit, so anyone can recompute the split.

### 4.5 Claim

The holder asks the registry to pay out the vault balance. The registry reads the token's owner at the moment of the call and pays that account only. The caller cannot name another receiver. A claim must carry exactly one yoctoNEAR, the smallest unit of NEAR, which a limited key cannot attach [8]. The registry sets a payout in flight lock and debits its record before telling the vault to send. It restores the debit only if the transfer failed (Section 5.4). While the lock is set the share cannot be claimed again or transferred.

### 4.6 Transfer and catalog sale

A holder transfers a share by sending its token to another credentialed member. The receiver must hold an active credential, and a transfer to anyone else fails. The transfer is refused while a payout is in flight, while the share is part of an open dispute, and while the holder is suspended.

Nothing is handed over except the token. The vault has no key to pass on, the seller has no approval left behind, and from then on the registry pays only the new holder. Any unclaimed balance stays in the vault and travels with the token. A catalog sale is therefore a set of token transfers, and payers change nothing: the next payment lands in the same vaults and the buyer claims it. A transfer can record an effective from date, but in the current design that date is history only. Payout goes to whoever holds the token at claim time.

On testnet, share s2 was sold by one writer to a buyer. A later payment to work 0 gave the buyer's vault its 0.5 mUSDC, the seller's claim was refused on chain, and the buyer claimed.

When a token is sent to a contract with a call, the standard lets that contract send the token back. A transfer pending lock during the call stops the receiving contract from claiming the balance and then giving the token back.

### 4.7 Dispute and resolve

Two events open a dispute automatically.

**Over-allocation.** A member registers a share that would take a group above 100 percent, naming the shares it contests. Only the contested fraction of new money is held. Uncontested shares are paid as normal and the contested shares' parts go to dispute escrow. The claimant share is not paid meanwhile, and neither it nor the contested shares can be transferred. Money already in a contested vault stays claimable up to a snapshot taken when the dispute opens. Stablecoin sent straight into a contested vault after that, bypassing the registry, is held in the vault, labeled unattributed, and cannot be claimed until resolution.

**Duplicate code.** A second registration of the same ISRC (for recordings) or ISWC (for compositions) opens a dispute. The original entry keeps being paid and payments to the duplicate are refused. Resolution can void the duplicate, keep both (for example where two different works were wrongly given one code), or flip which entry is payable.

The registrar resolves a dispute by naming an outcome and the hash of the decision document. For an over-allocation it either upholds the existing shares (escrow goes to them, the claimant share becomes void) or upholds the claimant with new basis points that keep the group at 100 percent (escrow is split under the new points by the same rounding rule). Resolution lifts the freeze and emits an event.

On testnet, one writer registered a 20 percent share on work 2, contesting share s5. Dispute 0 opened and the share minted as claimant was s7. A payment of 1.0 mUSDC paid s6 its 0.3 as usual and held 0.7 for s5 in dispute escrow. The registrar then resolved dispute 0 with `uphold_existing` and a document hash (a placeholder value in this demo): s7 became void and the 0.7 was released to s5 only.

### 4.8 Suspension

The registrar can suspend a member's credential, with a document hash. Suspension does not take the member's shares: each stays put and keeps receiving money. While suspended, the holder cannot claim, transfer out or receive, and its agents cannot pay. Only the registrar can move a suspended member's share, to an active member, with a document hash and an event, and only while the suspension lasts. Reinstatement restores every holder power.

### 4.9 States of a share

| State | Meaning |
|---|---|
| Pending (mint) | The vault is being made and registered. If either step fails, there is no token. |
| Waiting for approval | The token exists, but the work registrant has not signed. Its money is held in escrow. |
| Active | Paid on every payment to its work. Locks apply here: payout in flight, transfer pending. |
| Disputed | A dispute freezes transfers and holds the contested new money in escrow. |
| Holder suspended | The holder cannot claim or transfer, the money keeps arriving. |
| Void or rejected | Kept as a record and never paid again. |

## 5 On-chain design on NEAR

### 5.1 Tokens

Share tokens follow NEAR's non-fungible token standard, NEP-171, with its metadata and enumeration extensions [9]. Its transfer methods require exactly one yoctoNEAR, which keeps them out of reach of limited keys. The approval extension, NEP-178, which lets an owner authorise a marketplace to move a token [10], is switched off: approval calls fail and transfers that name an approval are refused.

Money is a stablecoin following NEAR's fungible token standard, NEP-141 [7]. Accounts must register with a token contract before holding it, under the storage standard NEP-145 [11]. The registry registers each vault at mint and each member when its credential is granted, so a claim never goes to an account that cannot receive it. On testnet the stablecoin is a mock token labeled "Mock USDC (testnet)", symbol mUSDC, 6 decimals. It is not Circle USDC.

Each token's view shows its work, right, territory, basis points, basis, provenance, evidence tier, status and approval, and three amounts: earned, paid out and claimable, all as of the last sync. Payments routed through the registry are recorded as they arrive. A plain transfer straight to a vault notifies no one, so it appears only after the holder or the registrar calls sync, which reads the vault's balance from the token contract. Earned is the recorded balance plus everything paid out.

### 5.2 Vaults

Each vault is a NEAR account named under the registry's account. On NEAR, only an account can create sub-accounts of itself, and an account has no control over its sub-accounts once they exist [12]. The registry creates the vault, funds it, deploys code to it, and never adds a key. An account with no keys is locked: no outside actor can sign transactions for it, and only its own contract can move its assets [13].

The vault code is deliberately tiny and does one thing: send an amount of a token to a receiver. It obeys only its parent, which it derives from its own account name, so it stores no owner and has no setup step anyone could call first. It has no upgrade method and no way to add keys or deploy code. Each vault holds its own copy of this code.

On testnet there are 15 share vaults, s1 to s15, and every one has zero access keys, checked live through the node's `view_access_key_list` call. Vaults s1 to s7 run the vault code of the first registry build, and vaults s8 to s15 run the code the current registry deploys. A vault keeps the code it was created with.

### 5.3 Payment flow

A payment arrives at the registry as a token transfer with a message. The registry first checks that it came from the one stablecoin contract fixed at setup. Anything else, including a fake token with the same name, is sent back in full and recorded nowhere. It then checks that the target is payable, computes the credits and schedules one forward to each vault. A forward that fails is recorded as undelivered for that share, held by the registry, and anyone can ask for one retry.

Agent allowances, the dispute and approval escrow (one total), the unallocated escrow and undelivered forwards each have a running total, and their sum must never exceed what the registry holds. A plain transfer to the registry without a message belongs to none of them, and no method can move it. There is no administrative sweep: stray transfers stay stuck by design, which removes one way the operator could move money.

### 5.4 Locks and ordering

On NEAR, cross-contract calls are asynchronous: the callback runs one or two blocks later, there is no automatic rollback, and other calls can reach the contract in between [15]. A callback is guaranteed to run but not to succeed [16]. These facts are the main source of risk around transfers.

- **Debit before the call.** A claim debits the registry's record and adds to paid out before it asks the vault to send money, as NEAR's security guidance recommends [17]. A repeat call in the meantime finds nothing to claim.
- **Payout in flight.** Set by a claim until its callback runs. Blocks claim, transfer and sync on that share.
- **Transfer pending.** Set by a transfer with a call until the transfer is settled. Blocks claim, transfer, note changes and sync on that share.
- **Stale lock release.** If a callback fails and a transfer lock is left set, anyone may clear it after 1200 blocks (about 12 minutes at the block time measured on testnet). A stuck payout lock is never cleared by code. Anyone may ask for a read that only reports the vault balance, and the lock stays until the real late callback or the registrar settles it with the final public receipt outcome. A payment forward whose callback never ran can be quarantined by anyone after about a day (150,000 blocks): it credits nothing and resends nothing, because no block count proves the original transfer cannot still land. Whether these waiting periods are long enough under congestion is UNVERIFIED.
- **Sync guard.** Every change to a share's record increments a counter. A sync that started before a change is discarded when it completes.
- **Stale mint release.** A mint whose callbacks never commit releases its reserved basis points after 20 minutes, and late callbacks mint nothing.

### 5.5 Agents

A NEAR function-call key can call only one named contract, optionally only listed methods, and cannot attach NEAR to a call [13]. Sending a stablecoin needs a one-yoctoNEAR deposit, so an agent holding only such a key cannot send stablecoin itself, and the design does not try to work around this. The organisation funds the agent's allowance inside the registry, and the agent's key may call only the registry's payment and reporting methods, which take no deposit. Every owner and administrative method requires the one-yoctoNEAR deposit as well as a role or ownership check, so an agent key cannot reach them even if a role check were wrong. Revocation takes effect on the agent's next call.

Because the contract looks up the vaults itself, text in metadata or share notes saying "pay account X" has no effect, and idempotency keyed by organisation and statement line makes a replayed line pay nothing. On testnet the scripted agent's full-access key was deleted on chain, so it holds a function-call key only. It paid from its allowance, was refused one unit over its per-transaction limit, was refused a replayed statement line, and paid a work whose metadata told it to pay another account with the contract choosing the payees and the other account receiving 0.

### 5.6 Events and history

Every state change emits an event in NEAR's NEP-297 format [18]: the NFT standard's own mint and transfer events, plus registry events for registrations, approvals, payments with per-share credits, claims, syncs, credential and role changes, allowance use, disputes and registrar actions. Each registrar event carries the hash of the document that justifies it. A share's history, covering what it earned, what was paid out and who held it, is rebuilt from these events, each line linking to its transaction on a block explorer.

### 5.7 The registry account and its code

The registry contract has no upgrade method of its own. On the testnet deployment the registry account still holds one full-access key, kept on purpose (Section 8.2), so whoever holds that key can redeploy the registry. That key signed three in-place redeploys, the last on 14 November 2026 at 09:09 UTC. It was one transaction made of a deploy action followed by a call of `assert_quiescent`, so a registry with a payment or lock in flight would have rolled the deploy back. Its code hash is `5YNk7mvmiq4SuSckR1gULXVvgcRdUjaioxmDE8niguAG` (sha256 `4378568f0459560130cdb56fa05152f9946116d3332d6c3f7f53e939005dc271`, 748115 bytes). The code hash on chain equals a build of the published source on the build machine, and anyone can check the hash against `deployments/testnet.json`. Whether a build on another machine is byte for byte identical is UNVERIFIED.

### 5.8 What the final registry fixes

The first registry build on testnet was reviewed, and the final build fixes the findings below. The full inventory with a test for each is in the repository (`docs/reviews`, `THREAT_MODEL.md`).

| Finding | First build | Final build on testnet |
|---|---|---|
| H1 sync | Anyone can call sync, and the phantom balance race is open | Holder or registrar only. New vaults keep `ft_transfer_call` tokens |
| H2 contests | No contest list means every active share of the group is contested | The list is mandatory and minimal. A share cannot name its own shares |
| H3 registrar credential | The registrar account can be granted a credential | Refused |
| M1 stuck mints | A mint that never commits blocks its basis points | Released after 20 minutes. Late callbacks mint nothing |
| M2 split sheet | Anyone with a credential can register a share on someone else's work and be paid at once | Held until the work registrant or the registrar approves it |
| L2 agent window | The rolling limit can be spent twice about 23 hours apart | Covers a full 24 hours |
| L4 sync epochs | A sync in flight across a dispute or a transfer call can land | Discarded |
| L5 payer attestation | Payer confirmed stays after the payer role is revoked | Counts only while the role holds |
| L6 mint to a suspended holder | Minted | The share fails and no token exists |
| L8 minimum | A share payment of 1 base unit is accepted | Same minimum as a group payment |
| L9 void work escrow | Unallocated escrow of a void work is stranded | The registrar can refund it |

A checked record of the contracts is kept in the repository: 232 Rust unit tests (155 registry, 5 vault, 4 Mock USDC, 68 auction), 54 sandbox tests, 14 WASI host tests and 18 Node tests, all passing on 14 November 2026. The command `npm run verify:testnet` reads testnet and prints one PASS line per check, 72 checks in all, and ended with ALL CHECKS PASSED on 14 November 2026.

## 6 The sealed-bid auction and the TEE

### 6.1 What it does

A holder can sell one share in a private auction run by a separate auction contract. A bidder seals an amount to the public key of a TEE. After the end time, a small WASI module running inside OutLayer's TEE opens the bids and prints only the winner and the price. The contract then moves the share and the money. The price is the second highest valid bid, or the reserve if there is only one bid.

1. **List.** The seller sends the share to the auction contract with `nft_transfer_call`, giving a reserve, a maximum deposit and an end time. The auction contract keeps the share. It needs no change to the registry, because it is an ordinary credentialed member.
2. **Bid.** A bidder encrypts the amount to the TEE public key, padded to a fixed length, and sends exactly the maximum deposit in Mock USDC with the ciphertext as the message. Every deposit is the same size, so the deposit leaks nothing about the bid. Only active members can bid, and at most 8 bids are accepted.
3. **Settle.** After the end time anyone calls `settle`. The contract asks OutLayer to run the module, which is pinned by its sha256. The contract accepts the answer only if the winner is a bidder, the price lies between the reserve and the maximum deposit, and the hash of the bid list matches the one the contract sent.
4. **Pay.** The share goes to the winner first, and the registry re-checks membership. Only then does the contract credit the seller the price, the winner the change, and each loser the full deposit. Money leaves with `withdraw`, which debits first, writes a pending record, sends, and closes the record in the callback.
5. **Failure.** A failed or empty answer reopens settling. A refused delivery or a stalled settle ends in a cancelled auction in which every deposit is credited back and the share returns to the seller.

### 6.2 The run on testnet

All money in this run is Simulated Mock USDC. Times are UTC, 14 November 2026.

| Step | What happened |
|---|---|
| Outsider refused, twice | An account with no credential asked OutLayer directly to open bids, first naming the auction as the contract and then naming itself. The OutLayer keystore refused both with `Access denied by access condition`. No instructions ran and the answer was null. |
| Listed | writer-a listed share s10 with a reserve of 20 mUSDC, a maximum deposit of 100 mUSDC and an end time of 10:38:50 UTC. The auction contract held s10. |
| Bids sealed | Two bidders each sent a sealed bid with a 100 mUSDC deposit. Only ciphertext was on chain. |
| Settled in the TEE | Settle ran at about 10:39 UTC through OutLayer (request 44422). buyer-pub won s10 at the second price, 70 mUSDC. The OutLayer charge was 0.0010383 NEAR and the rest of the 0.02 NEAR attached was refunded. |
| Withdrawn | The seller was paid 70 mUSDC, the losing bidder got its full 100 mUSDC deposit back and the winner got 30 mUSDC change. Afterwards the auction owed 0 and held no escrow. |

The sale price is public, and the sealed amounts of the bids are not shown by the record after settle. The record shows only the winner and the price.

### 6.3 What the attestation proves

The `outlayer-verify` tool (git revision `90c27d9`) checked the settle transaction (task 106286, executed at 10:41:00 UTC) and passed all 5 checks:

| Check | Question | Result |
|---|---|---|
| Authenticity | Is this a genuine Intel TDX quote? | PASS |
| Identity | Is this code approved on chain? | PASS |
| Binding | Does the quote cover this execution? | PASS |
| Input | Do the request bytes match what was attested? | PASS |
| Output | Does the response match what was attested? | PASS |

The module that ran has sha256 `c4a29e55ca61da2e069bdcfebd4d4e6690820620582ed2635d1dd6bc17c2594e`. The access rule for its secret is `And[WasmHash(c4a29e55...), Predecessor Whitelist[auction.royltee-dev.testnet]]`, so the secret opens only for this exact build when the call arrives through the auction contract. The check proves that this input produced this output inside genuine Intel TDX hardware running the approved build. It does not prove that the auction rules were followed. The verifier output is kept in the repository as `deployments/auction-settle-verify.txt`, and the page `/auction` of the site shows the run with its transactions.

### 6.4 What runs in the TEE and what does not

Inside the TEE: opening the sealed bids, which only the TEE can decrypt, and choosing the winner and the second price. Outside the TEE: the auction contract, the registry, the vaults and Mock USDC, all of which are ordinary NEAR contracts, and the fairness of the answer, which is not proven (Section 8.5).

### 6.5 Privacy of the auction

The winner's top bid and all losing bids except the second highest stay inside the module. The sale price is public, and with two bidders it is the loser's exact bid, so it is not true that losing bids are never published. Account names, the deposit and the time of each bid are public.

## 7 Privacy design

The design keeps private material off chain, but much of what a registry needs is necessarily public. We list both.

### 7.1 Public on chain

- Account names of members, agents, the registrar and every vault. Named accounts can reveal an organisation's name.
- Work codes (ISRC and ISWC are public identifiers by design), declared provenance and the metadata pointer.
- Every share's right, territory, basis points, basis, tier, status, approval and holder.
- Every payment, with payer, amount, period, statement hash and per-share credits, and every claim, transfer, dispute and registrar action, agent roles and limits.
- Auction accounts, deposits and bid times, and the winner and price of a sale.

### 7.2 Kept off chain, represented by hashes

- Contracts, split sheets and other evidence. A party holding the document can prove it matches the registered hash.
- Identity and business verification documents. A credential records only hashes of the organisation's name and the verification record.
- Song titles, payer statements and dispute decisions, each as a hash.
- Sealed bid amounts, which are encrypted to the TEE key.

### 7.3 Credentials are attestations

A member credential is an on-chain record, written by the registrar, that an account belongs to a verified organisation. It cannot be transferred and carries no personal data. Anyone can check that a holder is credentialed. The chain does not show what documents were seen. In the testnet build, verification is Simulated and the hashes are of dummy documents.

### 7.4 What is not private

We do not claim that the registry is private. Splits and payment amounts are public and may be commercially sensitive. A hash hides a long, random document well, but a short or guessable value such as a song title or an organisation name can be found by hashing likely candidates and comparing. Account names and transaction patterns can link activity to an organisation. Selective disclosure of split percentages, so that only parties to a work can see them, is a known open problem and is not part of this design.

## 8 Trust model, keys and the path to mainnet

### 8.1 What users must trust

- **The registrar.** To verify members honestly and resolve disputes fairly. The contract limits its powers but cannot judge whether a decision was right.
- **The registry contract and its key.** That the deployed code is the published code and behaves as described. On testnet the key that can redeploy it still exists (Section 8.2).
- **The stablecoin contract.** To behave as a standard fungible token. On mainnet this would be the issuer's contract, which this registry does not control.
- **NEAR validators.** To produce valid blocks. NEAR uses proof of stake, and a block needs approvals from producers holding more than two thirds of block producer stake [19].
- **The OutLayer executor, for the auction.** For honest opening of bids and a correct second price (Section 8.5).
- **The legal layer, when it exists.** Legal effect would come from a participation agreement among members, under which paying the registered share discharges the payer and disputes follow the registry process. It is planned and does not exist today.

### 8.2 Keys on testnet, by decision

| Account | Access keys on testnet |
|---|---|
| 15 share vaults, s1 to s15 | 0. Re-read on 14 November 2026 at about 11:03 UTC, s14 and s15 included. |
| Registry (`reg.royltee-dev.testnet`) | One full-access key, kept on purpose. |
| Mock USDC (`usdc.royltee-dev.testnet`) | One full-access key, kept on purpose. |
| Auction (`auction.royltee-dev.testnet`) | One full-access key, kept on purpose, because its recovery calls need the account itself to call them. |
| Payment agent | A function-call key only. Its full-access key was deleted on chain. |

On 14 November 2026 the project owner decided to keep the registry and Mock USDC full-access keys on testnet. This is a demo with Simulated money, and keeping the keys means a late fix can still be deployed in place. It also means that whoever holds the registry key can replace the registry code, which is why the project never calls the registry keyless. The repository's `verify:testnet` command checks that the live keys equal the record, so a change cannot go unseen.

### 8.3 Registrar powers and their limits

| Power | Limit |
|---|---|
| Grant, suspend and reinstate member credentials | Document hash and event. Suspension never moves a share by itself. System accounts, agent accounts and the registrar itself cannot be credentialed. |
| Grant and revoke the payer role | Document hash and event. |
| Resolve a dispute | Only open disputes, only outcomes valid for the dispute type. Escrow is released by the rounding rule to the shares the outcome names. Document hash and event. |
| Approve or reject a pending share | The work registrant can do the same. A registrar action carries a document hash. A rejected share is void and never paid, its basis points are freed and its held money moves to the unallocated escrow. |
| Correct a share's basis points | Only inside an open dispute. Document hash and event. |
| Move a suspended member's share | Only while that member is suspended, only to an active member, subject to both locks. Document hash and event. |
| Release unallocated escrow | Only to an active share of the same work, right and territory, never more than that group holds. Document hash and event. |
| Refund the unallocated escrow of a void work | Only for a void work. |
| Send the plain deposits of a rejected share's vault elsewhere | To any account except the registrar. A stated trusted power. |
| Settle a lost forward or a stuck payout lock | With the final public receipt outcome. A wrong answer can pay a share twice from pooled registry funds or credit it what the vault does not hold. A stated trusted power. |

The registrar cannot claim any share, cannot transfer a share belonging to an active member, cannot send money to itself, cannot sweep the registry's balance and cannot change the contract code by any registry method. Every registrar method requires the one-yoctoNEAR deposit. In the testnet build the registrar is one account. A multisig council with a time delay before actions take effect is planned.

### 8.4 What users do not have to trust

- **Agents.** An agent can spend only its own allowance, within limits, to vaults the contract chooses. It cannot transfer, claim or change anything else.
- **The previous holder of a share.** After a transfer the seller has no key, no approval and no claim. Unclaimed balance belongs to the token.
- **Whoever registered the share.** The minter never had a key to the vault, and a registrant who is not the work registrant cannot be paid before the work registrant signs.

### 8.5 Known limits of the trust model

- **First registrant of a work.** The split sheet signer is whoever registered the work first. A member who registers someone else's code first gets its own shares approved at once, and the real owner's later registration opens a duplicate-code dispute that freezes nothing on the first work until the registrar resolves it. Not fixed in this build, by decision.
- **Contest size.** A claimant names whole shares and may name the largest one, so a 1 basis point claim can hold the new money of a 9,999 basis point share until the registrar resolves. Not fixed in this build, by decision. There is no challenger bond.
- **Stray transfers stay stuck.** Stablecoin sent to the registry without a message cannot be moved by anyone. This is intended, and the cost falls on whoever made the mistake.
- **Direct transfers bypass escrow arithmetic.** Money sent straight to a contested vault is held as unattributed until resolution rather than split by the escrow rules.
- **One registrar.** Until the council exists, the registrar is a single point of failure and of trust.
- **Declared provenance.** Human, AI-assisted and fully AI are declared fields. The evidence behind them is not checked.
- **Auction fairness rests on the executor.** The contract bounds the answer and conserves escrow, but it does not prove that bids were opened honestly or that the second price was computed correctly. Whoever controls the OutLayer answer can name any bidder as winner at any price between the reserve and the maximum deposit. The loss is capped by the maximum deposit per bidder and the share. There is no on-chain fix. OutLayer's own documents describe single operator keys today (UNVERIFIED beyond those documents). A NEAR contract cannot check a TEE quote, so verification is off chain with `outlayer-verify`.
- **Auction recovery is not autonomous.** A delivery or return whose callback is lost stays pending until the operator settles it with the auction account's retained key, from the final receipt outcome. A share sent to the auction with a plain `nft_transfer` is not tracked, and only a privileged rescue can move it.
- **The live keystore refusal** of the two outsider requests shows that the access rule works for those two attacks. It does not prove the module's own caller check on the live worker, because the keystore refused first. That check is covered by the sandbox and Node tests.

### 8.6 The path to mainnet

None of this is built for mainnet. The path is:

1. Run the full security pack on the final code and test the signing screens of the site with a real wallet, neither of which has been done.
2. Delete the registry and token full-access keys after the final deploy, or hand upgrades to a multisig or a DAO with a time delay. Deleting the keys is one-way: fixing a bug then means deploying a new registry and moving members to it.
3. Prove the result with the same check the 15 vaults pass today. `view_access_key_list` on the account returns an empty list, and the code hash equals a local build of the published source. The check needs no wallet and no key.
4. Replace Mock USDC with the issuer's USDC, run the registrar as a multisig council with a time delay, and decide the auction key question separately, since the key must not be deleted while any auction is delivering or returning or while a withdrawal is pending.

## 9 Why NEAR

The design depends on a few properties of NEAR, each documented by the protocol.

- **Named sub-accounts.** Only a named account can create sub-accounts under its name, and it has no control over them once they exist [12]. Each share gets a readable, permanent vault address, and anyone can tell from the name that the registry created it.
- **Keyless accounts.** An account with no keys is locked, and only its contract can move its assets [13]. A keyless vault has no key to sell, lose or steal.
- **Function-call keys.** Limited keys cannot attach NEAR [13], so the one-yoctoNEAR rule [8] gives agents a protocol-level limit independent of the registry's role checks.
- **Asynchronous calls with callbacks.** Cross-contract calls report back through callbacks in later blocks [15]. This is a cost as much as a benefit and is the reason for the locks in Section 5.4, but it also defines exactly where another call can interleave, so each case can be named and blocked.
- **Explicit storage and fees.** Storage is paid by locking NEAR at a fixed network price, currently 10^19 yoctoNEAR per byte, or 100 kilobytes per NEAR [20], so the cost of each vault and record is known in advance and charged to whoever creates it. Gas is charged in fixed units per action at a price with a protocol floor [21].
- **Standards and a stablecoin.** NEAR has published standards for NFTs, fungible tokens, storage registration and events [7][9][11][18], and Circle issues native USDC on NEAR mainnet as a NEP-141 token [22].
- **A TEE runtime.** OutLayer runs WASI modules inside Intel TDX hardware for NEAR contracts and publishes a record that a verifier can check against the chain [24].

NEAR Intents, a protocol in which users or agents state an outcome and third parties compete to fulfil it [23], may later let payers on other chains pay a work. That is planned only and is not part of the current design.

## 10 What is Simulated, what is UNVERIFIED

### 10.1 Real, Simulated and not built

| Real, on NEAR testnet or in the tests | Simulated |
|---|---|
| Registry, vault, Mock USDC and auction contracts | Identity checks behind a credential |
| 15 vault accounts with zero keys | The fiat rail and conversion to USDC. The Simulated payer mints Mock USDC instead |
| Mock USDC flows: payments, splits, claims, transfers | Payer statements and statement line identifiers |
| Agent limits, idempotency and revocation | Payer attestations by the Simulated payer |
| Locks, credentials, disputes, escrow and split sheet approval | Provenance evidence. The field is real and the evidence is not checked |
| Events and per-token history | Arbitration and dispute documents. Hashes of dummy documents |
| The auction settle through the real OutLayer TEE, with `outlayer-verify` passing 5 of 5 checks | Mock USDC itself, and all auction money. It has no value |
| The site's live values, read with view calls | The scripted agent, which is a program and not a model |
| | OutLayer inside the auction tests (a mock contract) |

Not built: TEE attested payer statements, an auction on mainnet, a legal participation agreement, and any real money.

### 10.2 UNVERIFIED

These items have not been checked, and nothing in this paper depends on them being true.

- Which commit the hosted site runs, its live security headers, whether the help chat and support tickets are configured there, and whether a wallet sign-in works there.
- Whether the custom domain royltee.app serves the site.
- Whether the site's signing screens work with a real wallet. No transaction has been sent from the site on testnet.
- Typical payout delays and costs, and typical lags after a catalog sale (no source cited).
- Whether a build of the contracts on another machine is byte for byte identical to the deployed code.
- Whether the waiting periods for stale locks are long enough under congestion.
- OutLayer's key custody beyond its own documents, and whether the live worker and keystore run the source version the project read.
- A live measurement of the auction's storage reserve, a run with more than two bids, and a failed or empty answer on the live OutLayer.
- A Circle USDC address on NEAR testnet, which is why Mock USDC is used.
- Whether global contracts, which pin code by hash, suit this design.
- Legal effect of the token anywhere. Nothing here is legal advice.

## 11 Limitations and open questions

- **Garbage in.** The registry records what members register and confirm. It does not discover ownership, and a wrong registration stays wrong until a dispute corrects it.
- **Legal effect.** Without a participation agreement the token does not discharge a payer's obligation. Whether such an agreement can bind across territories is an open legal question.
- **Period allocation.** A sale agreement may assign income by period, but the design pays the holder at claim time. A seller who wants pre-sale income must claim before transferring, or before listing in an auction, because unclaimed vault money travels with the share.
- **Fully AI works.** The basis field records a contract based entitlement where copyright may not exist. How payers will treat such shares is unknown.
- **Vault code copies.** Each vault stores its own copy of the vault code, which costs storage per share. Shared code pinned by hash could reduce this (UNVERIFIED).
- **Other tokens in a vault.** A new vault keeps any NEP-141 token sent to it with `ft_transfer_call`, from any token contract, and tokens other than Mock USDC stay there for good. Inventory beyond Mock USDC is not built.
- **Failed mints.** When a mint fails after the vault account was made, the keyless vault stays and the NEAR spent on it is not refunded.
- **Scale.** A group is limited to 16 shares so that one payment fits in one call's limits. With the gas used for agent calls an agent pays a group of at most 5 shares in one call, and a larger group needs a larger allowance of gas. Works with very many shares would need a different split method.
- **Adoption.** Nothing in this paper shows that payers, societies or labels will use a registry like this. That is the largest open question.

## 12 Roadmap

Nothing below is built, and none of it has committed dates, partners or issuers.

1. A participation agreement drafted with counsel that gives the record legal effect.
2. An independent security audit of the contracts, a reproducible build on any machine, and a runbook for deployment and key removal.
3. Deleting the registry and token full-access keys, or handing upgrades to a multisig or DAO with a delay (Section 8.6), with the zero-key proof.
4. A registrar run as a multisig council with a time delay, and real verification providers issuing credentials.
5. Mainnet with real USDC.
6. Fixes for the first registrant and contest size limits, challenger bonds and dispute auto-expiry.
7. Verified fair TEE execution of the auction rules, an autonomous recovery path for a lost delivery callback, and an auction on mainnet.
8. Selective disclosure of split percentages, TEE attested payer statements, period based income allocation, import from industry data formats, and payment from other chains with NEAR Intents.

## 13 References

1. CISAC, "What is an ISWC?", ISWC International Agency. https://www.iswc.org/iswc
2. IFPI, "The International Standard Recording Code", ISRC official site. https://isrc.ifpi.org/en/
3. "Global Repertoire Database officially scrapped, £8m wasted", Music Week, 7 July 2014. https://www.musicweek.com/news/read/global-repertoire-database-officially-scrapped-8-million-wasted/058940
4. The MLC, "Historical Royalties" (data as of October 2026). https://www.themlc.com/historical-royalties
5. The MLC, "Frequently Asked Questions: The MLC" (unmatched and unclaimed royalties). https://www.themlc.com/faqs/categories/mlc
6. U.S. Copyright Office, "Copyright and Artificial Intelligence, Part 2: Copyrightability", January 2025. https://copyright.gov/ai/Copyright-and-Artificial-Intelligence-Part-2-Copyrightability-Report.pdf
7. NEAR Documentation, "Fungible Tokens: The Standard" (NEP-141). https://docs.near.org/primitives/ft/standard ; NEP-141 text: https://github.com/near/NEPs/blob/master/neps/nep-0141.md
8. NEAR Documentation, "Require one yoctoNEAR". https://docs.near.org/smart-contracts/security/one_yocto
9. NEAR Documentation, "Non-Fungible Tokens: The Standard" (NEP-171, NEP-177, NEP-181). https://docs.near.org/primitives/nft/standard ; NEP-171 text: https://github.com/near/NEPs/blob/master/neps/nep-0171.md
10. NEP-178, "Non-Fungible Token Approval Management". https://github.com/near/NEPs/blob/master/neps/nep-0178.md
11. NEP-145, "Storage Management". https://github.com/near/NEPs/blob/master/neps/nep-0145.md
12. NEAR Documentation, "Address (Account ID)" (named accounts and sub-accounts). https://docs.near.org/protocol/accounts-contracts/account-id
13. NEAR Documentation, "Access Keys" (function-call keys, locked accounts). https://docs.near.org/protocol/accounts-contracts/access-keys
14. NEAR Documentation, "Global Contracts". https://docs.near.org/smart-contracts/global-contracts
15. NEAR Documentation, "Cross-Contract Calls". https://docs.near.org/smart-contracts/anatomy/crosscontract
16. NEAR Documentation, "Callbacks can fail". https://docs.near.org/smart-contracts/security/cross-contract-calls/callback-failures
17. NEAR Documentation, "Balance Deduction". https://docs.near.org/smart-contracts/security/cross-contract-calls/reentrancy
18. NEAR Documentation, "Contract Events" (NEP-297). https://docs.near.org/smart-contracts/anatomy/events ; NEP-297 text: https://github.com/near/NEPs/blob/master/neps/nep-0297.md
19. NEAR Documentation, "Validators". https://docs.near.org/protocol/network/validators
20. NEAR Documentation, "Storage" (storage staking). https://docs.near.org/protocol/storage/storage-staking
21. NEAR Documentation, "Gas (Execution Fees)". https://docs.near.org/protocol/transactions/gas
22. Circle, "USDC on NEAR". https://www.circle.com/multi-chain-usdc/near
23. NEAR Documentation, "NEAR Intents". https://docs.near.org/chain-abstraction/intents/overview
24. RoylTEE repository, OutLayer call path notes and the settle evidence: `spike/outlayer/README.md`, `deployments/auction-testnet.json` and `deployments/auction-settle-verify.txt`. https://github.com/RoylTEE/RoylTEE

RoylTEE is a working name. Testnet only. The build uses Mock USDC on NEAR testnet and all demo data is Simulated. Not an investment product.
