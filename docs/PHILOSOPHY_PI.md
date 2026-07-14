# Pi philosophy → acex (idiomatic Rust)

> Reference: [earendil-works/pi](https://github.com/earendil-works/pi) (coding agent harness by Earendil Works / formerly Mario’s pi).  
> acex is **not** a coding agent. We adopt Pi’s **self-aware, self-extensible** maintainability philosophy — not its TypeScript monorepo or LLM runtime.

## Biography

| | |
|--|--|
| **Purpose** | Define “Pi-like” for acex as outcomes mapped to Rust seams. |
| **Origin** | Goal pass comparing acex control plane to earendil-works/pi. |
| **Status** | Canonical philosophy map; linked from AGENTS/README. |
| **How to change** | When discovery or extension model changes; note tracker. |

---

## What Pi optimizes for

| Pi idea | Meaning in Pi | **acex Rust equivalent** |
|---------|---------------|---------------------------|
| **Minimal core** | Small harness; features as extensions/skills/packages | Keep SOUL ownership: thin `acex` bin; pure `acex-model`; I/O in `herdr-client` / `acex-editor` / `acex-ui` only |
| **Progressive disclosure** | Startup lists skill *names+descriptions*; full `SKILL.md` loaded on demand | `acex-discover` scans manifests → summary list; detail load is separate API (no full doc parse at hot path) |
| **Discovery paths** | `~/.pi/...`, `.pi/skills`, packages, settings | Project: `.acex/packages/*/acex-package.toml`, `packages/*/acex-package.toml`, `skills/*/SKILL.md` |
| **Agent-readable rules** | `AGENTS.md`, docs the agent can explain | SOUL/GOAL/AGENTS + linked architecture/extend/verify/biographies |
| **Self-extensible** | Drop files → auto-discover without forking core | Drop package dir with TOML manifest → appears in `--status` / smoke JSON; wire code via Intent/registry recipe |
| **Machine-facing modes** | RPC / JSON / print streams | `--smoke` + `--status` parseable JSON (stable keys) |
| **Packages as bundles** | Share extensions + skills + themes | Declarative `acex-package.toml` (+ optional skill paths); no npm runtime |
| **Hot reload (TS)** | `/reload` re-jiti extensions | **Do not port** — Rust uses recompile for code hooks; rediscover manifests without rebuild |

---

## Explicit “do not port”

| Pi mechanism | Why not in acex |
|--------------|-----------------|
| jiti / runtime TypeScript extension loading | Unsafe & unidiomatic; use manifests + compile-time registry |
| LLM agent-core / tool loop | acex is a Herdr **control plane**, not a coding agent |
| `pi install` npm packages | Out of scope; git drop-in dirs only |
| Full TUI extension API (`ctx.ui.custom`) | Keep palette/Intent seams; no plugin UI host |
| Session JSONL tree / compaction | Different product domain |
| Cloning `packages/coding-agent` folder layout | TS monorepo patterns ≠ Rust crate graph |

---

## Mapped extension loop (Pi-like, Rust-native)

```
discover (fs scan, pure)  →  list summaries (smoke/status JSON)
                          →  progressive detail (optional load)
                          →  register into palette/intent via documented seams
                          →  tracker + skill docs for agents
```

1. **Discover** — `acex_discover::scan` (shipped, unit-tested).  
2. **Disclose lightly** — name + description only in default listing.  
3. **Extend** — [EXTENDING.md](./EXTENDING.md): Intent → palette → worker; packages may *declare* actions that map to known intent ids.  
4. **Prove** — [VERIFY.md](./VERIFY.md); discovery fixture tests call shipped scan.

---

## Vocabulary bridge

| Pi term | acex term |
|---------|-----------|
| skill | `skills/*/SKILL.md` or package-contributed skill path |
| extension | compile-time crate code + optional package manifest metadata |
| package | `.acex/packages/<id>/acex-package.toml` drop-in |
| resources_discover | `acex_discover::scan` / `--status` |
| AGENTS.md | `AGENTS.md` + this map |
| RPC/JSON mode | `--status` / smoke JSON block |

---

## Related

- [EXTENDING.md](./EXTENDING.md)  
- [ARCHITECTURE.md](./ARCHITECTURE.md)  
- [VERIFY.md](./VERIFY.md)  
- [skills/acex-dev/SKILL.md](../skills/acex-dev/SKILL.md)  
