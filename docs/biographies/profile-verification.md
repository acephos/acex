# Profile verification artifacts

Purpose: reproducible onboarding proof and clearly bounded performance/platform evidence.
Origin: 2026-10-01 profile improvement pass.
Status: `docs/artifacts/profile-verification-2026-10-01.md` records Linux live checks, an actual TUI asciinema recording, source metadata corrections, and offline timing results. `crates/acex-ui/examples/measure.rs` calls shipped reducer/render functions; the offline text frame contains synthetic rows. CI adds Windows/macOS offline contracts and Rust 1.88 MSRV checking; it is not live Windows certification.
Change: rerun the documented commands and append new dated records. Keep fixtures explicitly labeled and benchmark environment/units/sample counts attached to results. Preserve ledger history and update tracker capsule to the actual tail.
