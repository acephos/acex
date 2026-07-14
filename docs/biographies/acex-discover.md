# Biography: acex-discover

| | |
|--|--|
| **Path** | `crates/acex-discover/` |
| **Purpose** | Pure FS discovery of drop-in packages + skills (Pi progressive disclosure). |
| **Origin** | Pi-alignment goal; earendil-works/pi skills/packages philosophy without TS loaders. |
| **Status** | Shipped as the 8th workspace crate; wired to `--status`, `--checkpoint-status`, and smoke JSON; reports packages, skills, and diagnostics for invalid manifests/frontmatter. |
| **How to change** | Keep pure (no tokio/UI); extend scan roots carefully; add fixture tests for valid discovery and diagnostics. |
