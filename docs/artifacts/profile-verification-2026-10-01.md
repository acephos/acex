# Profile verification — 2026-10-01

Purpose: reproducible onboarding and dated proof with explicit coverage limits. Origin: GitHub profile review. Status: Linux live and offline measurements recorded; CI expands platform/MSRV checks. Change by rerunning commands and appending dated evidence rather than rewriting historical records.

## Linux live integration

Downloaded upstream release `preview-2026-07-07-f5354780e4ef`, matching the stored protocol-16 schema, to an isolated tool directory. Used an isolated `HERDR_SOCKET_PATH`, `HERDR_CONFIG_PATH`, and XDG configuration/data directories. No existing Herdr session was touched.

`HERDR_E2E=1 cargo test -p herdr-client --test live_herdr -- --nocapture` passed both tests: ping/snapshot and workspace-created subscription. The server initially had zero workspaces; the subscription fixture created one empty workspace. `cargo run -p acex -- --smoke` reported `conn=Live` without an error. The [status record](linux-live-status-2026-10-01.json) and [asciinema v2 recording](linux-live-2026-10-01.cast) show the actual Linux control-plane connection. Replay the recording with `asciinema play docs/artifacts/linux-live-2026-10-01.cast`.

The TUI was opened in a 156×48 PTY and quit with `q` (exit 0). A subsequent live smoke confirmed the server remained running. This empty-workspace run does not demonstrate concurrent coding agents, all palette actions, reconnection under load, or current Herdr release compatibility. No live Windows Herdr test was performed in this revision. Historical Windows claims remain historical evidence; added Windows/macOS CI jobs exercise offline contracts and unit/mock transport coverage.

## Offline performance

Run `cargo run --locked --release -p acex-ui --example measure -- docs/artifacts/offline-board.txt`.

The example calls shipped `Store::apply_event` and UI `render` functions for 16, 64, and 256 synthetic agents at 156×48. After warm-up it samples 1,800 reductions and 200 frames per size. [Raw results and environment](performance-2026-10-01.json) contain p50/p95/p99 timings. Frame p99 was 0.531, 0.659, and 0.789 ms; reducer p99 was 0.591, 1.553, and 3.567 μs respectively on this run. The [text frame](offline-board.txt) is explicitly a synthetic offline fixture.

This is one local release-build observation with no regression threshold. It excludes socket traffic, locks under worker contention, PTYs, terminal output I/O and coding-agent throughput. Repeated runs on reference hardware and a live multi-agent workload are required for broader performance claims.

## Metadata and continuation safety

Cargo repository URLs now identify this repository. License files implement the existing `MIT OR Apache-2.0` declaration. Locked dependency metadata requires Rust 1.88; `cargo check --workspace --locked` passed on 1.88.0. CI checks that minimum explicitly.

The capsule referenced ledger entry 23 while the validated ledger contained 24 entries. It was repaired to the actual tail before further work. The status probe now rejects mismatched count/hash and unsupported capsule versions; regression checks call the shipped status function and preserve `herdr.side_effects=none`. Ledger history is retained unchanged, with this revision recorded as one new entry.
