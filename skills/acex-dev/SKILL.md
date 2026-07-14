---
name: acex-dev
description: Develop and maintain the acex Herdr control-plane (SOUL/GOAL/tracker-first). Use for crates, discovery packages, palette/intents, Herdr client, and agent docs.
---

# acex-dev — drop-in agent skill

## Mandatory read order (before any code)

1. [SOUL.md](../../SOUL.md) — identity + hard refusals  
2. [GOAL.md](../../GOAL.md) — gates and near-term objective  
3. [docs/tracker.html](../../docs/tracker.html) — living status (sole tracker)  
4. [AGENTS.md](../../AGENTS.md) — operating manual  
5. [docs/PHILOSOPHY_PI.md](../../docs/PHILOSOPHY_PI.md) — Pi-like self-extension (Rust map)  
6. [docs/ARCHITECTURE.md](../../docs/ARCHITECTURE.md) — ownership  
7. [docs/EXTENDING.md](../../docs/EXTENDING.md) — Recipe 0 (manifest) + Recipe A (Intent)  
8. [docs/VERIFY.md](../../docs/VERIFY.md) — ship gates  

Lineage: [docs/biographies/INDEX.md](../../docs/biographies/INDEX.md).

**Continuation prompt:** “continue from the last checkpoint” is canonicalized in [AGENTS.md#stateless-continuation-new-session](../../AGENTS.md#stateless-continuation-new-session). Summary: tracker checkpoint capsule wins for live planning; JSONL ledger is historical evidence.

## Ownership (never violate)

- Only `herdr-client` talks NDJSON to Herdr.  
- Only `acex-ui` draws.  
- Only `acex-editor` spawns the editor.  
- `acex-model` / `acex-discover` are pure (no network UI).  
- Never `herdr server stop` on acex quit.  
- Peeks are caches; Herdr + FS are authority.  
- Tracker is the sole planning truth; update `docs/tracker.html` for scope/status changes.  
- **No jiti/TS runtime loaders** — manifests + compile-time registry only.

## Pi-like extension loop

1. **Discover** — `cargo run -p acex -- --checkpoint-status` for continuation state, or `cargo run -p acex -- --status` for live connection/discovery status.  
2. **Drop-in metadata** — `.acex/packages/<id>/acex-package.toml` or `packages/<id>/`.  
3. **Code hooks** — Intent → palette → worker (Recipe A in EXTENDING).  
4. **Record** — tracker feature status + comment + changelog + exactly one `docs/checkpoint-ledger.jsonl` entry.

## How to add a coded action (summary)

1. `Intent` variant in `acex-model/src/intent.rs`  
2. `PaletteAction` + wiring in `acex-ui`  
3. Arm in `acex/src/worker.rs` (+ `herdr-client/ops.rs` if new RPC)  
4. Optional: declare action in a package manifest for discovery  
5. Update `docs/tracker.html`  
6. Run the full verify set below before claiming done.

## Verify before claiming done

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p acex -- --status
cargo run -p acex -- --checkpoint-status
cargo run -p acex -- --smoke
```

Use `--status` for machine-readable live conn/packages/skills/diagnostics; use `--checkpoint-status` for no-spawn continuation state; use `--smoke` for the binary connect path. Dated proof belongs in the tracker capsule and checkpoint ledger, not this skill.

## Tracker discipline
Durable checkpoint facts go in `docs/checkpoint-ledger.jsonl` as append-only JSONL entries; corrections are new entries, not edits to history. File format alone does not guarantee append-only; run `python scripts/check-ledger-append-only.py <base-ref>` when ledger history changes.

Chat is ephemeral. Same change that alters product intent **must** update `docs/tracker.html`.
