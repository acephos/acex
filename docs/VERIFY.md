# Verify — production hygiene

> Run from repository root (`acex/`). Linked from [AGENTS.md](../AGENTS.md).

## Biography

| | |
|--|--|
| **Purpose** | Single checklist for “is this change shippable?” |
| **Origin** | Production-ready pass; encodes fmt/clippy/test/smoke bar. |
| **Status** | Gate for agent PRs and local release confidence. |
| **Change** | Edit when new gates are added (e.g. MSRV); mirror in AGENTS.md Commands. |

---

## Required gates (must all pass for production hygiene)

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p acex -- --smoke
```

### Smoke outcomes (both acceptable)

| Outcome | Meaning |
|---------|---------|
| `conn=Live` … | Herdr reachable; control plane healthy |
| Offline / actionable `err=…` without panic | Herdr missing/down; product failed closed honestly |

Never treat a panic backtrace as success.

### Optional live E2E

```bash
# Unix or Windows with herdr running
set HERDR_E2E=1   # pwsh: $env:HERDR_E2E="1"
cargo test -p herdr-client --test live_herdr -- --nocapture
```

### Reconnect smoke (mutates local Herdr server)

```bash
cargo run -p acex -- --smoke-reconnect
```

---

## What each gate protects

| Gate | Protects |
|------|----------|
| `fmt` | Diff noise, reviewability |
| `clippy -D warnings` | Footguns, dead code, needless clones |
| `test --workspace` | Pure reducers, resolve, mock RPC |
| `--smoke` | Binary entry + connect path |

---

## Agent definition of done

1. Gates above green (or smoke offline-honest).  
2. `docs/tracker.html` updated if intent/scope changed.  
3. No SOUL ownership violations.  
4. New pure logic has a unit test calling **shipped** functions.

---

## Related

- [EXTENDING.md](./EXTENDING.md)  
- [ARCHITECTURE.md](./ARCHITECTURE.md)  
- [tracker.html](./tracker.html)  
