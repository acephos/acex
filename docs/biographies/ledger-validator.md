# Biography: ledger append-only validator

| | |
|--|--|
| **Path** | `scripts/check-ledger-append-only.py` |
| **Purpose** | Validate JSONL checkpoint ledger entries, verify the SHA-256 hash chain, and enforce append-only prefix preservation against a base ref. |
| **Origin** | Added with the JSONL checkpoint ledger after clarifying that file format alone does not guarantee append-only behavior. |
| **Status** | Active verification helper; run locally for ledger changes and through `.github/workflows/ledger.yml` in CI. |
| **How to change** | Keep dependency-free and deterministic. Extend schema checks cautiously; do not weaken prefix preservation. |
