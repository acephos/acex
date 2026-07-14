# Biography: herdr-types

| | |
|--|--|
| **Path** | `crates/herdr-types/` |
| **Purpose** | Platform-agnostic Herdr protocol models; forward-compatible serde. |
| **Origin** | Phase 0 scaffold; enriched as snapshot/events understood. |
| **Status** | Pure library (no tokio). Schema snapshot in `schemas/`. |
| **How to change** | Prefer `#[serde(default)]` + flatten maps; add tests for fixtures; refresh schema on protocol bump. |
| **Key types** | `Request`, `Response`, `SessionSnapshot`, `AgentSummary`, `AgentState`, `Event` |
