# Rewrite status

Legend:

- `scaffold` — present in the tree, **not** accepted
- `parity_gap` — specified remaining delta with **measured** numbers
- `parity` — accepted against **Java-exported** goldens (`assert_eq`) **and**
  the structural honesty gates for that row

P12: `tools/honesty/check.sh` is green and this table has **no `scaffold`
rows**. A row is `parity` only after that wave’s Java `*Test` method set is
exported and `assert_eq` green, plus the matching honesty item. **2026-08-18
adversarial audit:** this is not a finished OmegaT rewrite. Honesty-green
plus a `parity` cell is not class completion. See the measured gap below.

| Area | Wave | Status |
|---|---|---|
| Java reference tree at `reference/java` | G0 | parity |
| Honest STATUS + ACCEPTANCE (this file) | P0 | parity |
| Java Gradle exporter `exportGoldens` (honesty surfaces) | P0 | parity_gap |
| Structural honesty gates (`tools/honesty/check.sh`) | P0 | parity |
| Text / PO / HTML Java-exported goldens | G0 | parity |
| Filter / align / SRX fixtures under `fixtures/` | G0 | parity |
| Sidecar method contract tests | G0 | parity |
| RealProject / SRX / TMX / matching / stats / tags | P1 | parity_gap |
| filters2: 21 Filter classes; HTML = FilterVisitor | P2 | parity_gap |
| filters3: 23 Dialect tag sets + OpenDoc/OpenXML write-back | P3 | parity_gap |
| filters4: ZIP / XLIFF / SDL / Office node write-back | P4 | parity |
| Tokenizers: named Lucene Analyzer pipelines | P5 | parity_gap |
| Spell / dictionaries / LanguageTool | P6 | parity_gap |
| Editor: 63 `gui/editor` classes + Marker goldens | P7 | parity_gap |
| Desktop: 120 menus, 25 controllers, 9 docks | P8 | parity_gap |
| 7 MT engines, External Finder, autocompleter | P9 | parity_gap |
| team2: 23 classes; GIT via `git2` | P10 | parity_gap |
| Aligner, Boa `IEditor` surface, Wiki / MED / CLI | P11 | parity_gap |
| 41 locales, packages, plugin ABI, manual | P12 | parity_gap |

## Remaining measured gap

Adversarial audit **2026-08-27** (Java 6.2 tree vs this rewrite). Inventory:
`tools/honesty/missing_java_tests.txt`.

**Size (not a completion proof, a scale check):**

- Java `src/main/java`: **779** files / **157825** lines
- Rewrite Rust: `crates/**/*.rs` **52511** lines; `apps/desktop/src`
  TS/TSX/CSS **13183** lines (**~42%** of Java main lines, a scale check only)
- Java GUI: **297** files / **61510** lines vs desktop TS/TSX/CSS **13183**
- Java `gui/editor`: **63** files / **14288** lines vs TS editor **5023**
- Java `*Test` `public void test*` (`src/test` + `aligner/src/test`): **778**
- Unique `java_test` goldens that match those methods: **817** (includes
  API-less product-class fixtures)
- **In-scope missing goldens: 0.** Remaining **22** `missing` rows are
  the Java-runtime-only `EXCLUDED_TESTS` (JAR/LT smoke, plugin metadata,
  language-module Bundle, SVN plugin pack, Swing Styles/StaticUIUtils).
- `WAVE_REQUIRED_TESTS` registers **148** in-scope `*Test` classes across
  R1–R10. Unassigned in-scope classes: **0**.

**2026-08-27 verification:** core selected suites **144 passed**, filters
**62 passed**, team **30 passed / 1 ignored**, script **10 passed**, CLI
**4 passed**, sidecar contract **3 passed**, and desktop **18 files / 90
tests passed** after a clean TypeScript check. Structural honesty is **18/18**.
The real Linux unpacked package restart E2E also passes; Windows and macOS
packaged restart were not run in this Linux-only environment.

