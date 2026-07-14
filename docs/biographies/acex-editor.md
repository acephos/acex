# Biography: acex-editor

| | |
|--|--|
| **Path** | `crates/acex-editor/` |
| **Purpose** | Sole editor-spawn crate: `EditorBridge` trait + Zed CLI adapter. |
| **Origin** | Scaffold for F17/F18. |
| **Status** | Zed open/diff/new-window paths usable; PATH errors typed. |
| **How to change** | New backends implement `EditorBridge`; wire in bin compose. |
