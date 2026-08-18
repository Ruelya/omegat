# Rewrite status

Legend:

- `scaffold` — present in the tree, **not** accepted
- `parity_gap` — specified remaining delta with **measured** numbers
- `parity` — accepted against **Java-exported** goldens (`assert_eq`) **and**
  the structural honesty gates for that row

P12: `tools/honesty/check.sh` is green and this table has **no `scaffold`
rows**. A row is `parity` only after that wave’s Java `*Test` method set is
exported and `assert_eq` green, plus the matching honesty item.

| Area | Wave | Status |
|---|---|---|
| Java reference tree at `reference/java` | G0 | parity |
| Honest STATUS + ACCEPTANCE (this file) | P0 | parity |
| Java Gradle exporter `exportGoldens` (honesty surfaces) | P0 | parity |
| Structural honesty gates (`tools/honesty/check.sh`) | P0 | parity |
| Text / PO / HTML Java-exported goldens | G0 | parity |
| Filter / align / SRX fixtures under `fixtures/` | G0 | parity |
| Sidecar method contract tests | G0 | parity |
| RealProject / SRX / TMX / matching / stats / tags | P1 | parity |
| filters2: 21 Filter classes; HTML = FilterVisitor | P2 | parity |
| filters3: 23 Dialect tag sets + OpenDoc/OpenXML write-back | P3 | parity |
| filters4: ZIP / XLIFF / SDL / Office node write-back | P4 | parity |
| Tokenizers: named Lucene Analyzer pipelines | P5 | parity |
| Spell / dictionaries / LanguageTool | P6 | parity_gap |
| Editor: 63 `gui/editor` classes + Marker goldens | P7 | parity_gap |
| Desktop: 120 menus, 25 controllers, 9 docks | P8 | parity_gap |
| 7 MT engines, External Finder, autocompleter | P9 | parity_gap |
| team2: 23 classes; GIT via `git2` | P10 | parity_gap |
| Aligner, Boa `IEditor` surface, Wiki / MED / CLI | P11 | parity_gap |
| 41 locales, packages, plugin ABI, manual | P12 | parity_gap |

## Remaining measured gap

- **P6 spell dictionaries**: 8 language-modules ship `.aff`/`.dic` (ca, es,
  fa, fr, ga, gl, pt, uk). **22** modules have no affix pair in-tree
  (ar, ast, be, br, da, de, el, en, eo, it, ja, km, nl, pl, ro, ru, sk,
  sl, sv, ta, tl, zh). CI uses the small `fixtures/spell` aff/dic files
  (not 30 product dictionaries).
- **P7 editor**: 63 TS files exist. Java `*Test` methods under
  `gui/editor` now have ExportGoldens-shaped JSON (markers, predictor,
  completer, EditorUtils, DocumentFilter3, SegmentExportImport,
  EditorController). `EditorControllerTest` translation range **31/31**
  is the Java fixture number for source `XXX` / empty translation, not a
  full Swing `insertString` port. `DocumentFilter3` models `isPossible`
  without `FilterBypass`.
- **P8 desktop**: 120 menu ids have observable-behavior tests in
  `actions.test.ts`. Keyboard walkthrough of new→translate 3→save→compile
  is not an automated `assert_eq` of a Java GUI log.
- **P9 MT / Finder**: 7 recorded fixtures `assert_eq` expected
  translations. No Java completer exporter goldens.
- **P10 team**: GIT product path is `git2`. SVN checkout/update/commit is
  `#[ignore]` (needs `svn` + `svnadmin`). HTTP two-client rebase uses
  `assert_eq` on conflict `ours`/`theirs`.
- **P11 align**: HEAPWISE / PARSEWISE / ID goldens `assert_eq` the Java
  pair lists (heap pair 3 merges the long EN sentence with “Where shall
  it end?”). Viterbi ≠ Forward-Backward; Poisson ≠ Normal. Remaining:
  Java `BundleTest` encodings are not a Rust resource bundle; aligner
  GUI is the Electron window, not Swing.
- **P12 ship**: Bundle locales leftover_eq_en = **0** (brand `OmegaT`
  only). Packaged manuals are `en` + `zh-CN` + Java HTML pointer, not 41
  languages. Packages are unsigned.

Rebuilt defects (honesty green; do not regress):

- no `stems::identity` / shared slavic/romance/nordic / golden word tables
- HTML parse is FilterVisitor, not a block-tag regex
- dialect tag sets match `dialect_tags.json`
- `Preferences.extra` is load-only; save drops it
- no `contentEditable` / `fallback_eval` / `translate_mock` product path
- GIT product path is `git2`, not `Command::new("git")`
- `project.edit` / `project.team-new` are distinct windows
- SegmentationCustomizer is a rule table
- no dock `className="placeholder"`
- Lucene goldens are NONE+GLOSSARY+MATCHING on that language’s text

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
`ภาษาไทย…`, and Arabic `اللغة العربية…`. Honesty identity-stem / token
items are green.

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