**P0 exporter / gates:** `exportGoldens` now writes one JSON per in-scope
`test*` (`util/` `search/` `engine/` `glossary/` `gui/` `mt/` `finder/`
`cli/` `team/` `remaining/`). R0 inventory is **148 classes / 0 missing
in-scope methods**. Structural gates stay green. A `parity` cell still
fails if that wave’s required `test*` set is incomplete. `SegmentEditor.tsx`
must reference `Document3` (unconditional). P12 leftover English phrases
(values equal a *different* `en.json` string) are **260** after Bundle
migration. `P12_GATES_GREEN` is gone. Product rows stay `parity_gap`
until R12 and the matching `assert_eq` set is complete.

**P1 engine:** Segmenter / FindMatches / Levenshtein / CalcMatchStatistics /
TMXWriter / TagValidation method goldens exist. Rewrite-wave goldens now
cover Searcher **34/34**, ProjectProperties **11/11**, TMXReader **9/9**,
SRX **7/7**, SRXManager **8/8**, RealProject import **3/3**. `assert_eq`
covers util + all **34/34** Searcher methods through the stateful project-search
product model + TMX L1 + Properties + import. The product searcher now also
traverses typed project-file / external-TM / glossary / text-file batches,
retains source-specific preambles, reports progress, and supports cooperative
entry-boundary cancellation without exposing incomplete results as complete.
`ExternalTMFactoryTest` (TMX
resegment / PO 1013 / Mozilla lang
33 / XLIFF 3 / fuzzy TUV) and `ProjectFileStorageTest` (defaults,
glossary paths, DTD entities, team XML, abs2rel) now `assert_eq` Java
method results. Remaining util goldens (`EntityUtil` / `MagicComment` /
`TagUtil` / `StaticUtils` / `EncodingDetector` / `Preferences` /
`MatchesTextArea.substituteNumbers`) also `assert_eq`. `Searcher.java` is
**1133** lines vs `search.rs` **1457** (size is not a completion claim): the
Rust path now retains UTF-16 match regions, source / target / note /
key-property hits, author/date filters, project/TM/orphan origins, duplicate
preambles, rerun lifecycle, and regex replacement groups. Remaining util
goldens now
`assert_eq` StringUtil / Language / BiDi / FileUtil / TMXDateParser /
TmxEscapingWriter / HttpConnectionUtils / Statistics / Token / Version /
PatternConsts / Merge / KnownException / Glossary CSV+TBX /
DictionaryData / MixedEol / ExternalFinder / CLI common params +
`constructCommandParams` / LegacyParameters initialize /
AlignSettings persist / CalcStandardStatistics PO table /
Latex `parseBracedCommand` / XML CJK path / Scripting #775.
`TmxSegmentationTest` project and external loaders now export and
`assert_eq` both resegmented source/translation pairs (2/2 methods).
`OStringsTest` **2/2**, deprecated `XMLStreamReaderTest` **2/2**,
`StatsResultTest` **1/1**, and `FindMatchesThreadTest` BUGS1248 now export
computed payloads and call dedicated Rust product APIs with strict equality.
`FileUtilTest` copy collision preflight/cancel, recursive listing, and
symlink-safe deletion replaced the last three API-name-only fixtures.

**P2 filters2:** `org.omegat.filters.*FilterTest` **150/150**.
`LineLengthLimitWriterTest` **10/10** goldens + `assert_eq` for
isSpaces / break-before / outLine / no-break word. FilterMaster /
PluginUtils / Latex unit goldens exist (plugin ABI replacement, not JAR
loader). HTML `FilterVisitor.java` **920** vs `filter_visitor.rs` **867**.
The Rust tokenizer now collapses arbitrary paired elements matched by
`ignoreTags` (including nested same-name elements), so protected subtree text
is neither extracted nor rewritten; exact identity and translated write-back
tests cover that traversal boundary. EOF-terminated comments, unclosed
script/style/ignoreTags elements, quoted tag delimiters, DOCTYPE internal
subsets, and UTF-16 LE/BE BOM inputs now have explicit product-path boundary
tests; incomplete protected subtrees stay intact after decoding instead of
leaking child text as segments. HTML write-back retains detected UTF-8,
UTF-16 LE/BE (including BOM), and declared legacy encodings instead of always
emitting UTF-8. Raw script/style closing and optional P/LI/DT/DD/table/option
implicit boundaries follow HTMLParser-style closure without swallowing later
segments. Incomplete start tags, declarations, and processing instructions are
now preserved as raw markup; recovery resumes at the next `<` boundary so one
broken tag cannot hide a later paragraph, including through the public
`HtmlFilter.parse` / `write` path.

