# Biography: acex-discover

| | |
|--|--|
| **Path** | `crates/acex-discover/` |
| **Purpose** | Pure FS discovery of drop-in packages + skills (Pi progressive disclosure). |
| **Origin** | Pi-alignment goal; earendil-works/pi skills/packages philosophy without TS loaders. |
| **Status** | Shipped as the 8th workspace crate; wired to `--status` / smoke JSON; current repo reports packages=1 skills=1. |
| **How to change** | Keep pure (no tokio/UI); extend scan roots carefully; add fixture tests. |
