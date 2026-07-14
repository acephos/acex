# Biography: PR verification helper

| | |
|--|--|
| **Path** | `scripts/verify-pr.sh` |
| **Purpose** | Single script for PR hygiene gates: fmt, clippy, workspace tests, status, checkpoint-status, smoke, and ledger validation. |
| **Origin** | Added with the OMP PR loop so local operator automation and GitHub CI run the same gate list. |
| **Status** | Active local/CI helper. Local PR loop runs live smoke by default; hosted CI passes `--offline-smoke` because GitHub runners do not have a Herdr daemon. |
| **How to change** | Mirror `docs/VERIFY.md` and keep live local smoke stronger than offline CI smoke. Do not weaken ledger validation for local PR creation. |
