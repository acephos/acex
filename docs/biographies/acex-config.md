# Biography: acex-config

| | |
|--|--|
| **Path** | `crates/acex-config/` |
| **Purpose** | Runtime config: socket, session, spawn flags, editor bin, peek lines. |
| **Origin** | Phase 0 env defaults. |
| **Status** | Env-first runtime config for socket/session/spawn/editor/peek; supports offline/status/smoke paths. File presets deferred. |
| **How to change** | Add fields + env keys; document in VERIFY/AGENTS if operator-facing. |
