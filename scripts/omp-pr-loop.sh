#!/usr/bin/env bash
set -euo pipefail

base_branch="master"
remote="origin"
max_iters=40
prompt="continue from the last checkpoint"
wait_for_merge=1
auto_merge=0
profile=""
model="github-copilot/gpt-5.5-1m"
thinking="high"

usage() {
  cat <<'EOF'
usage: scripts/omp-pr-loop.sh [options]

Runs one fresh foreground Oh My Pi continuation session per iteration, validates
it locally, pushes the result to a branch, opens a PR, then waits in the
foreground for that PR to merge before starting the next iteration.

Options:
  --base BRANCH          protected base branch (default: master)
  --remote NAME          git remote to push to (default: origin)
  --max-iters N          loop safety cap (default: 40)
  --prompt TEXT          prompt injected into each fresh omp session
  --profile NAME         optional isolated omp profile (default: current authenticated profile)
  --model MODEL          OMP model to force (default: github-copilot/gpt-5.5-1m)
  --thinking LEVEL       OMP thinking level to force (default: high)
  --auto-merge           request GitHub auto-merge after opening the PR
  --no-wait              open one PR and stop instead of waiting for merge
  -h, --help             show this help

Policy note: this script does not bypass CODEOWNERS. With the repository's
current protection policy, PRs touching tracker/ledger paths require @acephos
review. Use --auto-merge only to queue merge after required reviews/checks pass.
EOF
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --base)
      base_branch="${2:?--base requires a branch}"
      shift 2
      ;;
    --remote)
      remote="${2:?--remote requires a remote name}"
      shift 2
      ;;
    --max-iters)
      max_iters="${2:?--max-iters requires a number}"
      shift 2
      ;;
    --prompt)
      prompt="${2:?--prompt requires text}"
      shift 2
      ;;
    --profile)
      profile="${2:?--profile requires a name}"
      shift 2
      ;;
    --model)
      model="${2:?--model requires a model}"
      shift 2
      ;;
    --thinking)
      thinking="${2:?--thinking requires a level}"
      shift 2
      ;;
    --auto-merge)
      auto_merge=1
      shift
      ;;
    --no-wait)
      wait_for_merge=0
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

need() {
  command -v "$1" >/dev/null 2>&1 || {
    echo "missing required command: $1" >&2
    exit 127
  }
}

repo_clean() {
  [ -z "$(git status --porcelain)" ]
}

checkpoint_field() {
  local file="$1" expr="$2"
  python -c 'import json,sys; d=json.load(open(sys.argv[1], encoding="utf-8")); print(eval(sys.argv[2], {}, {"d": d}) or "")' "$file" "$expr"
}

need cargo
need gh
need git
need omp
need python

gh auth status -h github.com >/dev/null

if ! repo_clean; then
  echo "working tree is dirty; start the PR loop from a clean checkout" >&2
  exit 2
fi

for iter in $(seq 1 "$max_iters"); do
  echo "=== omp PR iteration $iter ==="
  git checkout "$base_branch"
  git pull --ff-only "$remote" "$base_branch"

  status_before="$(mktemp)"
  cargo run --quiet -p acex -- --checkpoint-status > "$status_before"
  ready_count="$(checkpoint_field "$status_before" 'len(d["tracker"]["next_ready"])')"
  if [ "$ready_count" = "0" ]; then
    echo "tracker.next_ready is empty; loop complete"
    rm -f "$status_before"
    exit 0
  fi

  feature="$(checkpoint_field "$status_before" 'd["tracker"]["next_ready"][0]')"
  prev_hash="$(checkpoint_field "$status_before" 'd["ledger"]["latest_hash"]')"
  branch_feature="$(printf '%s' "$feature" | tr '[:upper:]' '[:lower:]' | tr -c 'a-z0-9._-' '-')"
  branch="omp/${branch_feature}-$(date -u +%Y%m%d%H%M%S)"
  rm -f "$status_before"

  git checkout -b "$branch"
  omp_args=(--model "$model" --smol "$model" --slow "$model" --plan "$model" --models "$model" --thinking "$thinking")
  if [ -n "$profile" ]; then
    omp_args+=(--profile "$profile")
  fi
  omp_args+=(--no-session -p "$prompt")
  omp "${omp_args[@]}"

  scripts/verify-pr.sh --base-ref "$remote/$base_branch"

  if repo_clean; then
    echo "omp iteration produced no file changes; refusing to open an empty PR" >&2
    exit 3
  fi

  status_after="$(mktemp)"
  cargo run --quiet -p acex -- --checkpoint-status > "$status_after"
  new_hash="$(checkpoint_field "$status_after" 'd["ledger"]["latest_hash"]')"
  if [ "$new_hash" = "$prev_hash" ]; then
    echo "ledger hash did not advance; refusing to open PR" >&2
    rm -f "$status_after"
    exit 4
  fi
  tracker_valid="$(checkpoint_field "$status_after" 'd["tracker"]["valid"]')"
  ledger_valid="$(checkpoint_field "$status_after" 'd["ledger"]["valid"]')"
  if [ "$tracker_valid" != "True" ] || [ "$ledger_valid" != "True" ]; then
    echo "checkpoint-status reports invalid tracker or ledger" >&2
    rm -f "$status_after"
    exit 5
  fi
  rm -f "$status_after"

  git add -A
  git commit -m "omp: ${feature} checkpoint ${new_hash:0:12}"
  git push -u "$remote" "$branch"

  body_file="$(mktemp)"
  cat > "$body_file" <<EOF
Automated continuation PR for ${feature}.

Prompt injected into a fresh foreground omp session:

\`\`\`
${prompt}
\`\`\`

Local validation before opening this PR:

\`\`\`
scripts/verify-pr.sh --base-ref ${remote}/${base_branch}
\`\`\`

Checkpoint ledger advanced from ${prev_hash} to ${new_hash}.

This PR intentionally does not bypass CODEOWNERS; the foreground loop resumes only after required review, CI, and merge.
EOF

  pr_url="$(gh pr create --base "$base_branch" --head "$branch" --title "omp: ${feature} continuation" --body-file "$body_file")"
  rm -f "$body_file"
  echo "opened $pr_url"

  gh pr checks "$pr_url" --watch --fail-fast

  if [ "$auto_merge" -eq 1 ]; then
    gh pr merge "$pr_url" --auto --squash --delete-branch
  fi

  if [ "$wait_for_merge" -eq 0 ]; then
    echo "not waiting for merge (--no-wait); stop here and rerun after merge"
    exit 0
  fi

  echo "waiting in foreground for CODEOWNERS review and merge: $pr_url"
  while true; do
    merged_at="$(gh pr view "$pr_url" --json mergedAt --jq '.mergedAt // ""')"
    if [ -n "$merged_at" ]; then
      echo "merged at $merged_at"
      break
    fi
    state="$(gh pr view "$pr_url" --json state --jq '.state')"
    if [ "$state" = "CLOSED" ]; then
      echo "PR closed without merge: $pr_url" >&2
      exit 6
    fi
    sleep 60
  done

done

echo "max iteration cap reached: $max_iters" >&2
exit 7
