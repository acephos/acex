# Biography: checkpoint status schema

| | |
|--|--|
| **Path** | `docs/schemas/acex-checkpoint-status.schema.json` |
| **Purpose** | Machine-readable contract for `cargo run -p acex -- --checkpoint-status` JSON. |
| **Origin** | Added during stateless continuation hardening so fresh agents and scripts can rely on a versioned continuation oracle. |
| **Status** | Schema version 1; update with golden tests when checkpoint status output changes. |
| **How to change** | Change schema and `crates/acex/src/checkpoint_status.rs` tests together, then append a checkpoint ledger entry. |
