# Biography: herdr-client

| | |
|--|--|
| **Path** | `crates/herdr-client/` |
| **Purpose** | Only crate that speaks NDJSON to Herdr — transport, unary RPC, subscribe, spawn. |
| **Origin** | Phase 0; Windows named-pipe discovery; F03 subscribe; F04 resync. |
| **Status** | Production path for connect/ping/snapshot/ops; mock transport for tests. |
| **How to change** | New methods in `ops.rs`; transports implement `Transport`; keep unary connect-per-call. |
| **Modules** | `transport`, `stream`, `subscribe`, `resolve`, `ndjson`, `ops` |
