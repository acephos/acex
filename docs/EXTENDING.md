# Extending acex

> Drop-in agents: follow these steps exactly. Do not invent a parallel action system.

## Biography

| | |
|--|--|
| **Purpose** | Copy-paste contract for adding capabilities without violating ownership. |
| **Origin** | Codified after Phase 1/G1 core actions and `acex-discover` drop-in metadata shipped. |
| **Status** | Canonical extension how-to for the current 8-crate workspace. |
| **Change** | Update when a new seam is introduced; link from AGENTS.md. |

---

## Before you code

1. Read [SOUL.md](../SOUL.md) (hard refusals).  
2. Read [GOAL.md](../GOAL.md) (ship gate).  
3. Open [tracker.html](./tracker.html) — pick feature ID or add one.  
4. Skim [ARCHITECTURE.md](./ARCHITECTURE.md) ownership table.  
5. Skim [PHILOSOPHY_PI.md](./PHILOSOPHY_PI.md) — Pi-like discovery vs what we refuse to port.  

---

## Recipe 0 — Drop-in package (manifest only, Pi-like discovery)

**No recompile** for *metadata* discovery. Runtime behavior still needs Recipe A if you introduce a new Intent.

1. Create `.acex/packages/<id>/` or `packages/<id>/`.  
2. Add `acex-package.toml`:

```toml
name = "my-pack"
description = "What this package is for (loaded into --status)."
version = "0.1.0"

[[actions]]
id = "focus"
label = "Focus selected"
intent = "FocusSelected"   # known compile-time Intent; manifests do not create new runtime actions by themselves
```

3. Optionally add `skills/<name>/SKILL.md` (Agent Skills frontmatter).  
4. Verify:

```bash
cargo run -p acex -- --status
# emits acex_status, packages, skills, diagnostics, and seams
```

`--status` lists package summaries and skill summaries. Package manifests may declare skill paths for metadata/detail, while shipped skill summary scanning enumerates repo skill roots such as `skills/*/SKILL.md`.

5. Progressive detail: agents `read` the package README / skill body on demand — summaries only at scan time (`acex_discover::scan`).

---

## Recipe A — New operator action (most common)

Current shipped references: palette actions cover focus, peek, send, start, wait done/blocked, Zed open/new/add-window, attach/session attach, worktrees, resnapshot, refresh agents, and notify. The `Intent` surface also has Zed diff support for worker/editor integration.


**Goal:** palette + key binding + Herdr (or side-effect) behavior.

### 1. Intent (`acex-model`)

Add a variant to `Intent` in `crates/acex-model/src/intent.rs`:

```rust
MyAction { /* owned data only — no borrows */ },
```

Keep variants `Clone` + self-contained for `mpsc` send.

### 2. Palette (`acex-ui`)

In `crates/acex-ui/src/palette.rs`:

1. Add `PaletteAction::MyAction`
2. Implement `label()` and `keywords()`
3. Append to `PaletteAction::all()`
4. Wire in `apply_palette_action` in `lib.rs` → `app.send_intent(Intent::MyAction { … })`

Optional: board key in `handle_key` (`Mode::Board`).

### 3. Worker (`acex` bin)

In `crates/acex/src/worker.rs` `handle_intent`:

```rust
Intent::MyAction { … } => {
    let mut client = connect_with_optional_spawn(target, spawn).await?;
    // call herdr-client op or editor/process
    let mut s = store.lock().unwrap_or_else(|e| e.into_inner());
    s.set_toast("…");
}
```

If you need a new Herdr method, add it on `HerdrClient` in `crates/herdr-client/src/ops.rs` first.
Current unary Herdr surface: `ping` and `session.snapshot` in `HerdrClient`; `ops.rs` wraps `agent.list`, `agent.get`, `agent.focus`, `agent.send`, `agent.read`, `pane.read`, `agent.start`, `worktree.list`, and `notification.show`.


### 4. Tracker + ledger

In `docs/tracker.html`:

1. Set the feature status.
2. Prepend a **Comment**.
3. Append a **Changelog** line.
4. Append exactly one JSONL entry to `docs/checkpoint-ledger.jsonl` for the meaningful change.
5. Run `python scripts/check-ledger-append-only.py <base-ref>`.

### 5. Verify

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p acex -- --status
cargo run -p acex -- --checkpoint-status
cargo run -p acex -- --smoke
```
For discovery-only changes, also run `cargo run -p acex -- --status`; for continuation-affecting changes, run `cargo run -p acex -- --checkpoint-status` and update the schema/golden contract when output changes.


**Invariants:** UI does not open sockets; model does not spawn processes; worker does not draw.

---

## Recipe B — New editor backend

1. Implement `EditorBridge` in a new module/crate or alongside `ZedBridge` in `acex-editor`.  
2. Select implementation in `acex` compose (config flag / env `ACEX_EDITOR`).  
3. Do **not** call `Command::new("zed")` from `acex-ui` or `acex-model`.  
4. Document in tracker + this file.

---

## Recipe C — New transport

1. Implement `Transport` in `herdr-client` (`connect` / `disconnect` / `call_ndjson`).  
2. Prefer `PlatformTransport` cfg split or a new type used by `NdjsonStream` / `HerdrClient`.  
3. Add resolve rules in `resolve.rs` if path semantics change.  
4. Unit-test with `MockTransport` for unary logic; platform tests behind `#[cfg]` or `HERDR_E2E=1`.

---

## Recipe D — Protocol field / schema drift

Current observed schema/protocol: Herdr protocol 16, version 0.7.2-preview (2026-07-14 refresh).

1. Refresh schema:  
   `herdr api schema --json > crates/herdr-types/schemas/herdr-api.schema.json`  
2. Extend `herdr-types` with `#[serde(default)]` / flatten maps — **never** fail whole snapshot on unknown fields.  
3. Note protocol version in tracker Artifacts.  
4. Add a unit test that deserializes a fixture with the new field.

---

## Anti-patterns (refuse)

- Drawing outside `acex-ui`
- Opening Herdr sockets outside `herdr-client`
- Treating peek text as authority
- Calling `herdr server stop` on acex quit
- Adding cloud / plugin marketplace / embedded terminal “for later”

---

## Related

- [ARCHITECTURE.md](./ARCHITECTURE.md)  
- [VERIFY.md](./VERIFY.md)  
- [biographies/INDEX.md](./biographies/INDEX.md)  
