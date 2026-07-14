# Biography: acex-ui

| | |
|--|--|
| **Path** | `crates/acex-ui/` |
| **Purpose** | Only drawing crate — ratatui board, detail, palette, focus/input modals. |
| **Origin** | Phase 0 shell; Phase 1 board + palette. |
| **Status** | Usable board/palette/focus/peek/send/start/wait/Zed/attach/worktrees UI; sends `Intent` via channel; never opens Herdr sockets or spawns processes. |
| **How to change** | Palette registry in `palette.rs`; keys in `lib.rs`; no process spawn here. |
