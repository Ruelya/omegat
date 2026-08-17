#!/usr/bin/env python3
"""Re-emit committed goldens. Source of truth is the Java test methods named in each JSON."""

from pathlib import Path
import json
import sys

ROOT = Path(__file__).resolve().parents[2]
GOLDENS = ROOT / "fixtures" / "goldens"


def main() -> int:
    missing = []
    for path in sorted(GOLDENS.rglob("*.json")):
        data = json.loads(path.read_text(encoding="utf-8"))
        if "filters" in path.parts:
            for key in ("id", "fixture", "sources"):
                if key not in data:
                    missing.append(f"{path}: missing {key}")
            fixture = ROOT / "fixtures" / "filters" / data.get("fixture", "")
            if data.get("fixture") and not fixture.is_file():
                missing.append(f"{path}: fixture not found {fixture}")
        if "engine" in path.parts and "cases" not in data:
            missing.append(f"{path}: missing cases")
    if missing:
        print("\n".join(missing), file=sys.stderr)
        return 1
    print(f"validated {len(list(GOLDENS.rglob('*.json')))} golden files")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
