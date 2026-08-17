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

## P2 notes

filters2 Java `*FilterTest` methods are exported and `assert_eq` green,
including `HTMLFilter2Test` in full. HTML parse is `FilterVisitor` +
`HTMLOptions` + `HTMLWriter` (the 154-line block-tag regex file is gone).
Text write uses `LineLengthLimitWriter`. PO write is the Java line state
machine (UTF-8 replacement matches `InputStreamReader`). The P2 STATUS row
stays `scaffold`: filters3/4 test-method goldens, dialect tag sets, and the
other honesty gates are still red.

## P3 notes

23 filters3 dialects snapshot against `dialect_tags.json` (`assert_eq` of
paragraph / intact / out_of_turn / preformat / attrs / tag_attrs /
constraints). Camtasia intact is the Java list (includes
`AudioClickSensitivity`, `QuestionGroup_Array`, `Zoom_Array`). OpenDoc /
OpenXML goldens include `empty_write_parts` and `translated_write_parts`
(unzipped XML, not the zip bytes). `sniff_xml` still returns `None` for
unknown XML. Each filters3 `*FilterTest#test*` has an ExportGoldens JSON
and `p3_filters3_all_java_test_goldens` `assert_eq`s them (DocBook SYSTEM
entities, XLIFF `constructShortcuts` / `bpt`+`ept` pairing, FilterVisitor
HTML already in P2). The P3 STATUS row stays `scaffold`: Lucene identity
stems, editor / menu / locale gates, and later-wave honesty gates remain
red.

## P4 notes

filters4 Java `*FilterTest` methods are exported one JSON each
(`Xliff1FilterTest` 9, `Xliff2FilterTest` 5, `MsOfficeFileFilterTest` 5,
`OpenXmlFilterTest` 1). `p4_filters4_all_java_test_goldens` `assert_eq`s
sources / ids / paths / existing translations / write-back. Office ZIP
write lands on the corresponding `w:t` node; repeated sources each get
one replacement.

`.docx` / `.xlsx` / `.pptx` `FilterRegistry.for_path` selects filters3
`openxml` (`org.omegat.filters3.xml.openxml.OpenXMLFilter`). That path
has a golden (`engine/for_path_office.json` + parse vs
`openxml/testParse.json`). filters4 `msoffice`
(`MsOfficeFileFilter`) is selected by **id**, not by those extensions.
`OpenXmlFilter` (filters4 StAX part parser) is the ZIP inner processor;
`document.xml` `isFileSupported` is covered by
`msoffice/testOpenXmlFilterIsFileSupported.json`.

SdlXliff / SdlProject have no `*Test`; they keep `processFile` fixture
goldens. The P4 STATUS row stays `scaffold`.

## P5 notes

Each `Lucene*Tokenizer` now calls that class’s Analyzer pipeline (Snowball via
`rust-stemmers`, Lucene Light / 3.0 / Arabic / Hindi / Brazilian / Stempel-light
ports, Japanese lexicon+baseform+CJKWidth, Thai dictionary break, SmartChinese
longest-match). `stems::identity` / shared `slavic`/`romance`/`nordic` are gone.
`engine_goldens::tokens_match_java_lists` `assert_eq`s every exported case,
including the Japanese Wikipedia sentence (NONE/GLOSSARY/MATCHING), Thai
`ภาษาไทย…`, and Arabic `اللغة العربية…`. The P5 STATUS row stays `scaffold`:
editor / menu / locale / git2 gates remain red.

## P6 notes

Hunspell reads `PFX`/`SFX` with FLAG char/long/num. Lucene-Hunspell and
Morfologik load from distinct `fixtures/spell/{hunspell,lucene,morfologik}`
paths (colour / color / kolor). `ensure_lang` copies real `.aff`/`.dic`
from `reference/java/language-modules` for **ca / es / fa / fr / ga / gl /
pt / uk**. StarDict is `.ifo`+`.idx`+`.dict`/`.dict.dz`; DSL includes
`.dsl.dz`. LanguageTool with no URL emits `severity=info`; `fixture:`
parses `v2/check` `matches[].message` / `rule.id` / `offset`.

Languages in `language-modules` **without** an affix pair (download to
`config/spell` or keep as `parity_gap`): ar, ast, be, br, da, de, el, en,
eo, it, ja, km, nl, pl, ro, ru, sk, sl, sv, ta, tl, zh. CI uses the small
aff/dic fixtures; those are not 30-language product dictionaries.

The P6 STATUS row stays `scaffold` until later-wave honesty gates are green.

## P7 notes

`gui/editor` 63 Java classes each have a TS file. `Document3` holds the
active translation range, dirty flag, tag atoms, and styled spans.
`IEditor` implements the exported method set (gap empty vs
`ieditor_methods.json`). Each Marker computes intervals; goldens
`assert_eq` NBSP / whitespace / bidi / protected-tag ranges. Autocompleter
views: Glossary / Autotext / CharTable / HistoryCompleter /
HistoryPredictor (next-word) / Tag. The P7 row stays `scaffold`.

## P8 notes

120 `*ActionPerformed` ids are wired to observable behavior (`project.edit`
opens the properties dialog, not the wizard; `project.team-new` opens the
team flow; `edit.pdf` inserts U+202C). 25 preference controllers have
pages; Java keys are typed `controller_keys` (save still drops `extra`).
`SegmentationCustomizer` is a rule table. Nine docks are splitters (Dict/MT
are not a pinned aside). `RepositoriesMappingController` UI exists.
`className="placeholder"` is gone. The P8 row stays `scaffold`.

## P9 notes

Seven MT connectors use recorded HTTP under `fixtures/mt/<engine>/`.
Offline without a fixture fails and does not block the editor. External
Finder GUI edits XML and `finder.run` opens the URL. Five completer views
are keyboard-insertable. The P9 row stays `scaffold`.

## P10 notes

`GITRemoteRepository2` product path is `git2` (clone/fetch/reset/commit/push
+ credential callback). `Command::new("git")` remains only in `lib.rs`
tests that seed a bare repo. Mapping include/exclude UI is
`RepositoriesMappingController`. TMX and glossary rebase plus Keep
ours/theirs/manual stay. The P10 row stays `scaffold`.

## P11 notes

Aligner: HEAPWISE / PARSEWISE / ID; Viterbi ≠ Forward-Backward; CHAR/WORD
and Poisson vs Normal. Boa `editor` bindings cover the IEditor method set.
Wiki MediaWiki XML → source; MED unzip; CLI leftover flags remain in
`--help`. No `fallback_eval`. The P11 row stays `scaffold`.

## Intentional non-goals (must still have a full replacement)

- Java JAR plugins are not loaded. Replacement: `omegat-plugin.toml` + cdylib.
- Groovy is not executed. Replacement: embedded Boa with the Java binding
  surface (`IEditor` / `IProject` / `IGlossary` / `console` / `mainWindow` /
  `Core`). `fallback_eval` is forbidden.
- LanguageTool is not an embedded JAR. Replacement: HTTP `v2/check`, with an
  `severity=info` downgrade item when no URL is configured.
