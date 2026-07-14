# Artifacts

Durable outputs referenced by `docs/tracker.html` and biographies.

| Artifact | Location | Biography |
|----------|----------|-----------|
| Herdr API schema | `crates/herdr-types/schemas/herdr-api.schema.json` | [biographies/schema.md](../biographies/schema.md) |
| Living tracker | `docs/tracker.html` | [biographies/tracker.md](../biographies/tracker.md) |
| Checkpoint ledger | `docs/checkpoint-ledger.jsonl` | [biographies/checkpoint-ledger.md](../biographies/checkpoint-ledger.md) |
| Ledger append-only validator | `scripts/check-ledger-append-only.py` | [biographies/ledger-validator.md](../biographies/ledger-validator.md) |
| Ledger CI | `.github/workflows/ledger.yml` | [biographies/ledger-ci.md](../biographies/ledger-ci.md) |
| CODEOWNERS | `.github/CODEOWNERS` | [biographies/codeowners.md](../biographies/codeowners.md) |
| Repository protection | `docs/REPO_PROTECTION.md` | [biographies/repo-protection.md](../biographies/repo-protection.md) |
| Checkpoint status schema | `docs/schemas/acex-checkpoint-status.schema.json` | [biographies/checkpoint-status-schema.md](../biographies/checkpoint-status-schema.md) |
| Architecture | `docs/ARCHITECTURE.md` | self |
| Extension guide | `docs/EXTENDING.md` | self |
| Verify gates | `docs/VERIFY.md` | self |
| Agent skill | `skills/acex-dev/SKILL.md` | [biographies/skill.md](../biographies/skill.md) |
| Discovery crate | `crates/acex-discover/` | [biographies/acex-discover.md](../biographies/acex-discover.md) |
| Example package manifest | `packages/example-board-hints/acex-package.toml` | Drop-in discovery fixture (reported by `--status`) |

Current continuation oracle: `cargo run -p acex -- --checkpoint-status` emits schema version 1 JSON covering git state, tracker capsule, ledger validity, Herdr side-effect mode, and discovery diagnostics.

Refresh schema:

```bash
herdr api schema --json > crates/herdr-types/schemas/herdr-api.schema.json
```

Then note protocol version + date in the tracker Artifacts / Changelog sections.
