# Biography: ledger append-only validator

| | |
|--|--|
| **Path** | `scripts/check-ledger-append-only.py` |
| **Purpose** | Validate JSONL checkpoint ledger entries, verify the SHA-256 hash chain, enforce append-only prefix preservation against a base ref, and require one ledger entry for meaningful tracker/docs/CI/PR-automation changes. |
| **Origin** | Added with the JSONL checkpoint ledger after clarifying that file format alone does not guarantee append-only behavior. Hardened for fail-closed base handling, strict schema-versioned new entries, and PR-shaped continuation artifacts. |
| **Status** | Active verification helper; run locally for ledger changes and through `.github/workflows/ledger.yml` in CI. CI must not use `--allow-missing-base`. |
| **How to change** | Keep dependency-free and deterministic. Extend meaningful-path/schema checks cautiously; do not weaken prefix preservation or tracker-ledger coupling. |