The P6 STATUS row is `parity_gap` (22 language-modules without aff/dic).

## P7 notes

`gui/editor` 63 Java classes each have a TS file. `Document3` holds the
active translation range, dirty flag, tag atoms, styled spans, edit/trusted
flags, and chrome `translationStart`/`translationEnd`. `DocumentFilter3`
rejects edits outside the translation range unless trusted. `IEditor`
implements the exported method set (gap empty vs `ieditor_methods.json`).
Each Java marker/predictor/completer/`EditorUtils`/`DocumentFilter3`/
`SegmentExportImport`/`EditorController` `*Test` method has an
ExportGoldens-shaped JSON and a TS `assert_eq`. Autocompleter views:
Glossary / Autotext / CharTable / HistoryCompleter / HistoryPredictor
(next-word) / Tag. The P7 row stays `parity_gap` (Swing chrome / FilterBypass).

## P8 notes

120 `*ActionPerformed` ids are wired to observable behavior (`project.edit`
opens the properties dialog, not the wizard; `project.team-new` opens the
team flow; `edit.pdf` inserts U+202C). 25 preference controllers have
pages; Java keys are typed `controller_keys` (save still drops `extra`).
`SegmentationCustomizer` is a rule table. Nine docks are splitters (Dict/MT
are not a pinned aside). `RepositoriesMappingController` UI exists.
`className="placeholder"` is gone. Honesty menu / placeholder items are
green. The P8 row stays `parity_gap` (no Java GUI walkthrough log).

## P9 notes

Seven MT connectors use recorded HTTP under `fixtures/mt/<engine>/`.
Offline without a fixture fails and does not block the editor. External
Finder GUI edits XML and `finder.run` opens the URL. Five completer views
are keyboard-insertable. Recorded fixtures `assert_eq` the Java parse
shapes; offline without a fixture is an error. The P9 row stays
`parity_gap` (no Java completer exporter).

## P10 notes

`GITRemoteRepository2` product path is `git2` (clone/fetch/reset/commit/push
+ credential callback). `Command::new("git")` remains only in `lib.rs`
tests that seed a bare repo. Mapping include/exclude UI is
`RepositoriesMappingController`. TMX and glossary rebase plus Keep
ours/theirs/manual stay. HTTP two-client rebase `assert_eq`s conflict
`ours`/`theirs`. SVN product path is the `svn` binary and the
checkout/update/commit test is `#[ignore]`. Honesty git-command item is
green. The P10 row stays `parity_gap` (SVN ignored).

## P11 notes

Aligner: HEAPWISE / PARSEWISE / ID; Viterbi ≠ Forward-Backward; CHAR/WORD
and Poisson vs Normal. Goldens are the Java pair lists (heap pair 3 is
the long EN sentence merged with “Where shall it end?”). Boa `editor`
bindings cover the IEditor method set. Wiki MediaWiki XML → source; MED
unzip; CLI leftover flags remain in `--help`. No `fallback_eval`. HEAPWISE /
PARSEWISE / ID `assert_eq` the Java pair lists. The P11 row stays
`parity_gap` (Java `BundleTest` encodings; Swing aligner UI).

## P12 notes

41 locale JSON files share the `en.json` keyset. Honesty leftover count is
0 (values still equal to English are only the brand `OmegaT`). Literal
`\\uXXXX` leftovers from the Bundle remapper are decoded. electron-builder
targets Linux deb/rpm/tar, Windows nsis, macOS dmg (unsigned; see
`PACKAGING.md`). Plugin ABI is `omegat_plugin_register` (`PLUGIN_ABI.md`).
Packaged manuals are `docs/manual/en.md` + `zh-CN.md` + Java HTML pointer
(not 41 languages). The P12 row stays `parity_gap` for that manual set
and unsigned packages. P6 remains `parity_gap` (22 missing affix pairs).

## Intentional non-goals (must still have a full replacement)

- Java JAR plugins are not loaded. Replacement: `omegat-plugin.toml` + cdylib.
- Groovy is not executed. Replacement: embedded Boa with the Java binding
  surface (`IEditor` / `IProject` / `IGlossary` / `console` / `mainWindow` /
  `Core`). `fallback_eval` is forbidden.
- LanguageTool is not an embedded JAR. Replacement: HTTP `v2/check`, with an
  `severity=info` downgrade item when no URL is configured.
