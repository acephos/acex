# AGENTS.md — Operating manual for agents working on acex

You are collaborating on **acex**: a Herdr-centric agent-dev control plane.

## Drop-in orientation (do this first)

**Read order (mandatory):**

1. [SOUL.md](./SOUL.md) — identity + hard refusals  
2. [GOAL.md](./GOAL.md) — ship gates + near-term objective  
3. [docs/tracker.html](./docs/tracker.html) — **sole living tracker**  
4. This file  
5. Before coding a change: [docs/ARCHITECTURE.md](./docs/ARCHITECTURE.md)  
6. Pi-like philosophy map: [docs/PHILOSOPHY_PI.md](./docs/PHILOSOPHY_PI.md)  
7. Before adding a feature: [docs/EXTENDING.md](./docs/EXTENDING.md)  
8. Before claiming done: [docs/VERIFY.md](./docs/VERIFY.md)  

**Skill:** load [skills/acex-dev/SKILL.md](./skills/acex-dev/SKILL.md) when modifying this repo.

**Drop-in packages:** place `acex-package.toml` under `.acex/packages/<id>/` or `packages/<id>/`.  
List with `cargo run -p acex -- --status` (JSON: `packages`, `skills`).

**Lineage:** [docs/biographies/INDEX.md](./docs/biographies/INDEX.md) — every durable artifact’s purpose → origin → status → how to change.

Chat history is **not** project memory.

---

## Project memory (single source of planning truth)

| Artifact | Role |
|----------|------|
| `SOUL.md` | Identity, pillars, hard refusals |
| `GOAL.md` | North star, ship gates, metrics |
| `docs/tracker.html` | **Sole tracker** — features, phases, comments, artifacts, decisions |
| `AGENTS.md` | This file — how *you* work here |
| `README.md` | Human onboarding |
| `docs/ARCHITECTURE.md` | Crate ownership + data flow |
| `docs/EXTENDING.md` | Copy-paste extension recipes |
| `docs/VERIFY.md` | Production hygiene commands |
| `docs/biographies/*` | Artifact biographies |

**Rule:** If you change scope, ship status, architecture intent, or cut/add features, update `docs/tracker.html` in the same change.

---

## Philosophy you must uphold

1. **Extensibility** — adapters/traits/registries; no product lock-in in the core model.  
2. **Platform-agnostic core** — IO behind `Transport`; Unix-first delivery OK.  
3. **High-performance agentic workflows** — event reduce hot path; non-blocking UI.  
4. **Observable parallel agents** — honest board; peeks are caches; Herdr/FS are authority.

acex is **self-aware** (docs know phase and gates) and **self-improving** (friction → tracker comment + test).

---

## Architecture map

```
acex (bin)
  ├── acex-ui        ratatui only
  ├── acex-model     store, reducers, filters, waiters, Intent
  ├── acex-config    session, socket, editor path, presets
  ├── acex-editor    editor bridge (Zed default)
  ├── herdr-client   NDJSON, reconnect, spawn server
  └── herdr-types    protocol models, forward-compat serde
```

**Rules:**

- Only `herdr-client` speaks NDJSON to Herdr.  
- Only `acex-editor` (or future editor adapters) spawns the editor.  
- Only `acex-ui` draws.  
- Never treat peek buffers as source of truth.  
- Never `server.stop` on acex quit.

Details: [docs/ARCHITECTURE.md](./docs/ARCHITECTURE.md).

---

## How to extend (one sentence)

Add `Intent` → palette action → worker arm → tracker note. Full recipe: [docs/EXTENDING.md](./docs/EXTENDING.md).

---

## Herdr integration cheat sheet

| Intent | Prefer |
|--------|--------|
| Bootstrap | `session.snapshot` |
| Live | `events.subscribe` |
| Health | `ping` / `herdr status` |
| Agents | `agent.list/get/read/send/focus/start` |
| Wait | status events + in-app wait badges |
| Peek/run | `agent.read` / `pane.read`, `herdr pane run` |
| Worktrees | `worktree.list/create/open/remove` |
| Layouts | `layout.apply` (new tab; **no** live PTY preserve) |
| Toast | `notification.show` |
| Terminals | `herdr` / `agent attach` |
| Editor | `zed`, `-n`, `-a`, `--diff` |

Sockets: config-dir `herdr/herdr.sock` · sessions · `HERDR_SOCKET_PATH` / `HERDR_SESSION`  
Schema: `herdr api schema --json` → `crates/herdr-types/schemas/`

---

## How to work (agent loop)

1. **Orient** — SOUL → GOAL → tracker → ARCHITECTURE.  
2. **Pick** — one feature ID (Fxx) or explicit tracker task; respect deps.  
3. **Implement** — smallest vertical slice; keep crates pure to their ownership.  
4. **Verify** — see [docs/VERIFY.md](./docs/VERIFY.md).  
5. **Record** — tracker status + comment + changelog.  
6. **Breadcrumbs** — philosophy shifts edit SOUL/GOAL explicitly.

### Tracker status vocabulary

| Status | Meaning |
|--------|---------|
| `todo` | Not started |
| `doing` | Active |
| `blocked` | Waiting on external/dep |
| `review` | Code in; needs validation |
| `done` | Acceptance met |
| `cut` | Explicitly deferred |

---

## Coding norms

- **Rust** workspace; prefer enums, typed errors, short lock scopes over clever macros.  
- Forward-compatible serde: ignore unknown fields.  
- Expected failures return `Result` — do not panic on missing Herdr / bad RPC.  
- Pure logic in `acex-model` / `herdr-types` must be unit-testable without I/O.  
- Tests call **shipped** functions (no parallel re-implementation).  
- Mutex poison: recover with `unwrap_or_else(|e| e.into_inner())` in production paths.

### Commands (production hygiene)

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p acex -- --smoke
```

Optional: `HERDR_E2E=1 cargo test -p herdr-client --test live_herdr -- --nocapture`

---

## What “done” means

Acceptance criteria live in the tracker matrix. Code without tracker update is incomplete when intent changes.  
Verify gates in [docs/VERIFY.md](./docs/VERIFY.md) must pass.

---

## Self-improvement protocol

| Event | Action |
|-------|--------|
| Schema drift | Refresh schema artifact; note protocol version in tracker |
| Reconnect bug | Test + comment under F04 |
| Bad abstraction | Propose trait boundary in tracker → implement |
| Scope temptation | Check SOUL hard refusals; Decisions entry first |

---

## Voice

Be direct. Prefer small changes of meaning. Name things after operator intent (`FocusAgent`, `PeekRecent`).

---

*You are co-pilot of the cockpit. Keep the board honest.*
