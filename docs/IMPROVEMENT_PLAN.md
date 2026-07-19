# Improvement plan queue

This file is a stateless execution queue for the next high-value acex fixes.
It is subordinate to `docs/tracker.html`: if this file and the tracker disagree,
**the tracker wins**.

## How to use this queue

1. Read `SOUL.md`, `GOAL.md`, `AGENTS.md`, `docs/tracker.html`, and this file.
2. Pick the **first unchecked task only**.
3. Create a branch named `improve/<ID>-<slug>` from the current `master`.
4. Before editing, grep for every other call site or duplicate copy of the pattern you are changing.
   Do not touch only some of the matches.
5. Make the smallest correct diff. Do not restyle code.
6. Run the task's verification commands, including the full test crate/file that owns the change.
7. If the task touches tracker-owned artifacts, perform the tracker/ledger update described below.
8. Mark the task `[x]` in this file and add the PR number / merged commit hash in the task notes.
9. Push the branch, open a PR, watch the required checks green, and merge it when branch protection allows.
10. Stop. The next session picks up at the next unchecked task.

### When to update tracker + ledger

Update `docs/tracker.html`, the checkpoint capsule, and append exactly one JSONL ledger entry when a task changes any of these repo-owned planning/doc artifacts:

- `docs/tracker.html`
- `docs/VERIFY.md`
- `skills/acex-dev/SKILL.md`
- `docs/biographies/**`
- `.github/**`
- `scripts/**`
- `docs/REPO_PROTECTION.md`

For code-only tasks that do **not** touch those artifacts, do not invent a tracker/ledger entry.

### Windows shell note

On this machine, `bash` may resolve to the WSL stub with no installed distro.
If that happens, run the gate with explicit Git Bash instead:

`"C:\Program Files\Git\bin\bash.exe" scripts/verify-pr.sh --offline-smoke`

## Completion format for each task

When a task lands, append the PR reference and merge result inline, for example:

- `PR #123` — merged `2026-07-19`
- or `PR #123` — green, blocked by CODEOWNERS (`@acephos`)

Keep the task order unchanged. Do not start task N+1 until task N is merged or explicitly blocked.

## Task queue

- [x] **IMP-01 — Add `--help` and unknown-flag rejection** — PR #21 — merged 2026-07-19

  **Why this matters**
  
  The binary currently falls through to the TUI for typos and has no usage output.
  That is bad operator ergonomics and makes automation harder to debug.

  **Files**
  
  - `crates/acex/src/main.rs`

  **Change**
  
  - Replace the current `args.iter().any(...)` flag scan with a tiny parser that recognizes only the current flags.
  - Add `-h` / `--help` handling that prints usage and exits `0` before config or tracing setup.
  - Reject unknown flags with usage on `stderr` and exit code `2`.
  - Keep the existing valid flags working exactly as they do now.

  **Verify**
  
  - `cargo test -p acex`
  - `cargo run -p acex -- --help`
  - `cargo run -p acex -- --definitely-not-a-flag` exits `2`

  **Done when**
  
  - The full `acex` test crate passes.
  - The help path and unknown-flag path behave as described.
  - The branch is pushed and the PR is merged, or the green PR URL plus blocker is reported.

- [x] **IMP-02 — Sync operator docs with the CLI surface** — PR #23 — opened 2026-07-19

  **Why this matters**
  
  The docs currently show a stale HERDR_E2E example and do not teach `--help`.
  New users should be able to copy the docs without guessing shell syntax.

  **Files**
  
  - `README.md`
  - `docs/VERIFY.md`
  - `skills/acex-dev/SKILL.md`

  **Change**
  
  - In `README.md`, add a short `cargo run -p acex -- --help` mention in the command examples or nearby operator guidance.
  - In `docs/VERIFY.md`, replace the stale `set HERDR_E2E=1` line with a real bash invocation, and keep the PowerShell note separate and explicit.
  - In `skills/acex-dev/SKILL.md`, mention `--help` alongside `--status` and `--checkpoint-status`, and make sure the verification command examples still match the repo scripts.

  **Verify**
  
  - `grep -n "--help\|HERDR_E2E=1 cargo test\|--offline-smoke" README.md docs/VERIFY.md skills/acex-dev/SKILL.md`
  - `cargo run -p acex -- --help`

  **Done when**
  
  - The docs copy cleanly in both bash and PowerShell contexts.
  - The task updates `docs/tracker.html` and appends exactly one ledger entry, because `docs/VERIFY.md` and `skills/acex-dev/SKILL.md` are tracker-owned artifacts.
  - The PR is merged, or the green PR URL plus blocker is reported.

