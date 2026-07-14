# GOAL.md — What acex is *for*

> North star, ship gates, and success metrics.
> Pair with `SOUL.md` (identity) and `docs/tracker.html` (living status).

---

## North star

Make **parallel agent development** feel like flying a well-instrumented cockpit:

- You always know what every agent is doing.
- You can start, steer, wait, and hand off without leaving flow.
- You never fight the control plane for ownership of the terminal or the editor.
- The system stays fast and honest as agent count grows.

**Product shape:** Unix-first ratatui app over Herdr’s NDJSON socket; Zed as external editor; filesystem as truth.

---

## Success looks like

### For a human operator

1. Launch `acex` in a repo → auto-connect (spawn Herdr if needed).
2. See all agents with live states (idle / working / blocked / done / unknown).
3. Peek recent output without attach; send text; wait for done/blocked without freezing UI.
4. Open paths in Zed; attach to Herdr for real multi-pane interaction; return to acex still live.
5. Survive server kill → reconnect + resnapshot without restarting acex.

### For an agent operator (silicon)

1. Discover actions and config without reading source (`--help`, schema, tracker, AGENTS.md).
2. Script the same intents as the palette (start / send / wait / focus / open / attach).
3. Rely on stable state names and error codes — not scraped TUI pixels.
4. Extend via clear crate boundaries (`herdr-client`, `acex-model`, editor bridge trait).

### For the project itself (self-aware / self-improving)

1. `docs/tracker.html` is the **sole** project tracker — features, phases, comments, artifacts, decisions.
2. Every meaningful change updates tracker status (and SOUL/GOAL if philosophy shifts).
3. Control-plane implementation, schema, and demo script stay aligned with Herdr protocol reality.
4. Lessons from failures become tests + tracker comments, not lore.

---

## Ship gates

| Gate | Name | Must include |
|------|------|----------------|
| **G0** | Skeleton live | Connect, snapshot, subscribe, reconnect, empty/error UX |
| **G1** | **MVP ship** | Board + peek + start/send/wait + palette + Zed open + attach + reconnect + basic worktree visibility |
| **G1.5** | Recommended | Worktree CRUD polish, layout presets (warned), diff, notify, pane run, workspace strip |
| **G2** | Post-MVP | Additional platform polish, optional observe, multi-session, richer ANSI — not required for “usable” |

**Definition of “usable”:** G1 demo script steps 1–12 pass on a clean Unix-ish environment with `herdr` + `zed` on PATH.

---

## Non-goals (do not “almost” ship)

- Embedded terminal / libghostty
- Live frame streaming as primary UX
- Forking or RPC-controlling Zed buffers
- Cloud control plane
- Full git graph / file tree IDE
- Plugin marketplace UI
- Multi-session chrome before G1.5 solid

(See SOUL hard refusals + tracker cut list.)

---

## Pillar-aligned outcomes

| Pillar | Goal expression |
|--------|-----------------|
| **Extensibility** | New editor or transport = new adapter crate; palette grows by registration |
| **Platform-agnostic** | Library crates clean on all targets; OS IO isolated |
| **Performant agentic workflows** | Board stays responsive with many agents; reduce ≠ resnapshot |
| **Observable parallel work** | One glance board: state, filter counts, wait badges, last error |

---

## Metrics (lightweight — track qualitatively until automated)

- Time-to-first-board after cold start (including auto-spawn)
- Correctness of state after external Herdr changes (no polling lag surprises)
- Reconnect success without process restart
- Operator questions answered without attach (“is it blocked?”, “what did it last print?”)
- Agent (human or AI) can complete demo script from tracker alone

---

## Near-term objective (active)

**G1 usable core → polish to MVP ship:**

1. ~~Phase 0 / G0~~ · ~~Phase 1 core control plane~~ (board, filters, palette, focus/peek/send/start/wait/zed/attach/worktrees)
2. **Current:** G1 demo path is honest with live smoke/status checks; start presets, wait timeout UX, and attach target variants are tightened
3. **Next:** recommended G1.5 (worktree CRUD, pane run, layouts, diff, notify polish, paths chrome, workspace focus)
4. Phase 2 post-MVP

Phase 1 is usable today; the active work is polish and evidence, not first bring-up.

---

## How goals change

Edit this file deliberately. Log the change in `docs/tracker.html` → Decisions / Changelog. Do not silently redefine “MVP” in a PR description only.

---

*acex · usable cockpit first · clever second*
