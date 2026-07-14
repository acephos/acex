#!/usr/bin/env python3
"""Validate the JSONL checkpoint ledger and enforce append-only edits.

Usage:
  python scripts/check-ledger-append-only.py [base-ref] [--allow-missing-base] [--allow-missing-ledger-entry]

Checks:
- one JSON object per line, no blanks;
- schema_version=1 for new entries, with strict field/type validation;
- strict RFC3339 UTC timestamps ending in Z;
- SHA-256 hash chain over canonical JSON with `hash` removed;
- append-only prefix preservation against the provided base ref;
- meaningful planning/doc changes append exactly one new ledger line whose refs mention the changed artifacts.

Legacy ledger entries that already exist in the base prefix are hash-checked but
not schema-upgraded in place; append-only history must not be rewritten just to
add newly required fields. Every appended entry must use schema_version=1.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
import sys
from pathlib import Path
from typing import Any

LEDGER = Path("docs/checkpoint-ledger.jsonl")
GENESIS = "GENESIS"
SCHEMA_VERSION = 1
KINDS = {"checkpoint", "decision", "correction", "verification", "release", "process"}
STRICT_KEYS = {
    "schema_version",
    "ts",
    "kind",
    "actor",
    "commit",
    "refs",
    "state",
    "evidence",
    "next",
    "prev_hash",
    "hash",
}
CORRECTION_KEYS = STRICT_KEYS | {"corrects_hash"}
HEX40 = re.compile(r"^[0-9a-f]{40}$")
HEX64 = re.compile(r"^[0-9a-f]{64}$")
RFC3339_UTC = re.compile(r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z$")
MEANINGFUL_EXACT = {
    "AGENTS.md",
    "GOAL.md",
    "SOUL.md",
    "README.md",
    "docs/tracker.html",
    "docs/ARCHITECTURE.md",
    "docs/EXTENDING.md",
    "docs/VERIFY.md",
    "docs/PHILOSOPHY_PI.md",
    "skills/acex-dev/SKILL.md",
    ".github/CODEOWNERS",
    ".github/workflows/ledger.yml",
    "scripts/check-ledger-append-only.py",
    "docs/REPO_PROTECTION.md",
}
MEANINGFUL_PREFIXES = (
    "docs/biographies/",
    "docs/artifacts/",
    "docs/schemas/",
)


def fail(message: str) -> None:
    print(f"ledger check failed: {message}", file=sys.stderr)
    raise SystemExit(1)


def canonical_hash(entry: dict[str, Any]) -> str:
    payload = dict(entry)
    payload.pop("hash", None)
    canonical = json.dumps(payload, sort_keys=True, separators=(",", ":"), ensure_ascii=False)
    return hashlib.sha256(canonical.encode("utf-8")).hexdigest()


def current_lines() -> list[str]:
    if not LEDGER.exists():
        fail(f"missing {LEDGER}")
    return LEDGER.read_text(encoding="utf-8").splitlines()


def require_non_empty_string(entry: dict[str, Any], key: str, line_no: int) -> None:
    value = entry.get(key)
    if not isinstance(value, str) or not value.strip():
        fail(f"line {line_no} {key} must be a non-empty string")


def require_non_empty_string_array(entry: dict[str, Any], key: str, line_no: int) -> None:
    value = entry.get(key)
    if not isinstance(value, list) or not value:
        fail(f"line {line_no} {key} must be a non-empty array")
    if not all(isinstance(item, str) and item.strip() for item in value):
        fail(f"line {line_no} {key} entries must be non-empty strings")


def commit_reachable(sha: str) -> bool:
    result = subprocess.run(
        ["git", "cat-file", "-e", f"{sha}^{{commit}}"],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        check=False,
    )
    return result.returncode == 0


def validate_commit(value: Any, line_no: int) -> None:
    if isinstance(value, str):
        if not HEX40.fullmatch(value):
            fail(f"line {line_no} commit string must be a 40-hex SHA")
        if not commit_reachable(value):
            fail(f"line {line_no} commit {value} is not reachable from repo history")
        return
    if isinstance(value, dict):
        if set(value) != {"kind", "reason"}:
            fail(f"line {line_no} pending commit object has unexpected keys")
        if value.get("kind") != "pending":
            fail(f"line {line_no} commit object kind must be pending")
        reason = value.get("reason")
        if not isinstance(reason, str) or not reason.strip():
            fail(f"line {line_no} pending commit reason must be a non-empty string")
        return
    fail(f"line {line_no} commit must be a reachable 40-hex SHA or pending object")


def validate_schema(entry: dict[str, Any], line_no: int, previous_hashes: set[str]) -> None:
    if entry.get("schema_version") != SCHEMA_VERSION:
        fail(f"line {line_no} schema_version must be integer {SCHEMA_VERSION}")

    kind = entry.get("kind")
    allowed_keys = CORRECTION_KEYS if kind == "correction" else STRICT_KEYS
    unknown = set(entry) - allowed_keys
    missing = STRICT_KEYS - set(entry)
    if unknown:
        fail(f"line {line_no} unknown keys: {', '.join(sorted(unknown))}")
    if missing:
        fail(f"line {line_no} missing keys: {', '.join(sorted(missing))}")

    ts = entry.get("ts")
    if not isinstance(ts, str) or not RFC3339_UTC.fullmatch(ts):
        fail(f"line {line_no} ts must be strict RFC3339 UTC ending in Z")
    if kind not in KINDS:
        fail(f"line {line_no} kind must be one of {', '.join(sorted(KINDS))}")
    require_non_empty_string(entry, "actor", line_no)
    validate_commit(entry.get("commit"), line_no)
    require_non_empty_string_array(entry, "refs", line_no)
    require_non_empty_string(entry, "state", line_no)
    require_non_empty_string_array(entry, "evidence", line_no)
    require_non_empty_string(entry, "next", line_no)
    require_non_empty_string(entry, "prev_hash", line_no)
    require_non_empty_string(entry, "hash", line_no)
    if entry.get("prev_hash") != GENESIS and not HEX64.fullmatch(entry["prev_hash"]):
        fail(f"line {line_no} prev_hash must be GENESIS or a 64-hex SHA-256")
    if not HEX64.fullmatch(entry["hash"]):
        fail(f"line {line_no} hash must be a 64-hex SHA-256")
    if kind == "correction":
        corrects_hash = entry.get("corrects_hash")
        if not isinstance(corrects_hash, str) or not HEX64.fullmatch(corrects_hash):
            fail(f"line {line_no} correction entries must include a 64-hex corrects_hash")
        if corrects_hash not in previous_hashes:
            fail(f"line {line_no} corrects_hash does not reference a previous ledger hash")


def validate_legacy_schema(entry: dict[str, Any], line_no: int) -> None:
    """Compatibility for immutable pre-hardening entries already in base history."""
    required = {"ts", "kind", "actor", "commit", "refs", "state", "evidence", "next", "prev_hash", "hash"}
    missing = required - set(entry)
    if missing:
        fail(f"legacy line {line_no} missing keys: {', '.join(sorted(missing))}")
    require_non_empty_string_array(entry, "refs", line_no)
    require_non_empty_string_array(entry, "evidence", line_no)
    for key in ("ts", "kind", "actor", "state", "next", "prev_hash", "hash"):
        require_non_empty_string(entry, key, line_no)


def validate_jsonl(lines: list[str], legacy_prefix_len: int) -> list[dict[str, Any]]:
    if not lines:
        fail("ledger is empty")
    previous_hash = GENESIS
    previous_hashes: set[str] = set()
    entries: list[dict[str, Any]] = []
    for idx, line in enumerate(lines, start=1):
        if not line.strip():
            fail(f"blank line at {idx}; JSONL must be one object per line")
        try:
            entry = json.loads(line)
        except json.JSONDecodeError as exc:
            fail(f"line {idx} is not valid JSON: {exc}")
        if not isinstance(entry, dict):
            fail(f"line {idx} is not a JSON object")

        is_legacy = "schema_version" not in entry
        if is_legacy:
            if idx > legacy_prefix_len:
                fail(f"line {idx} missing schema_version; new entries must use schema_version={SCHEMA_VERSION}")
            validate_legacy_schema(entry, idx)
        else:
            validate_schema(entry, idx, previous_hashes)

        if entry.get("prev_hash") != previous_hash:
            fail(f"line {idx} prev_hash does not match previous entry")
        expected_hash = canonical_hash(entry)
        if entry.get("hash") != expected_hash:
            fail(f"line {idx} hash mismatch; expected {expected_hash}")
        previous_hashes.add(expected_hash)
        previous_hash = expected_hash
        entries.append(entry)
    return entries


def base_lines(ref: str, allow_missing: bool) -> list[str] | None:
    result = subprocess.run(
        ["git", "show", f"{ref}:{LEDGER.as_posix()}"],
        text=True,
        capture_output=True,
        check=False,
    )
    if result.returncode != 0:
        if allow_missing:
            print(f"ledger check warning: base {ref} has no readable {LEDGER}; bootstrap mode", file=sys.stderr)
            return None
        fail(f"cannot read {LEDGER} from base ref {ref}; pass --allow-missing-base only for bootstrap/manual use")
    return result.stdout.splitlines()


def validate_append_only(base: list[str] | None, current: list[str]) -> None:
    if base is None:
        return
    if len(current) < len(base):
        fail(f"ledger shrank: base has {len(base)} lines, current has {len(current)}")
    for idx, (old, new) in enumerate(zip(base, current), start=1):
        if old != new:
            fail(f"line {idx} changed; append corrections as new entries instead of editing history")


def changed_files(ref: str) -> list[str]:
    result = subprocess.run(
        ["git", "diff", "--name-only", ref, "--"],
        text=True,
        capture_output=True,
        check=False,
    )
    if result.returncode != 0:
        fail(f"cannot compute changed files against {ref}: {result.stderr.strip()}")
    return [line.strip().replace("\\", "/") for line in result.stdout.splitlines() if line.strip()]


def is_meaningful(path: str) -> bool:
    return path in MEANINGFUL_EXACT or any(path.startswith(prefix) for prefix in MEANINGFUL_PREFIXES)


def ref_mentions_path(refs: list[str], path: str) -> bool:
    for ref in refs:
        if ref == path:
            return True
        if ref.endswith("/*") and path.startswith(ref[:-1]):
            return True
        if ref.endswith("/**") and path.startswith(ref[:-2]):
            return True
    return False


def validate_tracker_ledger_coupling(
    ref: str,
    base: list[str] | None,
    current: list[str],
    entries: list[dict[str, Any]],
    allow_missing_ledger_entry: bool,
) -> None:
    if base is None:
        return
    changed = [path for path in changed_files(ref) if is_meaningful(path) and path != LEDGER.as_posix()]
    if not changed:
        return
    appended = len(current) - len(base)
    if appended != 1:
        if allow_missing_ledger_entry:
            print(
                "ledger check warning: meaningful files changed without exactly one appended ledger entry",
                file=sys.stderr,
            )
            return
        fail(
            f"meaningful planning/docs changed ({', '.join(changed)}); expected exactly one new ledger line, found {appended}"
        )
    refs = entries[-1].get("refs", [])
    if not isinstance(refs, list):
        fail("new ledger entry refs must be a list")
    missing_refs = [path for path in changed if not ref_mentions_path(refs, path)]
    if missing_refs:
        fail(
            "new ledger entry refs do not mention changed artifacts: " + ", ".join(missing_refs)
        )


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("base_ref", nargs="?", help="base git ref for append-only prefix comparison")
    parser.add_argument(
        "--allow-missing-base",
        action="store_true",
        help="bootstrap/manual only: do not fail when base ref lacks the ledger",
    )
    parser.add_argument(
        "--allow-missing-ledger-entry",
        action="store_true",
        help="emergency/manual only: allow meaningful docs changes without exactly one new ledger line",
    )
    return parser.parse_args(argv[1:])


def main(argv: list[str]) -> int:
    args = parse_args(argv)
    current = current_lines()
    base = base_lines(args.base_ref, args.allow_missing_base) if args.base_ref else None
    validate_append_only(base, current)
    legacy_prefix_len = len(base) if base is not None else len(current)
    entries = validate_jsonl(current, legacy_prefix_len)
    if args.base_ref:
        validate_tracker_ledger_coupling(
            args.base_ref,
            base,
            current,
            entries,
            args.allow_missing_ledger_entry,
        )
    print(f"ledger ok: {len(current)} entries")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
