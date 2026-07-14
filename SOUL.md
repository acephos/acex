# SOUL.md — What acex *is*

> Read this before writing code, opening a PR, or teaching an agent about acex.
> If a change violates SOUL, it is wrong — even if it ships faster.

---

## One sentence

**acex** is a high-leverage control plane for *parallel agent work*: it does not own terminals, editors, or filesystems — it *orchestrates, observes, and extends* the systems that do.

---

## Identity

| Role | Owner | acex does |
|------|--------|-----------|
| PTYs / panes / mux | **Herdr** | Control via NDJSON socket; never re-mux |
| Buffers / editing | **Zed** (or any editor bridge) | Spawn / open / diff; never embed |
| Files / truth | **Filesystem + git** | Paths, worktrees, cwd; never invent state |
| Agents | **Herdr agents + your tools** | Start / send / wait / focus / peek |
| Control plane UI | **acex (ratatui)** | Board, palette, live model, handoffs |
| Memory of the project | **`docs/tracker.html` + these docs** | Living plan; agent-readable truth |

acex is the **cockpit**, not the engine.

---

## Four pillars (non-negotiable)

### 1. Extensibility

- Prefer **traits, adapters, and registries** over hard-coded product branches.
- New backends (another mux, another editor, another transport) land as crates/plugins behind stable interfaces — not `if product == …` sprawl in UI.
- Command palette, actions, and status sources are **data-driven** where possible.
- Agents (human or silicon) must be able to discover capabilities: schema, action lists, config keys, socket methods.

### 2. Platform-agnostic core

- **Domain + model + protocol types** compile and test everywhere.
- Platform IO (Unix socket, Windows named pipe, SSH remote) lives behind a **Transport** trait.
- Unix-first *delivery* is allowed; Unix-only *architecture* is not.
- CI and `cargo check --workspace` must not assume a single OS in library crates.

### 3. Performance under high-abstraction agentic load

- Optimize for **many parallel agents**, not pretty single-pane demos.
- Hot path: event reduce → model patch → redraw. No full resnapshot on every blip.
- Unary RPC is explicit and bounded; subscriptions are long-lived and backpressured.
- Peek buffers are **caches**, not authority. Herdr (and FS) win conflicts.
- Avoid work on the render thread; keep UI non-blocking (wait badges, not freezes).

### 4. Observability you can trust with your eyes

- Parallel workflows are only productive if status is **honest, local, and scannable**.
- Prefer: state badges, age indicators, last error, wait targets, workspace filters.
- Prefer: one board that answers “what is happening?” over nested menus.
- Logs and traces serve agents and humans; name events after **intent**, not implementation.
- Never fake live PTY frames in the control plane — that lies about ownership. Attach to Herdr for truth.

---

## Agentic center

acex is designed **for** agents as first-class operators and **by** agentic workflows as the default way the product improves.

1. **Agent-readable project memory** — `docs/tracker.html` is the sole living tracker. Code changes that alter intent update the tracker in the same change.
2. **Self-aware** — the binary and docs know phase, ship gate, cut list, and non-goals. Agents should not re-derive product scope from tribal knowledge.
3. **Self-improving** — when a friction appears (schema drift, reconnect race, bad UX), encode the lesson in tracker comments + tests + SOUL/GOAL — not only in chat.
4. **Extensible interaction** — palette actions, CLI flags, and socket methods form a coherent surface; agents should script acex the same way humans click it.
5. **Interpretability** — every parallel agent row answers: *who, where (cwd/worktree), state, last signal, how to attach/focus/send*.

---

## Aesthetic & UX soul

- Control plane, not IDE cosplay.
- Dense information, sparse chrome.
- Handoff over reimplementation (`herdr attach`, `zed`).
- Empty and error states are features: offline CTA, actionable errors, no toast spam.
- Yellow/selection language in docs UI is optional flair; the **product** stays terminal-native.

---

## Hard refusals

Do **not**:

- Reimplement a terminal emulator or pane mux inside acex
- Treat peek/ANSI as source of truth
- Call `server.stop` on acex quit (Herdr outlives the cockpit)
- Ship Windows-looking support that is actually broken
- Grow multi-session chrome before single-session excellence
- Hide breaking protocol assumptions without schema/version checks

---

## How to disagree with SOUL

SOUL can change — but only by **explicit edit** to this file plus a tracker entry under Decisions / Comments. Silent drift is how cockpits become junk drawers.

---

*acex · herdr control plane · the soul is the boundary*
