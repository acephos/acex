# Biography: checkpoint-ledger.jsonl

| | |
|--|--|
| **Path** | `docs/checkpoint-ledger.jsonl` |
| **Purpose** | Append-only JSONL checkpoint ledger for durable stateless handoff facts. |
| **Origin** | Added after the stateless continuation checkpoint to make process history robust beyond mutable tracker sections. |
| **Status** | Active JSONL ledger; new meaningful scope/status/process changes append exactly one schema-versioned JSON object line here. Entries are hash-chained (`prev_hash`/`hash`). Append-only is enforced by process, `scripts/check-ledger-append-only.py`, CI prefix checks, CODEOWNERS, and repository branch protection — not by file format alone. |
| **How to change** | Append only. New entries use `schema_version=1`; corrections are new `kind=correction` entries with `corrects_hash`; do not rewrite, reorder, or delete historical lines. Keep tracker capsule and ledger in sync. |
