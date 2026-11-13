# Spend log

Testnet only. Real money spent so far: zero, except the two NEAR AI Cloud help-chat notes below: a smoke test (under $0.02, from existing credit, not a new credit purchase) and a preview chat injection run (about $0.05). Both amounts are UNVERIFIED estimates.

| Date (UTC) | Item | Network | Amount | Real money | Approved by |
|---|---|---|---|---|---|
| 2026-11-12 | Block 1 docs only. No accounts created, no deploys, no transactions. | none | 0 | 0 | not needed |
| 2026-11-13 | Step 1. Three faucet accounts (`royltee-dev.testnet`, `royltee-fund-1.testnet`, `royltee-fund-2.testnet`), 10 NEAR each from the testnet faucet (signer `testnet`). Faucet paid 0.0071149895375 NEAR fee per create, 0.0213449686125 NEAR in total. | testnet | +30 NEAR in (faucet); fees paid by the faucet | 0 | owner |
| 2026-11-13 | Step 2. Each fund account sent 9.9 NEAR to `royltee-dev.testnet`. Fee 0.0000446365125 NEAR each, 0.000089273025 NEAR in total. Each fund account kept 0.0999553634875 NEAR. | testnet | 19.8 NEAR moved to root; 0.000089273025 NEAR fees | 0 | owner |
| 2026-11-13 | Step 3. Root created eight project sub-accounts with 20.3 NEAR attached in total (reg 8, usdc 2.5, registrar 1, writer-a 4, writer-b 1.5, buyer-pub 2, payer 1, outsider 0.3). Create fee 0.0071149895375 NEAR each, 0.0569199163 NEAR in total, paid by root. | testnet | 20.3 NEAR funded; 0.0569199163 NEAR fees | 0 | owner |
| 2026-11-13 | Step 4. Deploy and init mock USDC in one transaction (`APzCzPmVMWStFkwXkJXYX4JQPQqYYghjkWKYR3c6vcmK`). Fee 0.001513192097854 NEAR, paid by the usdc account. | testnet | 0.001513192097854 NEAR fee | 0 | owner |
| 2026-11-13 | Step 5. Deploy and init the registry in one transaction (`Ckk8dvY7sR2fKpDBEQ1akEVx8QSviSNmJCKkyBRjpXeN`). Fee 0.0052473054807262 NEAR, paid by the registry account. | testnet | 0.0052473054807262 NEAR fee | 0 | owner |
| 2026-11-13 | Step 6. Delete full-access keys of the registry and mock USDC. Not run. Parked until the owner says go. | testnet | 0 | 0 | owner (parked) |
| 2026-11-13 | Step 7. Token storage for the registry and the payer (0.01 NEAR attached each) and mint 1000 mUSDC. Three transactions. Fees 0.0006609146415412 NEAR, paid by the payer. | testnet | 0.02 NEAR storage deposits; 0.0006609146415412 NEAR fees | 0 | owner |
| 2026-11-13 | Step 8. Three `grant_credential` calls, 0.02 NEAR attached each. Fees 0.0022421505136277 NEAR, paid by the registrar. | testnet | 0.06 NEAR attached; 0.0022421505136277 NEAR fees | 0 | owner |
| 2026-11-13 | Step 9. Three `register_work` (0.05 NEAR attached each) and six `register_share` (0.5 NEAR attached each). Most of each share deposit is refunded after vault storage is taken. Fees 0.0524679858232855 NEAR, paid by writer-a. | testnet | 3.15 NEAR attached (net of refunds stays as vault and registry storage); 0.0524679858232855 NEAR fees | 0 | owner |
| 2026-11-13 | Step 10. Work payment, plain transfer to s3, and `sync`. Three transactions, 1 yoctoNEAR on the two token calls. Fees 0.0025800863621557 NEAR (payer 0.0018963212180383, outsider 0.0006837651441174). | testnet | 0.0025800863621557 NEAR fees | 0 | owner |
| 2026-11-13 | Step 11. writer-a claimed s1, writer-b transferred s2 to buyer-pub, buyer-pub claimed s2. Then root sent 0.5 NEAR to outsider (`Agtkfe5qEGaXzYQJsHdUxXL7ELx4D432sM4bHPRSiM8R`, fee 0.0000446365125 NEAR). Then outsider's claim on s1 was refused (`GvcHLtG7vLQXihNgbH6cmGSnG8QWhsNgzYGFQ2URwAVB`, "only the current holder can claim", fee 0.0002155086236816 NEAR). Five transactions. Total fees 0.0025348046375589 NEAR. | testnet | 0.5 NEAR outsider top-up from root; 0.0025348046375589 NEAR fees | 0 | owner |
| 2026-11-13 | Step 12. buyer-pub created `pay-agent.buyer-pub.royltee-dev.testnet` with 0.5 NEAR (create fee 0.0071149895375 NEAR). Added a function-call key, granted the agent role (0.1 NEAR deposit, fee 0.000354890879669 NEAR), funded the allowance, paid, reported, deleted the agent's full-access key, and a duplicate pay was refused. Nine on-chain transactions, plus a claim that the protocol refused before a transaction was sent. Total fees 0.0103016814099867 NEAR. | testnet | 0.5 NEAR to the agent from buyer-pub; 0.1 NEAR grant_role deposit; 0.0103016814099867 NEAR fees | 0 | owner |
| 2026-11-13 | Step 13. writer-b registered a contesting share (0.5 NEAR attached, fee 0.0087471592398598 NEAR), the payer paid work 2 (fee 0.001243668011624 NEAR), and the registrar resolved the dispute (1 yoctoNEAR, fee 0.0008669013492981 NEAR). Three transactions. Total fees 0.0108577286007819 NEAR. | testnet | 0.5 NEAR writer-b deposit (net of refunds stays as s7 and dispute storage); 0.0108577286007819 NEAR fees | 0 | owner |
| 2026-11-13 | NEAR AI Cloud help-chat smoke test. About 16 calls. The executor estimated under $0.02 from existing credit. UNVERIFIED (no invoice or usage receipt checked for this record). No chain writes. | none (inference) | under $0.02 estimate, UNVERIFIED | UNVERIFIED | owner |
| 2026-11-13 11:45 to 11:47 (time zone UNVERIFIED) | Preview chat injection run. 44 API calls (about 37 model calls) on NEAR AI Cloud with the preview key. Cost about $0.05 (UNVERIFIED). | preview | about $0.05 (UNVERIFIED) | about $0.05 (UNVERIFIED) | approved before the run |

