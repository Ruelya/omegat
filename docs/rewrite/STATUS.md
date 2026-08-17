# Rewrite status

Legend:

- `scaffold` — present in the tree, **not** accepted
- `parity_gap` — specified remaining delta with **measured** numbers
- `parity` — accepted against **Java-exported** goldens (`assert_eq`) **and**
  the structural honesty gates for that row

A full-table `parity` is forbidden until P12: zero `scaffold` rows **and**
`tools/honesty/check.sh` is green. A row may become `parity` only after that
wave’s Java `*Test` method set is exported and `assert_eq` green.

Only the Java reference tree itself is `parity` today. Everything else is
`scaffold`. Previous G0–G10 `parity` marks were withdrawn after an adversarial
audit: the tree is a golden-driven CAT workstation, not a finished 6.2 rewrite.

| Area | Wave | Status |
|---|---|---|
| Java reference tree at `reference/java` | G0 | parity |
| Honest STATUS + ACCEPTANCE (this file) | P0 | scaffold |
| Java Gradle exporter `exportGoldens` (honesty surfaces) | P0 | scaffold |
| Structural honesty gates (`tools/honesty/check.sh`) | P0 | scaffold |
| Text / PO / HTML Java-exported goldens | G0 | scaffold |
| Filter / align / SRX fixtures under `fixtures/` | G0 | scaffold |
| Sidecar method contract tests | G0 | scaffold |
| RealProject / SRX / TMX / matching / stats / tags | P1 | scaffold |
| filters2: 21 Filter classes; HTML = FilterVisitor | P2 | scaffold |
| filters3: 23 Dialect tag sets + OpenDoc/OpenXML write-back | P3 | scaffold |
| filters4: ZIP / XLIFF / SDL / Office node write-back | P4 | scaffold |
| Tokenizers: named Lucene Analyzer pipelines | P5 | scaffold |
| Spell / dictionaries / LanguageTool | P6 | scaffold |
| Editor: 63 `gui/editor` classes + Marker goldens | P7 | scaffold |
| Desktop: 120 menus, 25 controllers, 9 docks | P8 | scaffold |
| 7 MT engines, External Finder, autocompleter | P9 | scaffold |
| team2: 23 classes; GIT via `git2` | P10 | scaffold |
| Aligner, Boa `IEditor` surface, Wiki / MED / CLI | P11 | scaffold |
| 41 locales, packages, plugin ABI, manual | P12 | scaffold |

## What is not accepted (must stay scaffold until rebuilt)

These are defects, not “accepted algorithms”:

- `stems::identity` in any `lucene_*.rs` (ar/th/hi/fa/hy/ga/lv/id)
- Shared `stems::slavic` / `romance` / `nordic` across Lucene languages that
  do not share a Java Analyzer
- Hard-coded golden word tables (Turkish / Chinese match tables)
- HTML/HHC parse whose only path is a block-tag regex + `replacen`
  (`crates/omegat-filters/src/html.rs`); Java is `FilterVisitor` (~920 lines)
- Dialect tag sets shorter than Java `dialect_tags.json` (Camtasia intact is
  ~20 tags here vs ~160 in Java)
- `Preferences.extra` as a writable model (load-only migration may remain;
  save must not emit `extra`)
- `contentEditable`, `fallback_eval`, `translate_mock` as engine main paths
- `Command::new("git")` as the product path of `GITRemoteRepository2`
  (`crates/omegat-team/src/team_utils.rs` → `run_git`)
- Menu `switch` that opens the same wizard for `project.edit` / `project.team-new`
- A single textarea/input standing in for `SegmentationCustomizer` or
  `Edit*OptionsDialog`
- Dock `className="placeholder"`
- Token goldens that only run `"Hello worlds running"` + `NONE` for a
  non-English Lucene tokenizer
- `n >= N`, `contains`, `must_contain`, or a fake `java_test` as a green test
- STATUS full-table `parity`

## P0 notes (this wave)

P0 does **not** mark any product feature `parity`. It only:

1. Restores an honest STATUS table (this file).
2. Restates the completion definition in `ACCEPTANCE.md`.
3. Extends `ExportGoldens` so the honesty surfaces have a defined JSON shape:
   `dialect_tags.json`, `ieditor_methods.json`, `menu_actions.json`,
   `preference_keys.json`, `filter_tests.json`, HTMLFilter2Test-per-method
   goldens, and Lucene tokenizer × `{NONE,GLOSSARY,MATCHING}` on **that
   language’s** text.
4. Adds `tools/honesty/check.sh`, wired in CI, **allowed to be red** on this
   tree.

Existing `exported_by=org.omegat.tools.ExportGoldens` goldens stay as a
**minimum floor**. They are not class-complete proof.

## P1 notes

`engine_goldens` now `assert_eq`s the Java method sets for Segmenter,
LevenshteinDistance, TagValidation, TagRepair, TMXWriter, FindMatches, and
CalcMatchStatistics (including per-source `StatCount` + `calcMaxSimilarity`).
The P1 STATUS row stays `scaffold`: honesty gates are still red (identity
stems, HTML regex, dialect tag sets, …) and this wave does not claim HTML
compile parity.

## Intentional non-goals (must still have a full replacement)

- Java JAR plugins are not loaded. Replacement: `omegat-plugin.toml` + cdylib.
- Groovy is not executed. Replacement: embedded Boa with the Java binding
  surface (`IEditor` / `IProject` / `IGlossary` / `console` / `mainWindow` /
  `Core`). `fallback_eval` is forbidden.
- LanguageTool is not an embedded JAR. Replacement: HTTP `v2/check`, with an
  `severity=info` downgrade item when no URL is configured.
