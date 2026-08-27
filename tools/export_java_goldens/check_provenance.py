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
            unit = any(
                k in data
                for k in (
                    "decoded",
                    "heading_levels",
                    "exclude_keys",
                    "supported",
                    "expect_error",
                    "handle_xml_tag",
                    "filters_equal_same_config",
                    "word_count",
                )
            )
            if "id" not in data:
                errors.append(f"{path}: missing id")
            if not unit and "fixture" not in data:
                errors.append(f"{path}: missing fixture")
            if "sources" not in data and "decoded" not in data and not unit:
                errors.append(f"{path}: missing sources")
            fixture = data.get("fixture")
            if fixture and fixture != "html/entity-decode":
                src = ROOT / "fixtures" / "filters" / fixture
                java_src = ROOT / "reference" / "java" / "src" / "test" / "resources" / "data" / "filters" / fixture
                if not src.is_file() and not java_src.is_file():
                    errors.append(f"{path}: fixture not found {src}")
            if unit and data.get("supported"):
                for row in data["supported"]:
                    rel = row.get("fixture")
                    if not rel:
                        continue
                    src = ROOT / "fixtures" / "filters" / rel
                    java_src = (
                        ROOT
                        / "reference"
                        / "java"
                        / "src"
                        / "test"
                        / "resources"
                        / "data"
                        / "filters"
                        / rel
                    )
                    if not src.is_file() and not java_src.is_file():
                        errors.append(f"{path}: supported fixture not found {rel}")
        if "engine" in path.parts:
            inventory = any(k in data for k in ("cases", "dialects", "methods", "actions", "controllers", "tests", "keys"))
            if not inventory:
                errors.append(f"{path}: missing cases/inventory")
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print(f"provenance ok: {len(files)} golden files")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
