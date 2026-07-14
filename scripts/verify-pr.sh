#!/usr/bin/env bash
set -euo pipefail

base_ref=""
offline_smoke=0

usage() {
  cat <<'EOF'
usage: scripts/verify-pr.sh [--base-ref REF] [--offline-smoke]

Runs the acex PR verification gates. Use --base-ref (for example
origin/master) when validating a branch before opening a PR so the ledger
validator can enforce append-only prefix and tracker/ledger coupling. Use
--offline-smoke in hosted CI where Herdr is intentionally not installed; local
operator loops should prefer the live smoke default.
EOF
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --base-ref)
      base_ref="${2:?--base-ref requires a ref}"
      shift 2
      ;;
    --offline-smoke)
      offline_smoke=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "unknown argument: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
if [ "$offline_smoke" -eq 1 ]; then
  cargo run -p acex -- --offline --status
else
  cargo run -p acex -- --status
fi
cargo run -p acex -- --checkpoint-status
if [ "$offline_smoke" -eq 1 ]; then
  cargo run -p acex -- --offline --smoke
else
  cargo run -p acex -- --smoke
fi
if [ -n "$base_ref" ]; then
  python scripts/check-ledger-append-only.py "$base_ref"
else
  python scripts/check-ledger-append-only.py
fi