All 50 on-chain transaction ids are in `deployments/testnet.json`. Combined gas fees for steps 1 to 13: 0.1667600075050178 NEAR, of which 0.0213449686125 NEAR was paid by the faucet. Real money: 0, aside from the UNVERIFIED inference estimates above.

Native NEAR balances at the time of this record (RPC `view_account`, 2026-11-13, about 07:55 UTC, block heights 277481803 to 277482071):

| Account | Balance (NEAR) |
|---|---|
| royltee-dev.testnet | 8.9430354471875 |
| royltee-fund-1.testnet | 0.0999553634875 |
| royltee-fund-2.testnet | 0.0999553634875 |
| reg.royltee-dev.testnet | 8.221662694519273799999998 |
| usdc.royltee-dev.testnet | 2.513486807902146000000013 |
| registrar.royltee-dev.testnet | 0.986350948137074199999999 |
| writer-a.royltee-dev.testnet | 3.236132083328670799999999 |
| writer-b.royltee-dev.testnet | 1.372247840334463599999999 |
| buyer-pub.royltee-dev.testnet | 1.391116015880764599999998 |
| pay-agent.buyer-pub.royltee-dev.testnet | 0.4980569101896278 |
| payer.royltee-dev.testnet | 0.993484760420760399999996 |
| outsider.royltee-dev.testnet | 0.799100726232201 |
| s1.reg.royltee-dev.testnet | 0.099999999999999999999999 |
| s2.reg.royltee-dev.testnet | 0.099999999999999999999999 |
| s3.reg.royltee-dev.testnet | 0.1 |
| s4.reg.royltee-dev.testnet | 0.1 |
| s5.reg.royltee-dev.testnet | 0.1 |
| s6.reg.royltee-dev.testnet | 0.1 |
| s7.reg.royltee-dev.testnet | 0.1 |
