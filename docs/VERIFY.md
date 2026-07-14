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
scripts/verify-pr.sh --base-ref origin/master
```

Equivalent expanded local gates:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p acex -- --status
cargo run -p acex -- --checkpoint-status
cargo run -p acex -- --smoke
python scripts/check-ledger-append-only.py origin/master
```
`scripts/verify-pr.sh` is the local/CI wrapper for these gates. Hosted GitHub runners use `scripts/verify-pr.sh --offline-smoke` because they do not have a live Herdr daemon; local operator/PR-loop runs keep the stronger live `--smoke` before opening a PR. `--status` is included because it is the machine-readable live discovery/connection contract used by agents and docs. `--checkpoint-status` is the pure JSON continuation oracle; it must not spawn Herdr and must report tracker capsule state, ledger validity, git state, and discovery diagnostics. The ledger check validates JSONL shape, hash chain, tracker-ledger coupling, and append-only prefix preservation when a base ref exists.

### Current observed baseline

Use the tracker checkpoint capsule and latest ledger entry for dated proof. Do not treat this section as live planning truth.


### Smoke outcomes (both acceptable)

| Outcome | Meaning |
|---------|---------|
| `conn=Live` … | Herdr reachable; control plane healthy |
| Offline / actionable `err=…` without panic | Herdr missing/down; product failed closed honestly |
| `--status` JSON with `acex_status`, `conn`, `packages`, `skills`, `diagnostics`, and `seams` | Machine-readable status/discovery contract healthy |
| `--checkpoint-status` JSON with `schema_version`, `git`, `tracker`, `ledger`, `herdr.side_effects=none`, and `discovery.diagnostics` | Stateless continuation oracle healthy |

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
| `test --workspace` | Pure reducers, resolve, mock RPC, discovery fixtures, checkpoint status golden contract |
| `--status` | `acex-discover` package/skill scan, diagnostics, and live/offline machine-readable status contract |
| `--checkpoint-status` | No-spawn stateless continuation oracle: tracker capsule, ledger validity, git state, discovery diagnostics |
| `check-ledger-append-only.py` | JSONL checkpoint ledger schema, hash chain, tracker-ledger coupling, and append-only prefix |
| `--smoke` | Binary entry + connect path; CI uses `--offline --smoke` while local PR automation requires live smoke |

### Foreground PR continuation loop

Use `scripts/omp-pr-loop.ps1` on Windows or `scripts/omp-pr-loop.sh` on Unix-like shells to run one fresh foreground OMP continuation session per PR. The loop uses the verified OMP CLI shape `omp --profile <name> --no-session -p "continue from the last checkpoint"` so each iteration is `/new`-like, isolated to a named profile, and not saved as a resumable session. It validates locally with live smoke before opening a PR, pushes a branch, opens the PR, watches checks, and then waits in the foreground for CODEOWNERS review and merge before starting the next iteration. It does not self-approve or bypass branch protection.

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
