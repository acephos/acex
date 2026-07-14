# Biography: acex binary

| | |
|--|--|
| **Path** | `crates/acex/` (`main.rs`, `worker.rs`) |
| **Purpose** | Compose runtime: bootstrap, live subscribe loop, intent worker, CLI. |
| **Origin** | Phase 0 binary shell → F03/F04 live → Phase 1 worker. |
| **Status** | Flags: `--offline`, `--smoke`, `--smoke-reconnect`. Never `server.stop` on quit. |
| **How to change** | Keep I/O here and in client; thin glue only. |
