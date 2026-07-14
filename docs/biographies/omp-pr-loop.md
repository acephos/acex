# Biography: OMP PR loop scripts

| | |
|--|--|
| **Path** | `scripts/omp-pr-loop.sh`, `scripts/omp-pr-loop.ps1` |
| **Purpose** | Run fresh foreground Oh My Pi continuation sessions one PR at a time, validate locally, open a GitHub PR, then wait for review/CI/merge before continuing. |
| **Origin** | Added when autonomous checkpoint continuation moved from direct commits to PR-shaped work units to avoid context exhaustion while preserving branch protection. |
| **Status** | Operator automation; intentionally does not bypass CODEOWNERS and defaults to waiting for human merge. |
| **How to change** | Keep foreground behavior, fresh `omp --profile <name> --no-session -p` invocations, local live verification before PR creation, and branch-protection wait semantics intact. Update tracker/ledger when policy changes. |
