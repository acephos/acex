# acex

**Agent Control-plane EXperience** — a Herdr-centric control plane for parallel agent workflows.

- **UI:** ratatui  
- **Runtime:** [Herdr](https://herdr.dev) owns PTYs/panes (NDJSON socket)  
- **Editor:** [Zed](https://zed.dev) via CLI (external; never embedded)  
- **Truth:** filesystem + Herdr session state — peeks are caches only  

```
acex  ──socket──▶  Herdr   ──PTY──▶  agents
  │
  └── spawn ──▶  Zed
```

## Start here (humans & agents)

| Doc | Purpose |
|-----|---------|
| [SOUL.md](./SOUL.md) | Identity, four pillars, hard refusals |
| [GOAL.md](./GOAL.md) | North star, ship gates, metrics |
| [AGENTS.md](./AGENTS.md) | **Agent entry** — how to work in this repo |
| [docs/tracker.html](./docs/tracker.html) | **Sole living project tracker** |
| [docs/checkpoint-ledger.jsonl](./docs/checkpoint-ledger.jsonl) | Append-only JSONL checkpoint/audit ledger |
| [docs/ARCHITECTURE.md](./docs/ARCHITECTURE.md) | Crate ownership + data flow |
| [docs/EXTENDING.md](./docs/EXTENDING.md) | How to add actions / transports / editors |
| [docs/VERIFY.md](./docs/VERIFY.md) | Production hygiene gates |
| [docs/PHILOSOPHY_PI.md](./docs/PHILOSOPHY_PI.md) | Pi (earendil-works/pi) → Rust acex map |
| [docs/biographies/INDEX.md](./docs/biographies/INDEX.md) | Artifact lineage (biographies) |
| [skills/acex-dev/SKILL.md](./skills/acex-dev/SKILL.md) | Project skill for drop-in agents |

**Continue from last checkpoint:** follow [AGENTS.md#stateless-continuation-new-session](./AGENTS.md#stateless-continuation-new-session). In one line: the tracker checkpoint capsule is live planning state; the JSONL ledger is historical evidence; continuation work is not done until it has a PR with required checks green, merged when branch protection permits.

**Pillars:** Extensibility · Platform-agnostic core · Performance under agentic load · Interpretable observability.

## Status

Gates **G0**, **G1**, and recommended **G1.5** are complete/usable: board, filters, palette, focus, peek, send, start, wait, Zed open/diff, attach, worktree/workspace actions, paths chrome, reconnect/resnapshot, and cached ANSI peek polish. Current active phase is **G2 tracker-selected polish**; next-ready work comes from the checkpoint capsule in `docs/tracker.html` and is machine-readable via `cargo run -p acex -- --checkpoint-status`.

```bash
start docs/tracker.html   # Windows
```

## Workspace layout

```
acex/
  crates/
    herdr-types/     # protocol models + schemas/
    herdr-client/    # NDJSON client, reconnect, spawn
    acex-model/      # store + reducers + Intent
    acex-ui/         # ratatui
    acex-editor/     # Zed bridge
    acex-config/     # config
    acex-discover/   # package + skill discovery
    acex/            # binary
  docs/              # architecture, extend, verify, biographies, tracker
  skills/acex-dev/   # agent skill
  SOUL.md GOAL.md AGENTS.md
```

## Build & verify

Requires Rust (edition 2021+). Herdr is optional for offline-honest `--status`; live `--smoke` exercises the connect path.

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p acex -- --status           # Machine-readable conn/packages/skills
cargo run -p acex -- --checkpoint-status # Pure JSON continuation oracle; no Herdr spawn
cargo run -p acex -- --help             # Flag/usage list
cargo run -p acex -- --smoke            # Live or actionable offline + discovery JSON
cargo run -p acex -- --smoke-reconnect  # F04 reconnect path; may mutate local Herdr server
cargo run -p acex                       # TUI (q quit)
```
Observed Herdr schema/protocol: protocol 16, Herdr `0.7.2-preview.2026-07-07-f5354780e4ef` (observed 2026-07-15); `herdr-types` schema artifact is protocol 16. Current discovery and continuation state are machine-readable via `--status` and `--checkpoint-status`.


**Drop-in package dirs:** `.acex/packages/*/acex-package.toml`, `packages/*/acex-package.toml`, `skills/*/SKILL.md`.

**Config:** acex reads `config.toml` from the platform config dir under `acex/` (override with `ACEX_CONFIG_DIR`). Start presets live under `[[start_presets]]` with `id`, `name`, `argv`, and optional `cwd`; the start prompt accepts `@id`/`name` and still accepts freeform `name | cmd args` or bare `cmd args`.

**Attach handoff:** selected agents launch `herdr agent attach <target>`; named sessions launch `herdr session attach <name>`; default/explicit-socket session attach keeps the bare `herdr` fallback. Handoffs pass isolated `HERDR_SOCKET_PATH` / `HERDR_SESSION` env from acex config so non-default sessions attach the intended target.


**Windows pipe:** `\\.\pipe\{absolute path to herdr.sock}` (sock file is a marker).

## Non-goals (short)

No embedded terminal, no Zed fork, no cloud CP, no multi-session chrome before MVP.  
Full list in SOUL + tracker cut list.

## License

MIT OR Apache-2.0 (workspace metadata).
