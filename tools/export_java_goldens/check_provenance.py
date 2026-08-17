#!/usr/bin/env python3
"""Refuse goldens that were not produced by the Java exporter.

This is not an exporter. It does not invent segment lists.
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
GOLDENS = ROOT / "fixtures" / "goldens"
VOIDED = GOLDENS / "_voided"

JAVA_TEST = re.compile(r"^org\.omegat\.[A-Za-z0-9_.]+#[A-Za-z0-9_]+$")
EXPORTER = "org.omegat.tools.ExportGoldens"


def main() -> int:
    errors: list[str] = []
    files = [
        p
        for p in sorted(GOLDENS.rglob("*.json"))
        if VOIDED not in p.parents and p.parent != VOIDED
    ]
    if not files:
        print("no goldens under fixtures/goldens/ (excluding _voided)", file=sys.stderr)
        return 1
    for path in files:
        try:
            data = json.loads(path.read_text(encoding="utf-8"))
        except json.JSONDecodeError as exc:
            errors.append(f"{path}: invalid JSON: {exc}")
            continue
        if "voided" in data and data["voided"] is True:
            errors.append(f"{path}: voided golden must live under fixtures/goldens/_voided/")
            continue
        if "must_contain" in data or (
            isinstance(data.get("translated"), dict) and "must_contain" in data["translated"]
        ):
            errors.append(f"{path}: must_contain is forbidden")
        java_test = data.get("java_test") or data.get("java_source")
        if not isinstance(java_test, str) or not JAVA_TEST.match(java_test.split()[0] if " " not in java_test else ""):
            if not (isinstance(java_test, str) and JAVA_TEST.match(java_test)):
                errors.append(f"{path}: java_test must be org.omegat…#method, got {java_test!r}")
        if data.get("exported_by") != EXPORTER:
            errors.append(f"{path}: exported_by must be {EXPORTER}")
        if "filters" in path.parts:
            for key in ("id", "fixture", "sources"):
                if key not in data:
                    errors.append(f"{path}: missing {key}")
            fixture = data.get("fixture")
            if fixture:
                src = ROOT / "fixtures" / "filters" / fixture
                if not src.is_file():
                    errors.append(f"{path}: fixture not found {src}")
        if "engine" in path.parts and "cases" not in data:
            errors.append(f"{path}: missing cases")
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print(f"provenance ok: {len(files)} golden files")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
