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
cargo run -p acex -- --status
cargo run -p acex -- --smoke
python scripts/check-ledger-append-only.py origin/master
```
`--status` is included because it is the machine-readable discovery/connection contract used by agents and docs. The ledger check validates JSONL shape, hash chain, and append-only prefix preservation when a base ref exists.

### Current observed baseline (2026-07-14)

- `cargo fmt --all -- --check` — OK
- `cargo clippy --workspace --all-targets -- -D warnings` — OK
- `cargo test --workspace` — OK, 29 passed
- `cargo run -p acex -- --status` — OK live; offline status also OK
- `cargo run -p acex -- --smoke` — OK live
- Current discovery: packages=1, skills=1


### Smoke outcomes (both acceptable)

| Outcome | Meaning |
|---------|---------|
| `conn=Live` … | Herdr reachable; control plane healthy |
| Offline / actionable `err=…` without panic | Herdr missing/down; product failed closed honestly |
| `--status` JSON with `acex_status`, `conn`, `packages`, `skills`, `seams` | Machine-readable status/discovery contract healthy |

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

This check deliberately exercises F04 by stopping/respawning a local Herdr server. Keep it outside routine non-mutating verification unless reconnect behavior changed.

---

## What each gate protects

| Gate | Protects |
|------|----------|
| `fmt` | Diff noise, reviewability |
| `clippy -D warnings` | Footguns, dead code, needless clones |
| `test --workspace` | Pure reducers, resolve, mock RPC, discovery fixtures |
| `--status` | `acex-discover` package/skill scan and machine-readable status contract |
| `check-ledger-append-only.py` | JSONL checkpoint ledger shape, hash chain, and append-only prefix |
| `--smoke` | Binary entry + connect path |

---

## Agent definition of done

1. Gates above green (or smoke/status offline-honest where Herdr is intentionally unavailable).  
2. `docs/tracker.html` updated if intent/scope/status changed; tracker is the sole planning truth. Append durable checkpoint/process facts to `docs/checkpoint-ledger.jsonl`.
3. No SOUL ownership violations.  
4. New pure logic has a unit test calling **shipped** functions.

---

## Related

- [EXTENDING.md](./EXTENDING.md)  
- [ARCHITECTURE.md](./ARCHITECTURE.md)  
- [tracker.html](./tracker.html)  
