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
| 2026-11-13 | Step 6. Delete full-access keys of the registry and mock USDC. Not run, by owner decision on 2026-11-14: on testnet the keys are kept so late fixes stay possible. | testnet | 0 | 0 | owner (decided not to run) |
| 2026-11-13 | Step 7. Token storage for the registry and the payer (0.01 NEAR attached each) and mint 1000 mUSDC. Three transactions. Fees 0.0006609146415412 NEAR, paid by the payer. | testnet | 0.02 NEAR storage deposits; 0.0006609146415412 NEAR fees | 0 | owner |
| 2026-11-13 | Step 8. Three `grant_credential` calls, 0.02 NEAR attached each. Fees 0.0022421505136277 NEAR, paid by the registrar. | testnet | 0.06 NEAR attached; 0.0022421505136277 NEAR fees | 0 | owner |
| 2026-11-13 | Step 9. Three `register_work` (0.05 NEAR attached each) and six `register_share` (0.5 NEAR attached each). Most of each share deposit is refunded after vault storage is taken. Fees 0.0524679858232855 NEAR, paid by writer-a. | testnet | 3.15 NEAR attached (net of refunds stays as vault and registry storage); 0.0524679858232855 NEAR fees | 0 | owner |
| 2026-11-13 | Step 10. Work payment, plain transfer to s3, and `sync`. Three transactions, 1 yoctoNEAR on the two token calls. Fees 0.0025800863621557 NEAR (payer 0.0018963212180383, outsider 0.0006837651441174). | testnet | 0.0025800863621557 NEAR fees | 0 | owner |
| 2026-11-13 | Step 11. writer-a claimed s1, writer-b transferred s2 to buyer-pub, buyer-pub claimed s2. Then root sent 0.5 NEAR to outsider (`Agtkfe5qEGaXzYQJsHdUxXL7ELx4D432sM4bHPRSiM8R`, fee 0.0000446365125 NEAR). Then outsider's claim on s1 was refused (`GvcHLtG7vLQXihNgbH6cmGSnG8QWhsNgzYGFQ2URwAVB`, "only the current holder can claim", fee 0.0002155086236816 NEAR). Five transactions. Total fees 0.0025348046375589 NEAR. | testnet | 0.5 NEAR outsider top-up from root; 0.0025348046375589 NEAR fees | 0 | owner |
| 2026-11-13 | Step 12. buyer-pub created `pay-agent.buyer-pub.royltee-dev.testnet` with 0.5 NEAR (create fee 0.0071149895375 NEAR). Added a function-call key, granted the agent role (0.1 NEAR deposit, fee 0.000354890879669 NEAR), funded the allowance, paid, reported, deleted the agent's full-access key, and a duplicate pay was refused. Nine on-chain transactions, plus a claim that the protocol refused before a transaction was sent. Total fees 0.0103016814099867 NEAR. | testnet | 0.5 NEAR to the agent from buyer-pub; 0.1 NEAR grant_role deposit; 0.0103016814099867 NEAR fees | 0 | owner |
| 2026-11-13 | Step 13. writer-b registered a contesting share (0.5 NEAR attached, fee 0.0087471592398598 NEAR), the payer paid work 2 (fee 0.001243668011624 NEAR), and the registrar resolved the dispute (1 yoctoNEAR, fee 0.0008669013492981 NEAR). Three transactions. Total fees 0.0108577286007819 NEAR. | testnet | 0.5 NEAR writer-b deposit (net of refunds stays as s7 and dispute storage); 0.0108577286007819 NEAR fees | 0 | owner |
| 2026-11-13 | NEAR AI Cloud help-chat smoke test. About 16 calls. The executor estimated under $0.02 from existing credit. UNVERIFIED (no invoice or usage receipt checked for this record). No chain writes. | none (inference) | under $0.02 estimate, UNVERIFIED | UNVERIFIED | owner |
| 2026-11-13 11:45 to 11:47 (time zone UNVERIFIED) | Preview chat injection run. 44 API calls (about 37 model calls) on NEAR AI Cloud with the preview key. Cost about $0.05 (UNVERIFIED). | preview | about $0.05 (UNVERIFIED) | about $0.05 (UNVERIFIED) | approved before the run |
| 2026-11-13 18:54 | Step 15. Registry redeployed in place with the contract review fixes, no init call (`9oR5awwxiJPouDhpYdTxTquskc8YpoFHdmAJy4SrrAMp`, signer `reg.royltee-dev.testnet`, full-access key). Code hash now `22JaDgJdfFRb2X9yGejNtgwBRbg3yuKQYptgjXMm6xv7` (sha256 `0f31232563b1d031ee6d6b6a03060784a018a78fecd812fa68419cb8aabc9e4c`). Fee 0.0051482685683912 NEAR, paid by the registry account. | testnet | 0.0051482685683912 NEAR fee | 0 | owner |
| 2026-11-13 20:23 | OutLayer spike. Root created `probe.royltee-dev.testnet` with 2.5 NEAR (`4GEXsjmgjmxEf979z98n9o9hTqHjXBUxgRkuScYjzgUs`). Fee 0.0071149895375 NEAR, paid by root. Fee read from NEAR RPC `EXPERIMENTAL_tx_status` (sum of `tokens_burnt`). | testnet | 2.5 NEAR funded; 0.0071149895375 NEAR fee | 0 | owner |
| 2026-11-13 20:29 | OutLayer spike. FastFS upload of the echo module (`Ht2ebzCBd8NpVAqcLWPRz4bQNoQqeyf2evLksghB4hDi`). The call fails out of gas by design; the indexer then serves the file. Fee 0.0001450760736569 NEAR, paid by the probe account. Fee read from NEAR RPC `EXPERIMENTAL_tx_status`. | testnet | 0.0001450760736569 NEAR fee | 0 | owner |
| 2026-11-13 20:30 | OutLayer spike. Deploy and init of the probe contract (`jT8VwctJf5Z1Sr4rmC9hg2qofGxsB7cUVubdpYVi3j5`). Fee 0.0015145889260714 NEAR, paid by the probe account. Fee read from NEAR RPC `EXPERIMENTAL_tx_status`. | testnet | 0.0015145889260714 NEAR fee | 0 | owner |
| 2026-11-13 20:31 | OutLayer spike. Probe call (`7LFZ3MoTENEP8cJfGYAcrgPhJ81es2CZkYDWjpfXzfQ1`), signed by root, 0.01 NEAR attached. OutLayer charged 0.0010626 NEAR and refunded 0.0089374 NEAR. The transaction's own gas fee was 0.0010867809661522 NEAR, paid by root, read from NEAR RPC `EXPERIMENTAL_tx_status`. | testnet | 0.0010626 NEAR OutLayer charge (0.0089374 NEAR refunded); 0.0010867809661522 NEAR gas fee | 0 | owner |
| 2026-11-13 21:22 | Demo write W1, catalog sale finish. The payer paid work 0 (1 mock USDC, line `line-sale-2`), split 500000 to `s1` and 500000 to `s2` (`2aAgWtcaEC1RinbQvVS1AoLpZn7zQeEmzAGW3FshXHQA`, 1 yoctoNEAR). Fee 0.0017066749198462 NEAR, paid by the payer. | testnet | 0.0017066749198462 NEAR fee | 0 | owner |
| 2026-11-13 21:23 | Demo write W2. writer-b, the seller of `s2`, tried to claim `s2` and was refused, as intended: "only the current holder can claim" (`EYDV3Tsv7Q1ZFPnJzmVdk6D4Rtosqpn8fYNY71ofVWJU`, 1 yoctoNEAR). Fee 0.0002196936345468 NEAR, paid by writer-b. | testnet | 0.0002196936345468 NEAR fee | 0 | owner |
| 2026-11-13 21:25 | Demo write W3. buyer-pub claimed `s2`: 500000 mock USDC from the `s2` vault (`ASynFkHYZ5CxZdSi6paosMVuZiQf66mvEusNn4mnxhiw`, 1 yoctoNEAR). Fee 0.0009080902076662 NEAR, paid by buyer-pub. | testnet | 0.0009080902076662 NEAR fee | 0 | owner |
| 2026-11-13 21:25 | Demo write W4. writer-a registered work 3, `ZZRYL2600005`, provenance `ai_assisted` (`3er9FiUfRj1msBQkDbM2WhxazMou8hdRw95xPPs1eyKA`, 0.05 NEAR attached). Fee 0.0003548000656753 NEAR, paid by writer-a. | testnet | 0.05 NEAR attached; 0.0003548000656753 NEAR fee | 0 | owner |
| 2026-11-13 21:26 | Demo write W5. Share `s8` on work 3, 5000 bps publishing WW, basis `copyright`, holder writer-a (`FGrwBa58Empz2zW1Yj2BTHPvaSw9ZjsrVrDT6PrbN4gf`, 0.5 NEAR attached, most refunded after vault storage). Fee 0.0086346952503379 NEAR, paid by writer-a. | testnet | 0.5 NEAR attached (net of refunds stays as vault and registry storage); 0.0086346952503379 NEAR fee | 0 | owner |
| 2026-11-13 21:27 | Demo write W6. Share `s9` on work 3, 5000 bps, basis `copyright`, holder writer-b, registered by writer-a (`BDE7EtebNBcfNX5PXCi2aniUCaLwQivKzQwpvSxq7Cft`, 0.5 NEAR attached). Fee 0.0087325777911823 NEAR, paid by writer-a. | testnet | 0.5 NEAR attached (net of refunds stays as vault and registry storage); 0.0087325777911823 NEAR fee | 0 | owner |
| 2026-11-13 21:28 | Demo write W7. writer-a registered work 4, `ZZRYL2600006`, provenance `fully_ai` (`BRt579jXQx8bhwbMHhtaykeik7D86TWLBVgEX3VNEp1K`, 0.05 NEAR attached). Fee 0.0003549112767883 NEAR, paid by writer-a. | testnet | 0.05 NEAR attached; 0.0003549112767883 NEAR fee | 0 | owner |
| 2026-11-13 21:29 | Demo write W8. Share `s10` on work 4, 5000 bps, basis `contract`, holder writer-a (`46nERgHy9ydk3x58zwqMCmrrVSnvMJQNMKd26BhpPebe`, 0.5 NEAR attached). Fee 0.008633699407357 NEAR, paid by writer-a. | testnet | 0.5 NEAR attached (net of refunds stays as vault and registry storage); 0.008633699407357 NEAR fee | 0 | owner |
| 2026-11-13 21:30 | Demo write W9. Share `s11` on work 4, 5000 bps, basis `contract`, holder writer-b, registered by writer-a (`vhe6Vgoc9btbQia4ArgcsbygYiECAmbSm58hqTy1qph`, 0.5 NEAR attached). Fee 0.0087339991035348 NEAR, paid by writer-a. | testnet | 0.5 NEAR attached (net of refunds stays as vault and registry storage); 0.0087339991035348 NEAR fee | 0 | owner |
| 2026-11-13 21:31 | Demo write W10. The scripted agent `pay-agent.buyer-pub.royltee-dev.testnet`, signing with its function-call key `ed25519:DtkqdkEc...`, paid work 1 from its allowance: 100000, split 60000 to `s3` and 40000 to `s4`, line `agent-line-2` (`2XjtVThM4g7tRkt79XCj1inLj1bBk3vNZiKAbUmQsBaW`, no deposit). Fee 0.0014075996661175 NEAR, paid from the agent key's gas allowance. | testnet | 0.0014075996661175 NEAR fee | 0 | owner |
| 2026-11-13 21:33 | Demo write W11. The agent's over-limit payment was refused, as intended: "over per-transaction limit" (`7Vu5wRDqx7dj487LAk3e69wFt6FmH8bMNfzpL8E2LuaW`, no deposit). Fee 0.0002417414181848 NEAR, paid from the agent key's gas allowance. | testnet | 0.0002417414181848 NEAR fee | 0 | owner |
| 2026-11-13 21:35 | Demo write W12. writer-a registered work 5, `ZZRYL2600007`, provenance `human`, with injected text in `metadata_uri` that asks to pay outsider instead (`ryAcUUtRGdxyzsraWZjJE3SnytCbZzpsmZNNCmNFq3y`, 0.05 NEAR attached). Fee 0.0003584183166343 NEAR, paid by writer-a. | testnet | 0.05 NEAR attached; 0.0003584183166343 NEAR fee | 0 | owner |
| 2026-11-13 21:35 | Demo write W13. Share `s12` on work 5, 5000 bps, basis `copyright`, holder writer-a (`GJopxb2zVGHNEgSUq9RKFHA3wmBYjcyM4wVmhZH5NcKd`, 0.5 NEAR attached). Fee 0.0088117648705073 NEAR, paid by writer-a. | testnet | 0.5 NEAR attached (net of refunds stays as vault and registry storage); 0.0088117648705073 NEAR fee | 0 | owner |
| 2026-11-13 21:36 | Demo write W14. Share `s13` on work 5, 5000 bps, basis `copyright`, holder writer-b, registered by writer-a (`FR5SZZWyr9dfQdpkxDTQamQXXoRKG6SPTPwK7NU21W1P`, 0.5 NEAR attached). Fee 0.0086964887749001 NEAR, paid by writer-a. | testnet | 0.5 NEAR attached (net of refunds stays as vault and registry storage); 0.0086964887749001 NEAR fee | 0 | owner |
| 2026-11-13 21:37 | Demo write W15. The agent paid work 5 with extra `payee` and `receiver_id` arguments naming outsider. The contract ignored them: 50000 to `s12`, 50000 to `s13`, 0 to outsider, line `agent-line-3` (`CUW8qssJVCJZuUgzzWHf9bdEyNCkFXpQjUKrJeWXeVPS`, no deposit). Read back after: `get_role` allowance 200000. Fee 0.0014110227723242 NEAR, paid from the agent key's gas allowance. | testnet | 0.0014110227723242 NEAR fee | 0 | owner |
| 2026-11-14 02:43 | Audit redeploy. Registry redeployed in place with the audit fix F1 (forward gas budget) on top of the review fixes, no init call (`BGBUJDTnnjpevSUZkdmfeVASxbKHn1FQDAZqj75kH5rU`, block 277597324, signer `reg.royltee-dev.testnet`, full-access key). Code hash now `5Taw8Syz7g1QxNa4Faw8mz7fnNgoXGj7MwX2VS3obRFo` (sha256 `423e37999e13cd1acd5f2a18ef396dcc21a9862481ce573d6271a2cea7e4636a`, 712958 bytes). Vault code hash unchanged (`9e9ef978...`). Fee 0.0051480829649394 NEAR, paid by the registry account, read from NEAR RPC `EXPERIMENTAL_tx_status` (sum of `tokens_burnt`). | testnet | 0.0051480829649394 NEAR fee | 0 | owner, 2026-11-14 |
| 2026-11-14 09:09 | Final contract fixes redeploy. Registry redeployed in place in ONE transaction: a deploy action, then a call of `assert_quiescent` (`52jp95BVJCk6a9bUGyYtYEQq99R56Xiz5zxJ1vCHBJrD`, block 277636549, 59.0 Tgas, signer `reg.royltee-dev.testnet`, full-access key). The call succeeded, so the registry was quiet; a busy registry would have rolled the deploy back. The read-only `check:quiescent` printed SNAPSHOT QUIET at block 277636451 before it. Code hash now `5YNk7mvmiq4SuSckR1gULXVvgcRdUjaioxmDE8niguAG` (sha256 `4378568f0459560130cdb56fa05152f9946116d3332d6c3f7f53e939005dc271`, 748115 bytes). Storage 731332 to 766489 bytes (about 0.35 NEAR staked more, paid from the registry balance, which read 8.309436090103712599999993 NEAR afterwards). Vault code hash unchanged. Fee 0.0058702528822306 NEAR, paid by the registry account, read from NEAR RPC `EXPERIMENTAL_tx_status` (sum of `tokens_burnt`). | testnet | 0.0058702528822306 NEAR fee | 0 | owner, 2026-11-14 |
| 2026-11-14 09:33 | Auction 17a. Root created `auction.royltee-dev.testnet` with 6 NEAR and one full-access key; the auction contract was written on branch `feat/tee-auction`, now merged into `main` (`6c1KYbcDcNvYqzLUMTyiPWWoouSLprQhViPrtoY4punJ`, block 277639017). Fee 0.0071149895375 NEAR, paid by root. | testnet | 6 NEAR moved to the auction account; 0.0071149895375 NEAR fee | 0 | owner |
| 2026-11-14 09:38 | Auction 17b. FastFS upload of the auction WASI module (1 Tgas call to `outlayer.testnet`, signed by the auction account). The call fails with MethodNotFound by design of FastFS, which reads the transaction data; FastFS then served the module with sha256 `c4a29e55ca61da2e069bdcfebd4d4e6690820620582ed2635d1dd6bc17c2594e` (`4c4xPqRuuhfcmj949aMzDB8VuvxGYFNiWK6B5cxMBrG1`, block 277639509). Fee 0.0010396110632104 NEAR, paid by the auction account. | testnet | 0 NEAR attached; 0.0010396110632104 NEAR fee | 0 | owner |
| 2026-11-14 09:41 | Auction 17c. Deploy and init of the auction contract in one transaction (code sha256 `48d32c71de83d94a4225502a726b80bd678ac3dfb1471952c93c4c16595a8d09`) (`EWWMu3chsa56hBybTpZ5hgiM6bazXRBekESv7KzpWYEt`, block 277639814). Fee 0.0026713613833586 NEAR, paid by the auction account. | testnet | 0 NEAR attached; 0.0026713613833586 NEAR fee | 0 | owner |
| 2026-11-14 09:45 | Auction 17d. The registrar granted a credential to the auction account (0.02 NEAR attached) (`2zysmt9BZCbFs3bcVQSdwQPouPNgtt4W1DH7RHMw4eeG`, block 277640210). Fee 0.0007635946207045 NEAR, paid by the registrar. | testnet | 0.02 NEAR attached; 0.0007635946207045 NEAR fee | 0 | owner |
| 2026-11-14 | Auction 17e. The owner set the OutLayer secret access rule in the OutLayer dashboard (And of the WASM hash and a Predecessor whitelist naming the auction account). Verified on chain by a read call. No transaction id is recorded by this project. Storage deposit 0.00692 NEAR. | testnet | 0.00692 NEAR storage deposit | 0 | owner |
| 2026-11-14 10:06 | Auction 17f. Root published the TEE key through the auction (`publish_key`, OutLayer request 44227, 0.02 NEAR attached; OutLayer charged 0.0011308003 NEAR and refunded the rest) (`7TLqoWrnBbTupg3PLccuc2eXgbquXACtYZqGDNgV58fq`, block 277642341). Fee 0.0011346153182622 NEAR, paid by root. | testnet | 0.0011308003 NEAR OutLayer charge; 0.0011346153182622 NEAR fee | 0 | owner |
| 2026-11-14 10:08 | Auction 17g part 1. The outsider asked OutLayer directly to run the module naming the auction as contract (OutLayer request 44242); the keystore refused (Access denied by access condition), return value null. 0.02 NEAR attached, OutLayer charged 0.001 NEAR and refunded 0.019 NEAR (`EaHtbQ54aAyZUgakGc4EkrYNLPMafboWStyC4rfQ9BvM`, block 277642548). Fee 0.0007949056762307 NEAR, paid by the outsider. | testnet | 0.001 NEAR OutLayer charge; 0.0007949056762307 NEAR fee | 0 | owner |
| 2026-11-14 10:13 | Auction 17g part 2. The outsider named itself as contract (OutLayer request 44261); same refusal, return value null. 0.02 NEAR attached, OutLayer charged 0.001 NEAR and refunded 0.019 NEAR (`57tzs8NbCKhhC9guscGE1NbgqZWxNLXsTF4mZgyFXtqP`, block 277643025). Fee 0.0008006873985831 NEAR, paid by the outsider. | testnet | 0.001 NEAR OutLayer charge; 0.0008006873985831 NEAR fee | 0 | owner |
| 2026-11-14 10:15 | Auction 17h. The payer minted 200 mUSDC to writer-b (Simulated money) (`9kwufV5FfUjXbZVwN662kawt2Q9RDRz9Hb6WiYt4vZY`, block 277643285). Fee 0.0001931299633146 NEAR, paid by the payer. | testnet | 0 NEAR attached; 0.0001931299633146 NEAR fee | 0 | owner |
| 2026-11-14 10:16 | Auction 17h. The payer minted 200 mUSDC to buyer-pub (Simulated money) (`6KZx5zga2jSAtWV1PPfucAra6RfECaSmWzx1v2wBnTmj`, block 277643349). Fee 0.0001927344103026 NEAR, paid by the payer. | testnet | 0 NEAR attached; 0.0001927344103026 NEAR fee | 0 | owner |
| 2026-11-14 10:18 | Auction 17i. writer-a listed share `s10` in the auction with `nft_transfer_call` (reserve 20 mUSDC, max deposit 100 mUSDC, ends 2026-11-14 10:38:50 UTC, 20 minutes after sending instead of the 10 in the plan, so both bids fit; 1 yoctoNEAR) (`Ae6ZccUJNUoRZRSPesWyJGCnpSPZ8hs11Ch5WjFANHiR`, block 277643609). Fee 0.0010251694412606 NEAR, paid by writer-a. | testnet | 1 yoctoNEAR; 0.0010251694412606 NEAR fee | 0 | owner |
| 2026-11-14 10:27 | Auction 17j. writer-b sealed bid 1 (sealed 70 mUSDC, deposit 100 mUSDC, Simulated; 1 yoctoNEAR) (`GHCfH4PhhcNCG4mmDCZXKmy9WvDucEnhCmerYVSjNm1v`, block 277644542). Fee 0.0010739862590389 NEAR, paid by writer-b. | testnet | 1 yoctoNEAR; 0.0010739862590389 NEAR fee | 0 | owner |
| 2026-11-14 10:29 | Auction 17j. buyer-pub sealed bid 2 (sealed 90 mUSDC, deposit 100 mUSDC, Simulated; 1 yoctoNEAR) (`AK88NkPkXsik1xPbTNDNZB6VCCRi2rKhxeVbap9bHkSc`, block 277644667). Fee 0.0010784436585155 NEAR, paid by buyer-pub. | testnet | 1 yoctoNEAR; 0.0010784436585155 NEAR fee | 0 | owner |
| 2026-11-14 10:39 | Auction 17k. Root settled auction 0 through the real OutLayer TEE (300 Tgas, 0.02 NEAR attached, OutLayer request 44422; OutLayer charged 0.0010383018 NEAR and refunded 0.0189616982 NEAR). Sold to buyer-pub at the second price, 70 mUSDC; `s10` is now owned by buyer-pub (`CATqXVEauHtQ1mpFHQo7VxQdj9fCwnEQQUFGh9NK5esW`, block 277645673). Fee 0.0020885780079987 NEAR, paid by root. | testnet | 0.0010383018 NEAR OutLayer charge; 0.0020885780079987 NEAR fee | 0 | owner |
| 2026-11-14 10:46 | Auction 17m. Root ran `withdraw` for writer-a, writer-b and buyer-pub; the auction owes nothing after it (`2PMK4vf1dKsPNhZwtNkWL2xninc7ZNRN515HiwndQdVY`, block 277646389). Fee 0.0016606892995477 NEAR, paid by root. | testnet | 0 NEAR attached; 0.0016606892995477 NEAR fee | 0 | owner |
| 2026-11-14 10:54 | Step 16.1. writer-a registered work 6, `ZZRYL2600004` (0.05 NEAR attached). An earlier identical send expired (Transaction has expired) and never landed (`EMdw5gBJg8LAeuZ96n767iYiV6zyYhAgqmQ1Gf89W2sy`, block 277647259). Fee 0.0003588195886742 NEAR, paid by writer-a. | testnet | 0.05 NEAR attached; 0.0003588195886742 NEAR fee | 0 | owner |
| 2026-11-14 10:55 | Step 16.2. writer-a registered share `s14` (50 percent of work 6, approved; 0.5 NEAR attached, most refunded after vault storage) (`9aUzpdgBPrdJ3XaSn3YdKu19yjaBjKZj1S9wzowL9Nwt`, block 277647307). Fee 0.0087903663911723 NEAR, paid by writer-a. | testnet | 0.5 NEAR attached (net of refunds stays as vault and registry storage); 0.0087903663911723 NEAR fee | 0 | owner |
| 2026-11-14 10:56 | Step 16.3. writer-b registered share `s15` (the other 50 percent of work 6, approval pending; 0.5 NEAR attached) (`5xYY2sWQQrZL7XCiQ8CVKhw27pUWDRhnN5k3doqjsgjB`, block 277647393). Fee 0.0088750057064136 NEAR, paid by writer-b. | testnet | 0.5 NEAR attached (net of refunds stays as vault and registry storage); 0.0088750057064136 NEAR fee | 0 | owner |
| 2026-11-14 10:56 | Step 16.4. The payer paid work 6 one mUSDC (Simulated, line `line-4`): `s14` got 0.5 at once, 0.5 for `s15` was escrowed awaiting approval (1 yoctoNEAR) (`8y7g9RfonZSfhBHKvRVAiYytyK1zEeo2zp5kgJV4cokL`, block 277647447). Fee 0.001291858779242 NEAR, paid by the payer. | testnet | 1 yoctoNEAR; 0.001291858779242 NEAR fee | 0 | owner |
| 2026-11-14 10:57 | Step 16.5. writer-a approved `s15` (`approve_share`, 1 yoctoNEAR): the held 0.5 mUSDC was released to the `s15` vault (`5dgncSm1CyDuiEKRzeUpZJTWrBnHzNuuEP6k51RNseGa`, block 277647543). Fee 0.0008330314951524 NEAR, paid by writer-a. | testnet | 1 yoctoNEAR; 0.0008330314951524 NEAR fee | 0 | owner |