**P3 filters3:** dialect tag snapshot exists.
`XMLFilterTest#testLoadCJKPath` golden is exported. OpenDoc/OpenXML now assign
part-qualified segment IDs (`content.xml#0`, `word/header1.xml#0`) during both
parse and write. **4/4** deep ZIP write-back tests strictly distinguish
same-source segments across parts, retain OpenXML protected nested tags, and
leave OpenDocument intact `office:styles` content unchanged. OpenDocument
attribute translation and out-of-turn note translation share the same
part-qualified ID stream; translated notes retain the original
`text:note-body` / paragraph structure around the translated span. Nested
note/annotation out-of-turn regions translate their own attributes, descendant
link/index attributes, and both text spans through one stable ID stream even
when the translation map is supplied out of order.
The deep write-back suite is now **9/9**: in addition to six ZIP cases,
nested XLIFF `sub` and DocBook `indexterm` regions preserve both nesting and
translated attributes/text under out-of-order translation maps. OpenXML
hidden field text, external relationship targets, and intact fallback content
now write independently under one options set. XLIFF nested callbacks receive
stable per-unit occurrence IDs, and translated `bpt`/`ept` shortcuts recover
their original content-based XML elements instead of becoming escaped text.
The XHTML product path now applies Java's case-insensitive, whole-entry
`skipRegExp` before allocating a segment ID; one combined write-back case
strictly separates skipped text/meta/intact subtrees from button, link,
language, and paragraph-on-`br` translations. A second option-matrix case
enables OpenDocument bookmark/sheet/link attributes while proving disabled
bookmark references, notes, comments, and presentation notes remain intact.

**P4 filters4:** `*FilterTest` **20/20**. SdlXliff / SdlProject still have
no Java `*Test` (fixture goldens only). `.docx` `for_path` still selects
filters3 `openxml`.

**P5 tokenizers:** `TokenizerTest` **7/7**. `BaseTokenizerTest` **6/6**
verbatim `assert_eq`. `DefaultTokenizerTest` contains / containsAll
`assert_eq`. `HunspellTokenizerTest` **3/3** goldens stay
`parity_gap` (language-module dic). Japanese word-break lexicon is **86**
entries, not Kuromoji / IPADIC — not parity.

**P6 spell dictionaries:** all **30** language-module stems have an
`.aff`/`.dic` pair reachable by `ensure_lang` (`reference/java` for
ca/es/fa/fr/ga/gl/pt/uk; `resources/languages/hunspell` for the rest).
**6** stems are Hunspell-format UTF-8 word lists, not upstream files
(ast, be, ja, km, tl, zh). **16** official wooorm / LanguageTool /
LibreOffice pairs are in-tree with `.dic` truncated to **2000** stems
(see `resources/languages/SOURCES.md`). CI affix logic still uses
`fixtures/spell/{hunspell,lucene,morfologik}`. Dictionary / LT Java tests
have ExportGoldens JSON + `assert_eq` for LingvoDSL article HTML,
StarDict idx/zip/pango, LanguageTool class mapping (rewrite bridge
is HTTP; Java default remains Native), SpellChecker dummy fallback,
and DictionariesManager ignore/`findWords`. P6 stays `parity_gap`
for the 6 lists + 2000-stem truncation.