- [ ] **IMP-03 — Add workspace lint policy**

  **Why this matters**
  
  The workspace has no lint boundary, so dangerous patterns can enter silently.
  A small manifest-level policy is cheap insurance.

  **Files**
  
  - `Cargo.toml`
  - `crates/acex/Cargo.toml`
  - `crates/acex-config/Cargo.toml`
  - `crates/acex-discover/Cargo.toml`
  - `crates/acex-editor/Cargo.toml`
  - `crates/acex-model/Cargo.toml`
  - `crates/acex-ui/Cargo.toml`
  - `crates/herdr-client/Cargo.toml`
  - `crates/herdr-types/Cargo.toml`

  **Change**
  
  - Add a workspace lint table in the root manifest.
  - Start with the smallest policy that is clearly safe for this repo, such as forbidding `unsafe_code` and denying `unused_must_use`.
  - Opt each crate into the workspace policy with `[lints] workspace = true`.
  - Do not change any source file for this task.

  **Verify**
  
  - `cargo check --workspace`
  - `cargo clippy --workspace --all-targets -- -D warnings`

  **Done when**
  
  - Every crate manifest has opted into the workspace lint policy.
  - The workspace still checks and clippy stays green.
  - This task does not need tracker/ledger updates because it only changes package manifests.

- [ ] **IMP-04 — Add `acex-editor` unit tests**

  **Why this matters**
  
  `acex-editor` currently has zero tests, so editor bridge regressions would be invisible.

  **Files**
  
  - `crates/acex-editor/src/lib.rs`

  **Change**
  
  - Add a `#[cfg(test)]` module.
  - Test that `ZedBridge::default()` uses `zed`.
  - Test that a missing binary returns `EditorError::NotFound` for both `open_path` and `diff`.
  - Keep the tests deterministic and avoid requiring a real editor installation.

  **Verify**
  
  - `cargo test -p acex-editor`

  **Done when**
  
  - The new unit tests cover the bridge's current observable behavior.
  - No other crates need to change.

- [ ] **IMP-05 — Trim and reject blank pane IDs**

  **Why this matters**
  
  Pane IDs flow into attach and selection logic as authoritative targets.
  Blank or whitespace IDs should never be accepted.

  **Files**
  
  - `crates/acex-model/src/lib.rs`

  **Change**
  
  - Update both `agent_from_list_row` and `agent_from_pane`.
  - Trim the `pane_id` string.
  - Reject empty or whitespace-only IDs before they become `AgentSummary` values.
  - Add a regression test for a blank/space-only `pane_id`.

  **Verify**
  
  - `cargo test -p acex-model`

  **Done when**
  
  - The parser accepts normal pane IDs unchanged.
  - Blank IDs no longer reach downstream selection/attach logic.

- [ ] **IMP-06 — Surface malformed Herdr response shapes**

  **Why this matters**
  
  The worker currently collapses unexpected `workspaces` / `worktrees` shapes into empty lists.
  That hides protocol drift as if nothing happened.

  **Files**
  
  - `crates/acex/src/worker.rs`

  **Change**
  
  - Keep the current empty-list behavior for truly missing fields.
  - When the field exists but has the wrong type or shape, emit a warning that includes the unexpected shape.
  - Leave the happy path unchanged.
  - Add tests for the malformed-shape case.

  **Verify**
  
  - `cargo test -p acex`

  **Done when**
  
  - Unexpected Herdr payload shapes are no longer silent.
  - Sibling tests in `worker.rs` still pass.

