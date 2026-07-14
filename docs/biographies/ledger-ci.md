# Biography: checkpoint ledger CI

| | |
|--|--|
| **Path** | `.github/workflows/ledger.yml` |
| **Purpose** | Run the checkpoint ledger validator on push and pull request so append-only/history checks are not only local convention. |
| **Origin** | Added with the JSONL checkpoint ledger after external best-practice review: format helps parsing; enforcement needs hooks/CI/server policy. |
| **Status** | Active GitHub Actions workflow; strongest when paired with branch protection requiring this check. |
| **How to change** | Keep fetch depth sufficient for base comparison. Do not remove the append-only validator without replacing it with stronger server-side enforcement. |