All on-chain transaction ids (steps 1 to 13, step 15, demo writes W1 to W15, the audit redeploy, the final redeploy, the auction demo 17a to 17m and step 16) are in `deployments/testnet.json`. Combined gas fees for steps 1 to 13: 0.1667600075050178 NEAR, of which 0.0213449686125 NEAR was paid by the faucet. The four OutLayer spike transactions are in this log and are not in that record. Real money: 0, aside from the UNVERIFIED inference estimates above.

Step 15 and demo writes W1 to W15 (2026-11-13, 18:54 to 21:37 UTC): 16 transactions. Each fee above is the sum of `tokens_burnt` over the transaction outcome and every receipt outcome, read with the RPC method `EXPERIMENTAL_tx_status`. Step 15 fee 0.0051482685683912 NEAR. Demo writes W1 to W15: 0.059206177475603 NEAR in total (writer-a 0.0533113548569173, pay-agent 0.0030603638566265, payer 0.0017066749198462, buyer-pub 0.0009080902076662, writer-b 0.0002196936345468). Together 0.0643544460439942 NEAR. W2 and W11 are refusals by design and still paid gas.

Audit redeploy (2026-11-14 02:43 UTC): 0.0051480829649394 NEAR. Step 15, the demo writes and the audit redeploy are 17 transactions and 0.0695025290089336 NEAR in gas fees. OutLayer spike (2026-11-13, 20:23 to 20:31 UTC): 4 transactions. Combined gas fees 0.0098614355033805 NEAR (root 0.0082017705036522, probe 0.0016596649997283). OutLayer charged 0.0010626 NEAR and refunded 0.0089374 NEAR. The four transaction ids are in this log and are not in `deployments/testnet.json`. Together, 21 transactions since the first record, 0.0793639645123141 NEAR in gas fees, plus the 0.0010626 NEAR OutLayer charge.

