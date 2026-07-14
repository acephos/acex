# Biography: acex binary

| | |
|--|--|
| **Path** | `crates/acex/` (`main.rs`, `worker.rs`) |
| **Purpose** | Compose runtime: bootstrap, live subscribe loop, intent worker, CLI. |
| **Origin** | Phase 0 binary shell → F03/F04 live → Phase 1 worker. |
| **Status** | CLI/TUI control plane usable: board/palette/focus/peek/send/start/wait/Zed/attach/worktrees plus `--offline`, `--status`, `--smoke`, `--smoke-reconnect`; current status/smoke observed live and offline-honest with packages=1 skills=1. Never `server.stop` on quit. |
| **How to change** | Keep I/O here and in client; thin glue only. |
