# RoylTEE

A permissioned registry of music rights shares on NEAR testnet: each share is an NFT that controls its own keyless payout vault. A share registered by anyone other than the work's registrant is paid only after the registrant approves it (on-chain split sheet). The registrar is a trusted role; THREAT_MODEL.md T12 lists what it can do. On testnet the registry's full-access key is still present (key deletion is parked). The full judge-facing README lands in block 7; until then see DESIGN.md and THREAT_MODEL.md. Testnet only, mock USDC only.