Native NEAR balances at the time of the first record (RPC `view_account`, 2026-11-13, about 07:55 UTC, block heights 277481803 to 277482071). The current balances are in the table after this one.

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

Auction demo and step 16 (2026-11-14, 09:33 to 10:57 UTC): 19 transactions, 0.0417815779984826 NEAR in fees (royltee-dev.testnet 0.0119988721633086, writer-a 0.0110073869162595, writer-b 0.0099489919654525, the auction account 0.003710972446569, the payer 0.0016777231528592, the outsider 0.0015955930748138, buyer-pub 0.0010784436585155, the registrar 0.0007635946207045). The fees are the sum of `tokens_burnt` over the transaction and every receipt, read with `EXPERIMENTAL_tx_status`. OutLayer charged 0.0041691021 NEAR in total (17f 0.0011308003, 17g part 1 0.001, 17g part 2 0.001, 17k 0.0010383018) out of the 0.02 NEAR attached each time; the rest was refunded. The root account also moved 6 NEAR into the auction account (17a). Step 17e had no transaction id recorded here. All amounts of mock USDC in this section are Simulated. Real money: 0.

Native NEAR balances after the audit redeploy (RPC `view_account`, finality final, read on 2026-11-14 at 02:50 UTC, block heights 277597550 to 277597890). The root account is 2.5092643705036522 NEAR lower than in the first table because of the OutLayer probe spike (2.5 NEAR to create `probe.royltee-dev.testnet`, fees and the OutLayer charge), which is logged in the OutLayer spike rows above.