**P7 editor:** **50/50** `gui.editor` Java `test*` goldens exist.
Product `SegmentEditor.tsx` **imports and calls `Document3`**
(`extractTranslation`) and routes mutations through `EditorTextArea3`'s
`Document3` path. Thickness is improved
but remains below Swing: `Document3` **288** vs **233**, `EditorTextArea3`
**521** vs **963**, `EditorController` **509** vs **2365**. The headless
product model now shares document mutations across the surface/controller,
enforces active bounds and atomic tags, tracks selection/caret/overtype/popups,
and implements filtered navigation/history/undo/loaded windows. Loaded windows
now expose stable-key multi-segment pages and the React renderer displays the
active segment in its surrounding lazy-expanded page; inactive segment
click/keyboard activation uses store navigation. Directional selection,
cut/copy/paste clamping, token deletion, Shift+Enter, tag double-click, focus
transitions, atomic caret motion, and hidden-textarea IME events call
`EditorTextArea3` rather than duplicating mutations in JSX. IME updates remain
one replaceable composition with commit/cancel. Variable-height prepends retain
a stable segment/viewport scroll anchor; Chromium caret range hit-testing maps
mouse pixels to UTF-16 offsets, and native `beforeinput` operations replace
printable-key synthesis while still flowing through `Document3`.
MarkerController caches
per-entry generations, maps translation/source marks into `Document3` spans,
and invalidates those spans after edits.
`FontFallbackMarker` uses canvas
`measureText` when a document exists. IEditor name table remains a
surface list, not a second editor.

**P8 desktop:** 120 menu ids. Save / compile tests `assert_eq` the
sidecar log `saved TMX …/omegat/project_save.tmx`, `compiled target`,
and `document3.dirty === false`. Walkthrough uses the same Document3
model. Java UI `*Test` goldens exist; Dialogs window ids and the 120
menu-action count `assert_eq` the ExportGoldens JSON (desktop maps
`glossary-new` → `glossary-add`). File-list progress helpers and
column-order numbers `assert_eq` the Java cases; Swing
`TableColumnModel` itself stays a measured UI-toolkit gap.
`EditorUtils.replaceGlossaryEntries` `assert_eq`s the Java snowman
replacements.
`PropertiesShortcutsTest` **6/6** now exports actual merged property,
Swing `KeyStroke`, recursive menu, and input-map results; the desktop
shortcut product path parses/merges/binds those values and native menu
accelerators pass through the same normalizer.
`ProjectUICommandsTest` **5/5**, `SimpleIssueTest` **5/5**,
`IssueCheckerTest` **3/3**, `GlossaryTextAreaTest` **3/3**, and
`NotesTextAreaTest` **2/2** now use toolkit-independent Rust/desktop product
models and strict Java-exported values. Packaged restart is assembled through
the actual main-process IPC registration: Electron's native no-argument
`app.relaunch()` preserves the original command line, then the handler stops
the sidecar and calls `app.exit(0)`. **3/3** lifecycle tests assert registration
and exact call order. The Linux E2E builds the release sidecar and real
`linux-unpacked` application, launches it under Xvfb, invokes the packaged
preload's `window.omegat.relaunch()`, and strictly verifies distinct old/new
browser and sidecar PIDs, preserved debug-port and unique-marker arguments,
and a ready renderer after restart. This is Linux package evidence only, not a
Windows or macOS E2E claim.

**P9 MT / finder / completer:** 7 engines use recorded HTTP fixtures (not
live protocol parity). `MachineTranslatorsManagerTest` **3/3** and
`ExternalFinderTest` **5/5** goldens exist. GlossarySearcher control
flow is ported. GlossarySearcher remaining methods (Italian
`GLOSSARY_FULL` `paesi`/`paese`, CJK/Korean, merge, tags, sort EN/JA)
`assert_eq` Java counts.

