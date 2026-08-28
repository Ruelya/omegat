# Honesty gates

`check.sh` is the structural gate for the 6.2 rewrite. Failure is a CI red.

P0 commits the script **red** on the current tree. Later waves may turn only
their own items green. Do not mark a STATUS row `parity` while the matching
item here is red.

Run from the repository root:

```
bash tools/honesty/check.sh
```

The script always runs every check and prints a summary. Exit status is
nonzero if any check failed.

Additional wave-honesty checks (2026-08-18 audit):

- `java_coverage.py` inventories every `public void test*` under
  `reference/java/src/test` and `aligner/src/test` against nested
  `java_test` fields. A `parity` STATUS wave fails if its required
  classes still have missing goldens. The missing list is written to
  `missing_java_tests.txt`.
- P7 may not be `parity` unless `SegmentEditor.tsx` references
  `Document3`.
- P12 may not be `parity` while leftover English phrases equal a
  *different* `en.json` value (e.g. `glossary=Glossaries`).
- `P12_GATES_GREEN` is not a full-table-parity bypass.