| Account | Balance (NEAR) |
|---|---|
| royltee-dev.testnet | 6.4337710766838478 |
| royltee-fund-1.testnet | 0.0999553634875 |
| royltee-fund-2.testnet | 0.0999553634875 |
| reg.royltee-dev.testnet | 8.315306342985943199999993 |
| usdc.royltee-dev.testnet | 2.520986807902146000000021 |
| registrar.royltee-dev.testnet | 0.986350948137074199999999 |
| writer-a.royltee-dev.testnet | 2.471380728471753499999999 |
| writer-b.royltee-dev.testnet | 1.372028146699916799999999 |
| buyer-pub.royltee-dev.testnet | 1.390207925673098399999997 |
| pay-agent.buyer-pub.royltee-dev.testnet | 0.4949965463330013 |
| payer.royltee-dev.testnet | 0.991778085500914199999995 |
| outsider.royltee-dev.testnet | 0.799100726232201 |
| s1.reg.royltee-dev.testnet | 0.099999999999999999999999 |
| s2.reg.royltee-dev.testnet | 0.099999999999999999999998 |
| s3.reg.royltee-dev.testnet | 0.1 |
| s4.reg.royltee-dev.testnet | 0.1 |
| s5.reg.royltee-dev.testnet | 0.1 |
| s6.reg.royltee-dev.testnet | 0.1 |
| s7.reg.royltee-dev.testnet | 0.1 |
| s8.reg.royltee-dev.testnet | 0.1 |
| s9.reg.royltee-dev.testnet | 0.1 |
| s10.reg.royltee-dev.testnet | 0.1 |
| s11.reg.royltee-dev.testnet | 0.1 |
| s12.reg.royltee-dev.testnet | 0.1 |
| s13.reg.royltee-dev.testnet | 0.1 |