**P10 team:** GIT product path is `git2`. SVN checkout/update/commit is
**1 `#[ignore]`** (needs `svn` + `svnadmin` — reason stays). HTTP
two-client rebase uses `assert_eq` on conflict `ours`/`theirs`.
`RemoteRepositoryFactoryTest` detect-type **4/4** `assert_eq`.
`RemoteRepositoryProvider2Test` slash / abs-local helpers and HTTP
`file://` retrieve, 304 skip-write, `switchToVersion` (`null` ok /
non-null `"Not supported"`), and remaining copy/rename mapping
goldens `assert_eq` Java cases. Mapping glob evaluation now distinguishes
slash-anchored exclusions from recursive unanchored exclusions using
separator-aware matching. The two Java all-copy cases assert exact destination
sets (**5** unanchored / **9** slash-anchored), through the same observable copy
path used by sync. Git checkout/update/commit no longer swallow `git2` fetch,
missing-branch, commit, or push failures. No-change commits avoid pushes;
tracked deletion and observed-version guards are exercised without a product
`git` subprocess. Persistent HEAD checkpoints now make recently deleted paths
one-shot and mapping-aware; prepare/switch initialize and update submodules at
the recorded gitlink. Provider APIs expose file version, guarded commit, and
version switching through `IRemoteRepository2`. Multi-repository sync and
explicit project-file commits now prepare and stage all mappings before
publishing. A later repository failure restores the project/prep snapshots and
unpublished checkouts; already-published Git repositories receive a
fast-forward compensating commit with the pre-transaction tree instead of a
history rewrite. Prepare failure and second-repository commit failure have
deterministic product-path tests. Active state, phase history, rollback
versions, publication checkpoints, and project/file-remote snapshots persist
under `.repositories/transactions`; the next product operation recovers an
interrupted transaction before writing. A child test process terminates after
its first real Git publication but before its publication checkpoint; the
parent infers the remote tip, then verifies restart recovery and compensating
history. Two pre-observed clients create competing commits before either
publication; the actual libgit2 pushes produce one remote acceptance followed
by one non-fast-forward rejection.
An advisory exclusive `operation.lock` now serializes recovery, sync,
project-file commits, guarded commits, and version switches for the same
project across processes. A real child process holds the product lock while a
second sync receives an exact conflict without creating `active.json`; sync
continues after the holder exits. The suite is **30 passed / 1 ignored** (the
preserved SVN binary prerequisite).

**P11 aligner:** `AlignerTest` + prefs + Bundle **18/18** unit goldens
exist (HEAPWISE / PARSEWISE / ID). `AlignerWindowTest` merge/split/move
ops golden is exported. CLI Main / Legacy / CommandCommon goldens exist.
All **8/8** aligner-settings methods now call product persistence APIs with
strict values for enum keys, booleans, language fallback/round-trip, input
directories, and empty-filter fallback. The TMX write test parses the product
output back to exact source/target pairs instead of using substring checks.
The real CLI parser accepts the Java restart/common argument vectors, including
post-subcommand `--no-project-locking`, `--no-location-save`, `--no-team` /
`--team`, tokenizer overrides, alternate filename patterns, and empty
`--config-dir=` handling. Default/start now launches Electron instead of only
printing a hint: config-dir, project, locale and validated scripts directory
flow through Electron startup into the NDJSON sidecar, and the renderer opens
the requested project. Manual align edits select source, target, or both;
the edited rows are written through `align.write` and parsed back as exact TMX
pairs. Pending-bead `do_align` now invokes the configured alignment algorithm
instead of returning its input unchanged. The product state now retains
`MutableBead` score, nullable source/target line lists, enabled flag, and
accepted/needs-review status across split, pinpoint, bulk keep, and
realign-pending RPCs; the renderer edits and writes that state rather than
flattened pairs. Source/target selection now addresses contiguous visual row
spans across bead boundaries. Merge, move, replace, and pinpoint mutate the
exact selected line range, reset touched review state, and preserve the
shorter-side empty cells; React renders those rows and sends the row bounds to
the sidecar. The focusable table now routes arrow/Home/End/Page navigation,
shift-extended selection, source/target column movement, and Java's unmodified
U/D/S/M/E/A/R/C/K/Space/Escape accelerators through a tested product keyboard
model. Mixed enabled selections toggle each bead once, and pinpoint completion
requires a different row and column as in `AlignPanelController`. The sidecar
now also accepts arbitrary-row drag moves. React native drag/drop uses Java
`AlignTransferHandler.canImport` semantics: one matching source/target column,
non-null cells only, edge-line movement, a target outside the selected span,
and a different target bead. The Rust mutation preserves Java's directional
insertion order and clears review state on every touched bead. The sidecar
returns the exact post-move anchor/focus rows (without duplicate-text
matching), React restores that directional selection, and explicit top/bottom
drop targets expose Java's new-bead boundary moves. The sidecar contract is
**3/3**, including strict multiline
split/review/span-merge/span-replace/pinpoint/drag output.
Status actions now advance to the next bead through an exact sidecar selection
response. The table uses a bounded real scroll viewport, derives PageUp/PageDown
distance from currently visible variable-height rows, minimally reveals the
selection lead after edits, supports reverse Shift extension by mouse, and
routes Swing's N/P/F/B row/column actions through the shared keyboard model.
Wiki / MED have ExportGoldens API fixtures where Java has no `*Test`.
`ScriptItemTest` **6/6** now exports actual Java inline/file text,
metadata, missing-file, and I/O results and `omegat-script` imports the
corresponding product API for strict equality. `ScriptingTest` script/property
catalog and `ScriptRunnerTest` engine/compile cases now call the Boa-backed
product path while recording Groovy as a `parity_gap`. The latest per-method
thin-fixture inventory moved from **28 remaining + 13 gui + 3 util**
`method`/`api`/`computes` rows to **0** (the `engine/filter_tests.json`
inventory schema still has an `api` field and is not a method placeholder).
This closes fixture thinness only; product rows remain `parity_gap`.

