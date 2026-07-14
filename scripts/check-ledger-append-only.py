#!/usr/bin/env python3
"""Validate the JSONL checkpoint ledger and enforce append-only edits.

Usage:
  python scripts/check-ledger-append-only.py [base-ref]

Checks:
- one valid JSON object per line;
- required checkpoint fields;
- monotonic timestamps;
- SHA-256 hash chain (`prev_hash` + `hash`);
- if base-ref is provided, every base line is an identical prefix of the current ledger.

JSONL is append-friendly, not append-only by itself. This script supplies the
repo-level enforcement; branch protection/CI should run it for stronger guarantees.
"""

from __future__ import annotations

import hashlib
import json
import subprocess
import sys
from pathlib import Path
from typing import Any

LEDGER = Path("docs/checkpoint-ledger.jsonl")
REQUIRED_KEYS = {
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
GENESIS = "GENESIS"


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


def validate_jsonl(lines: list[str]) -> None:
    if not lines:
        fail("ledger is empty")
    previous_ts = ""
    previous_hash = GENESIS
    for idx, line in enumerate(lines, start=1):
        if not line.strip():
            fail(f"blank line at {idx}; JSONL must be one object per line")
        try:
            entry = json.loads(line)
        except json.JSONDecodeError as exc:
            fail(f"line {idx} is not valid JSON: {exc}")
        if not isinstance(entry, dict):
            fail(f"line {idx} is not a JSON object")
        missing = REQUIRED_KEYS - set(entry)
        if missing:
            fail(f"line {idx} missing keys: {', '.join(sorted(missing))}")
        if not isinstance(entry.get("refs"), list) or not entry["refs"]:
            fail(f"line {idx} refs must be a non-empty list")
        if not isinstance(entry.get("evidence"), list) or not entry["evidence"]:
            fail(f"line {idx} evidence must be a non-empty list")
        ts = str(entry.get("ts", ""))
        if previous_ts and ts < previous_ts:
            fail(f"line {idx} timestamp {ts} is older than previous {previous_ts}")
        if entry.get("prev_hash") != previous_hash:
            fail(f"line {idx} prev_hash does not match previous entry")
        expected_hash = canonical_hash(entry)
        if entry.get("hash") != expected_hash:
            fail(f"line {idx} hash mismatch; expected {expected_hash}")
        previous_ts = ts
        previous_hash = expected_hash


def base_lines(ref: str) -> list[str] | None:
    result = subprocess.run(
        ["git", "show", f"{ref}:{LEDGER.as_posix()}"],
        text=True,
        capture_output=True,
        check=False,
    )
    if result.returncode != 0:
        return None
    return result.stdout.splitlines()


def validate_append_only(base: list[str] | None, current: list[str]) -> None:
    if base is None:
        return
    if len(current) < len(base):
        fail(f"ledger shrank: base has {len(base)} lines, current has {len(current)}")
    for idx, (old, new) in enumerate(zip(base, current), start=1):
        if old != new:
            fail(f"line {idx} changed; append corrections as new entries instead of editing history")


def main(argv: list[str]) -> int:
    lines = current_lines()
    validate_jsonl(lines)
    if len(argv) > 1:
        validate_append_only(base_lines(argv[1]), lines)
    print(f"ledger ok: {len(lines)} entries")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
