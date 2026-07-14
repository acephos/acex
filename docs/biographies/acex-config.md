# Biography: acex-config

| | |
|--|--|
| **Path** | `crates/acex-config/` |
| **Purpose** | Runtime config: socket, session, spawn flags, editor bin, peek lines, start presets. |
| **Origin** | Phase 0 env defaults; F31 added file-backed start presets. |
| **Status** | Env-first runtime config plus optional `config.toml` start presets; supports offline/status/smoke/checkpoint paths. |
| **How to change** | Add fields + env keys; document operator-facing sources in README/AGENTS/tracker. |