**P12 ship:** same-key leftover count is 0 (brand `OmegaT` may equal
English). Cross-key leftover English phrases: **260** (e.g.
`ar.notes=Notepad` vs `en.notes=Notes`). `en.md` follows the Java
DocBook directory; other locales stay “English long manual + short
translation”. Packages stay unsigned (`PACKAGING.md`). Full-table
`parity` is forbidden while P6 truncation / SVN ignore / leftover /
Japanese 86-word LEX remain.

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

`ensure_lang` now copies every language-module stem. Official pairs that
are too large for git keep the upstream `.aff` and the first 2000 `.dic`
stems. ast / be / ja / km / tl / zh remain Hunspell-format stem lists
(Java also has no in-tree aff/dic for those modules). CI still uses
`fixtures/spell` for the three-backend split. The P6 row stays
`parity_gap` for those 6 lists + the 2000-stem truncation.

## P7 notes

`gui/editor` 63 Java classes each have a TS file. `Document3` holds the
active translation range, dirty flag, tag atoms, styled spans, edit/trusted
flags, and chrome `translationStart`/`translationEnd`. `DocumentFilter3`
rejects edits outside the translation range unless trusted. `IEditor`
implements the exported method set (gap empty vs `ieditor_methods.json`).
Each Java marker/predictor/completer/`EditorUtils`/`DocumentFilter3`/
`SegmentExportImport`/`EditorController` `*Test` method has an
ExportGoldens-shaped JSON and a TS `assert_eq`, including
`MarkerColorFreshnessTest`, `CharTableModelTest`, `CollapsibleBarTest`,
and `EditorProjectReloadLeakTest`. `SegmentBuilder` builds the active
document with `insertString` (`TF_CUR_SEGMENT_START` chrome);
`EditorControllerTest` is headless-skipped in Java, so the LTR empty
`XXX` offset is the computed insertString value (4), not a hardcoded 31.
`DocumentFilter3.replace` takes a `FilterBypass`. Autocompleter views:
Glossary / Autotext / CharTable / HistoryCompleter / HistoryPredictor
(next-word) / Tag. `Document3.applyDocumentEdit` is now the common product
mutation path used by `SegmentEditor`, `EditorTextArea3`, and
`EditorController`; trusted chrome edits move live translation positions
without dirtying user text. The P7 row is `parity_gap` (the controller and
text-area remain substantially smaller than Java and browser/Electron behavior
is not Swing behavior).

## P8 notes

120 `*ActionPerformed` ids are wired to observable behavior (`project.edit`
opens the properties dialog, not the wizard; `project.team-new` opens the
team flow; `edit.pdf` inserts U+202C). 25 preference controllers have
pages; Java keys are typed `controller_keys` (save still drops `extra`).
`SegmentationCustomizer` is a rule table. Nine docks are splitters (Dict/MT
are not a pinned aside). `RepositoriesMappingController` UI exists.
`className="placeholder"` is gone. Honesty menu / placeholder items are
green. `walkthrough.test.ts` `assert_eq`s TMX / compile / Document3 dirty for
new → translate 3 (tags kept) → save → compile → replace → mark prefs
still applied after `applyPrefs`. The P8 row is `parity_gap` (Java GUI
`*Test` goldens exist; Dialogs + 120-action count `assert_eq`).

