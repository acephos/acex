# Biography: verify CI

| | |
|--|--|
| **Path** | `.github/workflows/verify.yml` |
| **Purpose** | Run production hygiene gates on pull requests and pushes so PRs cannot merge with only the ledger check green. |
| **Origin** | Added when continuation work moved to PRs and required CI had to mirror `docs/VERIFY.md`. |
| **Status** | Active GitHub Actions workflow. Uses offline status/smoke because hosted runners lack a live Herdr daemon; local PR loop still runs live smoke before opening PRs. |
| **How to change** | Keep job name aligned with `docs/REPO_PROTECTION.md` required check (`verify / hygiene`) and preserve script syntax checks for loop helpers. |
