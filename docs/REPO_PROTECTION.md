# Repository protection

> Settings required to make the JSONL checkpoint ledger and tracker handoff policy enforceable on GitHub.

## Biography

| | |
|--|--|
| **Purpose** | Documents branch protection settings that cannot be fully encoded in repository files. |
| **Origin** | Added with stateless continuation hardening after recognizing validator/CI alone cannot prevent force-push or admin bypass. |
| **Status** | Required repository policy for protected branches. |
| **How to change** | Update when workflow names, protected paths, or review policy changes; append a checkpoint ledger entry. |

---

## Required GitHub settings

For `master` and any release branch:

1. Require pull requests before merging.
2. Require review from Code Owners for paths named in `.github/CODEOWNERS`.
3. Require the `ledger / checkpoint-ledger` workflow status check.
4. Require the `verify / hygiene` workflow status check.
5. Disallow force pushes.
6. Disallow branch deletion.
7. Restrict admin bypass if the repository plan permits it; otherwise record any admin bypass in `docs/checkpoint-ledger.jsonl` as a `process` or `correction` entry.
8. Keep Actions enabled for pull requests and pushes with `fetch-depth: 0` in the ledger workflow.
9. PR-loop automation must not bypass Code Owners: it opens a PR, optionally queues auto-merge, and waits in the foreground until human review, required checks, and merge complete.

## Why code is not enough

`docs/checkpoint-ledger.jsonl` is append-friendly and hash-chained, but Git history can still be rewritten without server-side policy. The repository therefore needs both:

- in-repo checks: validator, workflow prefix check, verify workflow, CODEOWNERS;
- repository settings: required `ledger / checkpoint-ledger` and `verify / hygiene` status checks, code-owner review, no force pushes.

The tracker checkpoint capsule remains live planning authority; the ledger remains historical evidence.
