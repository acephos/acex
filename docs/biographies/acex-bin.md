# Biography: acex binary

| | |
|--|--|
| **Path** | `crates/acex/` (`main.rs`, `worker.rs`) |
| **Purpose** | Compose runtime: bootstrap, live subscribe loop, intent worker, CLI. |
| **Origin** | Phase 0 binary shell → F03/F04 live → Phase 1 worker. |
| **Status** | CLI/TUI control plane usable: board/palette/focus/peek/send/start/wait/Zed/attach/worktrees plus `--offline`, `--status`, `--checkpoint-status`, `--smoke`, `--smoke-reconnect`; `--checkpoint-status` is no-spawn pure JSON continuation state. Never `server.stop` on quit. |
| **How to change** | Keep I/O here and in client; thin glue only. |