## P9 notes

Seven MT connectors use recorded HTTP under `fixtures/mt/<engine>/`.
Offline without a fixture fails and does not block the editor. External
Finder GUI edits XML and `finder.run` opens the URL. Five completer views
are keyboard-insertable. Recorded fixtures `assert_eq` the Java parse
shapes; offline without a fixture is an error.
`GlossaryAutoCompleterViewTest#testSuggestions` is an ExportGoldens JSON
and `assert_eq`s payloads (including capitalization). The P9 row is
`parity_gap` (recorded HTTP only; GlossarySearcher remaining methods
now `assert_eq`; TransTips mark offsets `assert_eq`; MT manager /
ExternalFinder goldens exist).

## P10 notes

`GITRemoteRepository2` product path is `git2` (clone/fetch/reset/commit/push
+ credential callback). Fetch/reset/commit/push errors propagate, an unchanged
index is a no-op, and tracked deletion plus version guarding are tested with
`git2`-created repositories. `Command::new("git")` remains only in `lib.rs`
tests that seed other two-client fixtures. Mapping include/exclude UI is
`RepositoriesMappingController`. TMX and glossary rebase plus Keep
ours/theirs/manual stay. HTTP two-client rebase `assert_eq`s conflict
`ours`/`theirs`. SVN product path is the `svn` binary and the
checkout/update/commit test is `#[ignore]`. Honesty git-command item is
green. The P10 row stays `parity_gap` (SVN 1 ignore + reason; factory detect
4/4; slash/HTTP/exclude `assert_eq`; SVN 1 ignore remains).

## P11 notes

Aligner: HEAPWISE / PARSEWISE / ID; Viterbi ≠ Forward-Backward; CHAR/WORD
and Poisson vs Normal. Goldens are the Java pair lists (heap pair 3 is
the long EN sentence merged with “Where shall it end?”). Boa `editor`
bindings cover the IEditor method set. Wiki MediaWiki XML → source; MED
unzip; CLI leftover flags remain in `--help`. No `fallback_eval`. HEAPWISE /
PARSEWISE / ID `assert_eq` the Java pair lists.
`BundleTest#testBundleEncodings` `assert_eq`s US-ASCII / Windows-1252
(not UTF-8) and forbids U+202E. The Electron aligner window wires
column-aware merge / split / up / down through `align.edit`, then persists
the corrected rows through `align.write`; the RPC test parses the final TMX
back to exact pairs. CLI `start` resolves Java-style config-file locale and
`scripts_dir`, launches Electron, passes config/project/scripts context to the
sidecar, and auto-opens the project. The P11 row is
`parity_gap` (unit goldens exist; `AlignerWindowTest` ops + CLI goldens
are exported; Wiki/MED have API fixtures).

## P12 notes

41 locale JSON files share the `en.json` keyset. Honesty leftover count is
0 (values still equal to English are only the brand `OmegaT`). Literal
`\\uXXXX` leftovers from the Bundle remapper are decoded. electron-builder
targets Linux deb/rpm/tar, Windows nsis, macOS dmg (unsigned; see
`PACKAGING.md`). Plugin ABI is `omegat_plugin_register` (`PLUGIN_ABI.md`).
Packaged manuals are one markdown file per UI locale under `docs/manual/`
plus `java-html.md`. `ar.recent` and other leftover English menu phrases
are taken from `Bundle_*.properties`. Packages stay unsigned
(`PACKAGING.md`). The P12 row is `parity_gap` (260 leftover English phrases that equal a
*different* `en.json` string; `en.md` is the long DocBook-mapped manual;
other locales are short translations; unsigned packages).

## Intentional non-goals (must still have a full replacement)

- Java JAR plugins are not loaded. Replacement: `omegat-plugin.toml` + cdylib.
- Groovy is not executed. Replacement: embedded Boa with the Java binding
  surface (`IEditor` / `IProject` / `IGlossary` / `console` / `mainWindow` /
  `Core`). `fallback_eval` is forbidden.
- LanguageTool is not an embedded JAR. Replacement: HTTP `v2/check`, with an
  `severity=info` downgrade item when no URL is configured.
