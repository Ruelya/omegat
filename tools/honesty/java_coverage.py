#!/usr/bin/env python3
"""Inventory Java *Test methods vs ExportGoldens java_test fields.

Used by the honesty gates. A STATUS row marked `parity` fails when that
wave's required Java test classes still have methods without a golden.
"""

from __future__ import annotations

import json
import re
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
METHOD_RE = re.compile(r"public\s+void\s+(test[A-Za-z0-9_]*)\s*\(")
CLASS_RE = re.compile(r"class\s+(\w+Test)\b")
PKG_RE = re.compile(r"package\s+([\w.]+);")

from waves import EXCLUDED_TESTS, STATUS_WAVE_ALIASES, WAVE_REQUIRED_TESTS


def java_test_roots() -> list[Path]:
    return [
        ROOT / "reference/java/src/test",
        ROOT / "reference/java/aligner/src/test",
    ]


def collect_java_methods() -> list[str]:
    out: list[str] = []
    for root in java_test_roots():
        if not root.is_dir():
            continue
        for path in root.rglob("*Test.java"):
            text = path.read_text(encoding="utf-8", errors="replace")
            pkg = PKG_RE.search(text)
            cls = CLASS_RE.search(text)
            if not pkg or not cls:
                continue
            fqn = f"{pkg.group(1)}.{cls.group(1)}"
            for m in METHOD_RE.finditer(text):
                out.append(f"{fqn}#{m.group(1)}")
    return out


def walk_java_tests(obj: object, acc: list[str]) -> None:
    if isinstance(obj, dict):
        jt = obj.get("java_test")
        if isinstance(jt, str) and "#" in jt:
            acc.append(jt)
        for v in obj.values():
            walk_java_tests(v, acc)
    elif isinstance(obj, list):
        for v in obj:
            walk_java_tests(v, acc)


def collect_golden_java_tests() -> list[str]:
    acc: list[str] = []
    gold = ROOT / "fixtures/goldens"
    if not gold.is_dir():
        return acc
    for path in gold.rglob("*.json"):
        try:
            data = json.loads(path.read_text(encoding="utf-8"))
        except json.JSONDecodeError:
            continue
        walk_java_tests(data, acc)
    return acc


def coverage() -> dict[str, object]:
    methods = collect_java_methods()
    golds = collect_golden_java_tests()
    java_set = set(methods)
    gold_set = set(golds)
    missing = sorted(java_set - gold_set)
    extra = sorted(gold_set - java_set)
    by_class: dict[str, list[str]] = defaultdict(list)
    for m in missing:
        by_class[m.split("#", 1)[0]].append(m)
    wave_missing: dict[str, list[str]] = {}
    for wave, classes in WAVE_REQUIRED_TESTS.items():
        miss: list[str] = []
        for cls in classes:
            miss.extend(by_class.get(cls, []))
        wave_missing[wave] = miss
        alias = next((p for p, r in STATUS_WAVE_ALIASES.items() if r == wave), None)
        if alias:
            wave_missing[alias] = miss
    assigned = {c for classes in WAVE_REQUIRED_TESTS.values() for c in classes} | set(EXCLUDED_TESTS)
    unassigned = sorted({m.split("#", 1)[0] for m in methods if m.split("#", 1)[0] not in assigned})
    in_scope_missing = [m for m in missing if m.split("#", 1)[0] not in EXCLUDED_TESTS]
    return {
        "java_methods": len(methods),
        "golden_unique": len(gold_set),
        "missing": missing,
        "in_scope_missing": in_scope_missing,
        "extra": extra,
        "wave_missing": wave_missing,
        "unassigned": unassigned,
        "excluded": sorted(EXCLUDED_TESTS),
    }


def parse_status_waves(text: str) -> dict[str, str]:
    """Map wave id (P1, P7, …) to status cell."""
    out: dict[str, str] = {}
    for ln in text.splitlines():
        if not ln.startswith("|"):
            continue
        cells = [c.strip() for c in ln.strip("|").split("|")]
        if len(cells) < 3:
            continue
        wave, status = cells[1], cells[2]
        if wave in {"Wave", "---"} or status not in {"parity", "scaffold", "parity_gap"}:
            continue
        out[wave] = status
    return out


def english_phrase_leftovers() -> list[str]:
    """Locale values that are English UI phrases but not the same-key en.json value.

    The same-key leftover gate misses remapped Bundle titles such as
    `glossary=Glossaries` when `en.glossary` is `Glossary`.
    """
    i18n = ROOT / "apps/desktop/src/renderer/i18n"
    en_path = i18n / "en.json"
    if not en_path.is_file():
        return ["en.json missing"]
    en = json.loads(en_path.read_text(encoding="utf-8"))
    phrases = {v for v in en.values() if isinstance(v, str) and v and v != "OmegaT"}
    # Bundle menu titles that differ from the shortened desktop key.
    phrases.update(
        {
            "Glossaries",
            "Preferences...",
            "Character Table",
            "Auto-completion",
            "Machine Translation",
            "Notepad",
            "Project Folder",
            "Replace All",
            "Regular expressions",
            "Align Files...",
            "Download Team Project...",
            "History Completion",
            "History Prediction",
            "Use as Default Translation",
            "Create Alternative Translation",
            "Source Files",
            "Colours",
            "Saving and Output",
            "Minimal threshold to show a fuzzy match",
            "Highlight Segments with Alternative Translation",
            "Team synchronization...",
        }
    )
    hits: list[str] = []
    for path in sorted(i18n.glob("*.json")):
        if path.name == "en.json":
            continue
        data = json.loads(path.read_text(encoding="utf-8"))
        for key, val in data.items():
            if not isinstance(val, str) or val == "OmegaT":
                continue
            ev = en.get(key)
            if val in phrases and val != ev:
                hits.append(f"{path.stem}.{key}={val!r} (en={ev!r})")
    return hits


def product_editor_uses_document3() -> bool:
    editor = ROOT / "apps/desktop/src/renderer/editor/SegmentEditor.tsx"
    if not editor.is_file():
        return False
    text = editor.read_text(encoding="utf-8")
    return "Document3" in text


if __name__ == "__main__":
    cov = coverage()
    print(f"java_test*={cov['java_methods']} golden_unique={cov['golden_unique']} missing={len(cov['missing'])}")
    for wave, miss in cov["wave_missing"].items():
        print(f"  {wave} required-missing={len(miss)}")
    print(f"english_phrase_leftovers={len(english_phrase_leftovers())}")
    print(f"product_document3={product_editor_uses_document3()}")