Native NEAR balances after step 16 and the auction demo (RPC `view_account`, finality final, read on 2026-11-14 at about 11:05 UTC, block heights 277648160 to 277648315). The root account is 6 NEAR lower mainly because it funded the auction account (17a).

| Account | Balance (NEAR) |
|---|---|
| royltee-dev.testnet | 0.4196031024205392 |
| royltee-fund-1.testnet | 0.0999553634875 |
| royltee-fund-2.testnet | 0.0999553634875 |
| reg.royltee-dev.testnet | 8.346596090103712599999994 |
| usdc.royltee-dev.testnet | 2.524736807902146000000029 |
| registrar.royltee-dev.testnet | 0.982087353516369699999999 |
| writer-a.royltee-dev.testnet | 2.339673341555493999999997 |
| writer-b.royltee-dev.testnet | 1.245369154734464299999998 |
| buyer-pub.royltee-dev.testnet | 1.389129482014582899999996 |
| pay-agent.buyer-pub.royltee-dev.testnet | 0.4949965463330013 |
| payer.royltee-dev.testnet | 0.990100362348054999999994 |
| outsider.royltee-dev.testnet | 0.7955051331573872 |
| s1.reg.royltee-dev.testnet | 0.099999999999999999999999 |
| s2.reg.royltee-dev.testnet | 0.099999999999999999999998 |
| s3.reg.royltee-dev.testnet | 0.1 |
| s4.reg.royltee-dev.testnet | 0.1 |
| s5.reg.royltee-dev.testnet | 0.1 |
| s6.reg.royltee-dev.testnet | 0.1 |
| s7.reg.royltee-dev.testnet | 0.1 |
| s8.reg.royltee-dev.testnet | 0.1 |
| s9.reg.royltee-dev.testnet | 0.1 |
| s10.reg.royltee-dev.testnet | 0.1 |
| s11.reg.royltee-dev.testnet | 0.1 |
| s12.reg.royltee-dev.testnet | 0.1 |
| s13.reg.royltee-dev.testnet | 0.1 |
| s14.reg.royltee-dev.testnet | 0.1 |
| s15.reg.royltee-dev.testnet | 0.1 |
| auction.royltee-dev.testnet | 5.996289027553430999999996 |