- [ ] **IMP-07 — Surface discovery scan failures in status JSON**

  **Why this matters**
  
  `discover_project()` currently logs and then returns `DiscoveryReport::default()`.
  Operators lose the failure signal unless they are watching stderr.

  **Files**
  
  - `crates/acex/src/main.rs`

  **Change**
  
  - In `discover_project()`, keep the warning log.
  - Replace the plain default report with a report that contains one `DiscoveryDiagnostic` entry describing the scan failure.
  - Use the public diagnostic fields directly; do not add a new helper unless you need one.
  - Make sure `--status` and `--checkpoint-status` still serialize valid JSON.

  **Verify**
  
  - `cargo test -p acex`
  - `cargo run -p acex -- --offline --status`
  - `cargo run -p acex -- --offline --checkpoint-status`

  **Done when**
  
  - Discovery failures are visible in the JSON diagnostics instead of being silently erased.
  - The rest of the status surface stays unchanged.

- [ ] **IMP-08 — Ignore the OMP scratch directory**

  **Why this matters**
  
  `.omp/` appears as untracked noise during local sessions and should not tempt accidental staging.

  **Files**
  
  - `.gitignore`

  **Change**
  
  - Add `.omp/` near the other repo-local scratch/build entries.
  - Do not change any other ignore rules.

  **Verify**
  
  - `git status --short` no longer shows `.omp/` after an OMP run.

  **Done when**
  
  - The scratch directory stays out of normal worktree noise.

## Appendix A — Ledger helper

Use this only for tasks that touch tracker-owned artifacts and need one new ledger line.
Run it from Git Bash.

```bash
python - <<'PY'
from __future__ import annotations

import argparse
import hashlib
import json
from datetime import datetime, timezone
from pathlib import Path

LEDGER = Path("docs/checkpoint-ledger.jsonl")
GENESIS = "GENESIS"


def canonical_hash(entry: dict) -> str:
    payload = dict(entry)
    payload.pop("hash", None)
    canonical = json.dumps(payload, sort_keys=True, separators=(",", ":"), ensure_ascii=False)
    return hashlib.sha256(canonical.encode("utf-8")).hexdigest()


def main() -> int:
    p = argparse.ArgumentParser()
    p.add_argument("--kind", required=True)
    p.add_argument("--state", required=True)
    p.add_argument("--next", dest="next_", required=True)
    p.add_argument("--refs", required=True, help="comma-separated list")
    p.add_argument("--evidence", required=True, help="semicolon-separated list")
    p.add_argument("--actor", default="agent")
    p.add_argument("--reason", default="entry created in same commit")
    args = p.parse_args()

    lines = LEDGER.read_text(encoding="utf-8").splitlines()
    prev_hash = json.loads(lines[-1])["hash"] if lines else GENESIS
    entry = {
        "schema_version": 1,
        "ts": datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z"),
        "kind": args.kind,
        "actor": args.actor,
        "commit": {"kind": "pending", "reason": args.reason},
        "refs": [item for item in args.refs.split(",") if item],
        "state": args.state,
        "evidence": [item for item in args.evidence.split(";") if item],
        "next": args.next_,
        "prev_hash": prev_hash,
    }
    entry["hash"] = canonical_hash(entry)

    lines.append(json.dumps(entry, ensure_ascii=False, sort_keys=True, separators=(",", ":")))
    LEDGER.write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(entry["hash"])
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
PY
```

## Appendix B — Tracker comment template

Insert the new comment immediately before the first existing comment under the `<!-- COMMENTS -->` section.
Use the current date in the `id` and `tag`.

```html
<div class="comment" id="2026-07-19-<slug>">
  <div class="meta"><span class="tag">2026-07-19</span> · <short title> · agent</div>
  <p style="margin:0">One-sentence summary with the PR number, test result, and the next checkpoint capsule note.</p>
</div>
```

When tracker-owned artifacts change, update the capsule fields too:

- `last_updated`
- `latest_comment_id`
- `ledger.latest_hash`
- `ledger.entry_count`
- `next_ready` (usually the next unchecked task ID; `[]` only when nothing is ready)

