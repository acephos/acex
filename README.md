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
| [docs/ARCHITECTURE.md](./docs/ARCHITECTURE.md) | Crate ownership + data flow |
| [docs/EXTENDING.md](./docs/EXTENDING.md) | How to add actions / transports / editors |
| [docs/VERIFY.md](./docs/VERIFY.md) | Production hygiene gates |
| [docs/PHILOSOPHY_PI.md](./docs/PHILOSOPHY_PI.md) | Pi (earendil-works/pi) → Rust acex map |
| [docs/biographies/INDEX.md](./docs/biographies/INDEX.md) | Artifact lineage (biographies) |
| [skills/acex-dev/SKILL.md](./skills/acex-dev/SKILL.md) | Project skill for drop-in agents |

**Pillars:** Extensibility · Platform-agnostic core · Performance under agentic load · Interpretable observability.

## Status

Phase **0 / G0 complete**. Phase **1** control plane is usable (board + palette + actions).  
Production hygiene: `fmt` + `clippy -D warnings` + `test` + smoke entry path.

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
    acex/            # binary
  docs/              # architecture, extend, verify, biographies, tracker
  skills/acex-dev/   # agent skill
  SOUL.md GOAL.md AGENTS.md
```

## Build & verify

Requires Rust (edition 2021+). Herdr optional for offline-honest smoke.

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p acex -- --smoke            # Live or actionable offline + discovery JSON
cargo run -p acex -- --status           # Machine-readable packages/skills/conn (no TUI)
cargo run -p acex -- --smoke-reconnect  # F04 (stops local herdr server)
cargo run -p acex                       # TUI (q quit)
```

**Drop-in package dirs:** `.acex/packages/*/acex-package.toml`, `packages/*/acex-package.toml`, `skills/*/SKILL.md`.

**Windows pipe:** `\\.\pipe\{absolute path to herdr.sock}` (sock file is a marker).

## Non-goals (short)

No embedded terminal, no Zed fork, no cloud CP, no multi-session chrome before MVP.  
Full list in SOUL + tracker cut list.

## License

TBD.
