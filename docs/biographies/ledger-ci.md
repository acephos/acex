# Biography: checkpoint ledger CI

| | |
|--|--|
| **Path** | `.github/workflows/ledger.yml` |
| **Purpose** | Run an inline protected prefix check and then the checkpoint ledger validator on push and pull request so append-only/history checks are not only local convention. |
| **Origin** | Added with the JSONL checkpoint ledger after external best-practice review: format helps parsing; enforcement needs hooks/CI/server policy. Hardened so PRs cannot rely solely on their mutable branch copy of the validator for prefix preservation, and so force-pushed branch events fall back to the default branch when the previous branch tip is no longer fetchable. |
| **Status** | Active GitHub Actions workflow; strongest when paired with branch protection requiring this check. Hygiene gates live separately in `.github/workflows/verify.yml`. |
| **How to change** | Keep fetch depth sufficient for base comparison and preserve the pull-request/default-branch base fallback. Do not remove the inline prefix check or append-only validator without replacing them with stronger server-side enforcement. |
