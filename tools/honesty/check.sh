#!/usr/bin/env bash
# Structural honesty gates. Failure is CI red.
# P0 commits this red on the current tree.
set -u
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
exec python3 tools/honesty/check.py
