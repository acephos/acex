# Biography: checkpoint-ledger.jsonl

| | |
|--|--|
| **Path** | `docs/checkpoint-ledger.jsonl` |
| **Purpose** | Append-only JSONL checkpoint ledger for durable stateless handoff facts. |
| **Origin** | Added after the stateless continuation checkpoint to make process history robust beyond mutable tracker sections. |
| **Status** | Active JSONL ledger; new meaningful scope/status/process changes append one JSON object line here. Entries are hash-chained (`prev_hash`/`hash`). Append-only is enforced by process, `scripts/check-ledger-append-only.py`, and CI, not by file format alone. |
| **How to change** | Append only. Corrections are new `kind=correction` entries; do not rewrite, reorder, or delete historical lines. Keep tracker and ledger in sync. |
