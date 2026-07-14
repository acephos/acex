# Biography: acex-ui

| | |
|--|--|
| **Path** | `crates/acex-ui/` |
| **Purpose** | Only drawing crate — ratatui board, detail, palette, input modals. |
| **Origin** | Phase 0 shell; Phase 1 board + palette. |
| **Status** | Sends `Intent` via channel; never opens Herdr sockets. |
| **How to change** | Palette registry in `palette.rs`; keys in `lib.rs`; no process spawn here. |
