# Biography: OMP PR loop scripts

| | |
|--|--|
| **Path** | `scripts/omp-pr-loop.sh`, `scripts/omp-pr-loop.ps1` |
| **Purpose** | Run fresh foreground Oh My Pi continuation sessions one PR at a time, validate locally, open a GitHub PR, then wait for review/CI/merge before continuing. |
| **Origin** | Added when autonomous checkpoint continuation moved from direct commits to PR-shaped work units to avoid context exhaustion while preserving branch protection. |
| **Status** | Operator automation; intentionally does not bypass CODEOWNERS and defaults to waiting for human merge. |
| **How to change** | Keep foreground behavior, fresh GPT-5.5 1M high-reasoning `omp --model ... --smol ... --slow ... --plan ... --models ... --thinking high --no-session -p` invocations, local live verification before PR creation, and branch-protection wait semantics intact. Do not default to a synthetic OMP profile; inherit the user's authenticated profile unless `-Profile`/`--profile` is explicitly supplied. In PowerShell, invoke `omp` through the dedicated native-call helper instead of the generic wrapper so `-p` is passed to OMP rather than bound as an ambiguous PowerShell common parameter. Update tracker/ledger when policy changes. |
