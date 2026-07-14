# Biography: CODEOWNERS

| | |
|--|--|
| **Path** | `.github/CODEOWNERS` |
| **Purpose** | Requires owner review for tracker, ledger, validator, workflows, PR-loop automation, and agent operating manual changes. |
| **Origin** | Added during stateless continuation hardening so branch protection can enforce review on handoff-critical artifacts; extended when continuation moved to PR-shaped automation. |
| **Status** | Active when GitHub branch protection enables code-owner reviews. |
| **How to change** | Keep the protected path list aligned with ledger/verify workflows, PR-loop scripts, and `docs/REPO_PROTECTION.md`; append a checkpoint ledger entry. |
