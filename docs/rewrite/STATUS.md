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
- Rewrite Rust: `crates/**/*.rs` **59846** lines; `apps/desktop/src`
  TS/TSX/CSS **22921** lines (**~52%** of Java main lines, a scale check only)
- Java GUI: **297** files / **61510** lines vs desktop TS/TSX/CSS **22869**
- Java `gui/editor`: **63** files / **14288** lines vs TS editor **9164**
- Java `*Test` `public void test*` (`src/test` + `aligner/src/test`): **778**
- Unique `java_test` goldens that match those methods: **817** (includes
  API-less product-class fixtures)
- **In-scope missing goldens: 0.** Remaining **22** `missing` rows are
  the Java-runtime-only `EXCLUDED_TESTS` (JAR/LT smoke, plugin metadata,
  language-module Bundle, SVN plugin pack, Swing Styles/StaticUIUtils).
- `WAVE_REQUIRED_TESTS` registers **148** in-scope `*Test` classes across
  R1–R10. Unassigned in-scope classes: **0**.

**2026-08-27 verification:** core selected suites **148 passed**, filters
**86 passed**, team **30 passed / 1 ignored**, script **10 passed**, CLI
**4 passed**, plugin registry **4 passed**, sidecar contract **12 passed** plus
sidecar watcher unit **2 passed**, native plugin RPC/fault isolation **1
passed**, and desktop **23 files / 160
tests passed** after a clean TypeScript check.
Structural honesty is **18/18**.
The real Linux unpacked package restart E2E also passes; Windows and macOS
packaged restart were not run in this Linux-only environment. A separate real
Linux packaged aligner E2E now passes with XTEST pointer input through the
native application menu, renderer drag events, stationary edge autoscroll, and
the sidecar-backed drop result.
The real Linux unpacked native Marker E2E builds the release sidecar and
example cdylib, loads that plugin through the packaged Electron application,
renders its exact tooltip, deliberately aborts one callback worker, then
strictly verifies that the same sidecar and renderer remain responsive and a
later Marker callback still succeeds. The same packaged path now starts a
delayed native callback for an inactive page-edge entry, pages that entry out,
waits for every old worker to return, and proves the entry has no cached mark
when it re-enters the page; only a newly started callback may publish. It also
uses native F5 after adding a lexically earlier source file: the active entry
moves from **#1 to #2** while its complete EntryKey, exact translation, and
UTF-16 caret **16** remain unchanged. Enabling “Untranslated only” through the
packaged Preferences UI rebuilds the actual renderer page, removes translated
entries **#2/#3**, and selects empty entry **#4**. This is Linux evidence only;
Windows and macOS input/package behavior was not run.

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
parse and write. **7/7** deep ZIP write-back tests strictly distinguish
same-source segments across parts, retain OpenXML protected nested tags, and
leave OpenDocument intact `office:styles` content unchanged. OpenDocument
attribute translation and out-of-turn note translation share the same
part-qualified ID stream; translated notes retain the original
`text:note-body` / paragraph structure around the translated span. Nested
note/annotation out-of-turn regions translate their own attributes, descendant
link/index attributes, and both text spans through one stable ID stream even
when the translation map is supplied out of order.
The deep write-back suites are now **31/31**: in addition to seven ZIP cases,
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
Two further nested cases exercise the same public filter parse/write API:
double-nested XLIFF `sub` regions retain depth-first occurrence IDs alongside
`bpt`/`ept`/`ph` shortcuts, while XHTML keeps an ignored nested subtree and
its attributes byte-for-byte independent from translated outer/inline
attributes and paragraph-on-`br` text. The two latest product-path cases keep
duplicate XLIFF `trans-unit` IDs and their protected callback streams
independent, and sort physically reversed OpenXML parts by natural part order
while writing hidden/visible nested callbacks only through part-qualified IDs.
The Android product path now also preserves comment-based “do not translate”
and `translatable=false` resources while writing named string/plural IDs,
protected inline tags, apostrophe escaping, and an explicitly empty plural
translation independently; the test checks the rewritten XML structure and
reparsed segment set with exact equality.
Three further public `FilterRegistry.for_path` cases cover standalone dialect
write-back with strict structure and reparse equality. ResX and WiX duplicate
source strings now prefer their distinct named IDs over a conflicting
source-key fallback. ResX also keeps decoded `>` names, `FieldName`, `type`,
and `mimetype` data intact. TXML writes duplicate targets by occurrence ID,
restores each protected `ut`, keeps `source`/`skeleton`/`revisions` intact,
and accepts an explicitly empty target translation through the common XML
product path. Fourteen further `FilterRegistry.for_path` cases give every one
of the **23/23** filters3 dialects a strict parse/write/reparse product-path
case. Properties XML, Schematron, RELAX NG, SVG, Camtasia, Scribus, VDX Visio,
XML Spreadsheet, Flash, WordPress, Help & Manual, Typo3, L10nmgr, and Infix
write translated occurrences while keeping their numeric, metadata, geometry,
schema-value, `translate=false`, and dialect-specific intact regions exact.
Flash and WordPress now use their Java namespace read-ahead checks during
public `.xml` registry selection, and translated XML Spreadsheet/Flash/
WordPress text keeps the source `XMLText` CDATA mode during write-back.
OpenDocument and OpenXML public filters now carry cancellation through ZIP
entry reads, XML character/event loops, part callbacks, and write-back.
Cancelled Office output is discarded from a sibling temporary file instead of
replacing the destination; the exact product test keeps the original archive
unchanged.

**P4 filters4:** `*FilterTest` **20/20**. SdlXliff / SdlProject still have
no Java `*Test` (fixture goldens only). `.docx` `for_path` still selects
filters3 `openxml`. The filters4 abstract ZIP/XML and MS Office product paths
also check cancellation inside entry I/O and StAX traversal, with atomic final
archive replacement.

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
**720** vs **963**, `EditorController` **944** plus extracted
`EditorDocumentLifecycle` **141**, `HeadlessMarkerLifecycle` **274**, and
`EditorNavigation` **102** vs Java controller **2365**; its standalone headless
`HeadlessLoadedWindow` is **141** lines, and the mounted renderer's
independent `RendererPageProjection` is **248** lines. The headless
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
a stable segment/viewport scroll anchor. `EditorController` now exposes Java's
entry-relative caret/selection positions, selected text, and insertion at the
live selection instead of always appending; selection replacement, dirty state,
caret collapse, entry synchronization, and undo are strictly exercised through
its `EditorTextArea3` / `Document3` product path. Chromium caret range
hit-testing maps mouse pixels through model-aware fragments to UTF-16 offsets.
Those fragments retain separate rendered/model lengths, so expanded BiDi and
whitespace glyphs, glossary decoration, stacked Marker/spell classes, and
protected tags cannot shift a later caret; protected text and tags choose an
atomic side from the visual half. Native `beforeinput` operations replace
printable-key synthesis while still flowing through `Document3`.
The hidden textarea now subscribes directly to Chromium `beforeinput` and
composition events instead of relying on React's synthetic `onBeforeInput`.
Repeated native `compositionstart` events retain one replaceable
`EditorTextArea3` composition session, and both `insertFromComposition` and
Chromium's final `insertText` commit route through `Document3`. A real Linux
`linux-unpacked` E2E now uses XTEST `mousedown`/mouse motion/`mouseup` to
select exactly `alpha`; Chromium reports the exact trusted
`pointerdown`/`pointermove`/`pointerup` mouse sequence, and renderer pointer
capture maps both pixel hits through `EditorTextArea3`'s directional selection
over the active `Document3`. Chromium `Input.imeSetComposition` updates then
replace that selection and XTEST Tab commits a second active composition on
real focus loss. After refocusing with a real click, XTEST Escape cancels
`取消中` and restores the exact pre-composition text; a late native
`compositionend` is discarded instead of reinserting cancelled text. Enter
then persists the exact `日本語失焦 😀 beta` translation through the NDJSON
sidecar. Entering the
workspace in that E2E also exposed and fixed a React 19 infinite-update defect
by keeping the Multiple Translations Zustand snapshot stable. This is Linux
Electron evidence only, not Windows/macOS evidence.
The same real `linux-unpacked` workflow now enables whitespace, NBSP, BiDi, and
glossary decorations through the Preferences UI, then inserts one translation
containing an emoji, NBSP, LRM, glossary hit, and protected tag. Exact
Chromium DOM assertions keep rendered and UTF-16 model lengths separate,
including stacked product Marker classes. A native XTEST hover returns the
strict `<html>NBSP</html>` tooltip, and trusted XTEST
`pointerdown`/`pointermove`/`pointerup` over the decorated fragments selects
exactly `gamma`. This remains Linux packaged evidence only.
Controller navigation now adopts direct `EditorTextArea3` edits, commits an
active IME composition, synchronizes the old entry, and deactivates its
`Document3` before opening another segment. New entries start at relative
caret zero; `commitAndLeave` restores the prior relative caret. Next/previous,
translated/untranslated, noted, unique, xAUTO, and xENFORCED navigation wrap
across files through one filtered product path. Undo/redo snapshots retain the
relative caret or selection while marker spans are recalculated. The live
Zustand product path persists a changed draft/note through `entry.set` before
selection, history, or cyclic navigation; an optimistic-write failure leaves
the original dirty document active instead of discarding it.
Project reload now commits and saves the live document before asking the
sidecar to reload, then rebinds the active segment with the complete
Java-shaped `EntryKey` after reordering instead of trusting the old numeric
index. The headless controller preserves and clamps the translation-relative
caret or selection for that exact key, clears stale history/undo state, and
chooses a deterministic next visible entry (or a truly empty view) when a
reload or rebuilt filter removes the active segment. Exact Zustand and
controller tests cover commit-before-reload, reordered same-source entries,
caret clamping, filter retention, and empty-filter recovery.
`EditorDocumentLifecycle` now owns the sole headless `Document3`, its
`EditorTextArea3`, activation presentation order, live IME adoption,
deactivation, relative caret/selection, and protected-range binding.
`HeadlessMarkerLifecycle` owns the loaded-page key set, synchronous active
decoration, asynchronous publication fences, per-page cache retention, and
conversion of protected marks back into document ranges. `EditorController`
only orchestrates those two product modules with project/navigation state.
Desktop golden tests directly import both extracted APIs: all four exported
`EditorControllerTest` payloads remain strict-equal, and the lifecycle page
publishes the exact Java-exported NBSP source interval.
Complete `EntryKey` matching, source/default lookup, file lookup, filtered
cyclic traversal, and reload rebinding now live in the imported
`EditorNavigation` product module. Both the headless controller and mounted
Zustand renderer call that module instead of maintaining two navigation
algorithms; exact tests distinguish alternatives by prev/next/path and verify
deterministic reload fallback.
The controller no longer stores visible indices, loaded bounds, page radius,
marker-key membership, or loaded-page generation itself; those concerns live
in `HeadlessLoadedWindow`, with exact isolation/paging/generation tests.
External-fix refresh now rebuilds the active segment from the authoritative
entry without first committing a stale live draft; ordinary refresh retains
commit-before-rebuild behavior. Controller `changeCase` clamps roaming UTF-16
selections to the translation, expands a collapsed caret to its current word,
preserves OmegaT tags, and writes through `EditorTextArea3` / `Document3` with
undo selection retention. The exporter calls Java `EditorUtils.doChangeCase`
for **17** inputs across lower/upper/sentence/title/cycle plus a five-step
cycle sequence; strict desktop equality includes uncased CJK and the
`Ǉ`/`ǈ`/`ǉ` title-case distinction.
The renderer now owns its directional UTF-16 caret/selection in Zustand, and
the `IEditor` command surface inserts or replaces through an
`EditorTextArea3` over the active `Document3`. Matches, glossary, and machine
translation docks import and call that product surface instead of appending to
the draft independently. `commitAndDeactivate` no longer advances to another
entry, `commitAndLeave` preserves the active entry and relative selection, and
window deactivation clears the completer rather than spuriously saving the
project. The IEditor method table is compared by exact equality, not a minimum
method count. The Linux XTEST/Chromium IME and decorated-pointer packaged path
remains green with this shared selection state.
The renderer store no longer keeps a second `draft` string alongside
`Document3.translation`: native input, `IEditor`, menu actions, Finder, and all
inserting docks read or publish the same document snapshot. `IEditor` also no
longer keeps module-global selected-text or filter mirrors; source selection
and the serializable editor filter live in the store. The mounted renderer path
no longer imports or instantiates `EditorController`.
`RendererPageProjection` derives one immutable page plus request-scoped Marker
inputs from Zustand's index and sole `Document3`; it owns only page bounds,
Marker jobs/cache, tooltips, and scroll anchors. The native Marker bridge now
targets that narrow host, while file-drop behavior is a standalone product
boundary. Exact tests verify the headless controller retains no renderer active
segment or selection and late inactive page-edge callbacks cannot repopulate
the mounted projection.
`EditorController.replacePartOfText` now treats start/end as translation-
relative UTF-16 offsets, selects through `EditorTextArea3`, mutates the active
`Document3`, synchronizes the entry, recalculates markers, and restores the
original relative selection on undo. Arbitrary `ProtectedPart` intervals now
snap hit-tested carets, expand selection/replacement/IME ranges atomically,
delete as one unit, and include the same adjacent BiDi controls as Java's
double-click selection. Marker hit-testing collects all ranges overlapping an
atomic rendered fragment, keeps source/translation and active/context entry
keys isolated, emits Java-shaped HTML, and displays the same tooltip on source,
active target, and non-active segments. Electron file drops pass real native
paths through preload/main-process inspection: an `omegat.project`
opens its project root, while ordinary files import only into an already-open
project. Successful leave commits invoke the sidecar issue product path,
retain issues scoped to the old file, and reveal the issue window without
discarding a successful commit if checking fails. A real Linux
`linux-unpacked` E2E sends Chromium file drags carrying actual filesystem paths:
it opens a dropped `omegat.project`, imports an ordinary file through
`project.import`, commits a missing-tag translation, verifies navigation to the
next file plus the file-scoped `Tag MISSING` dialog, and clicks that issue back
to its original entry. This is Linux packaged/CDP drag evidence, not a
Windows/macOS or external-file-manager XTEST claim. Desktop verification is now
**23 files / 159 tests**, including exact success and
failure-state assertions for these transitions. Default commits now update the
source-wide translation atomically in `ProjectSession`, return every affected
entry over NDJSON, and refresh repeated occurrences in both the Zustand and
headless controller paths. Renderer writes require the full Java-shaped
`EntryKey` (`file` / source text / `id` / `prev` / `next` / `path`), reject a
stale index whose key changed, and preserve empty boundary context separately
from null. Project TMX save/load resolves alternatives with the complete key;
an exact product test keeps two alternatives with the same file, id, and source
independent solely through prev/next/path. Converting an alternative back to
default removes only that complete-key override and propagates the new default
without overwriting other alternatives. Optimistic
revision failures preserve the dirty `Document3`, expose base/ours/theirs in
the active editor, and resolve through exact ours/theirs/manual product paths
using the live remote revision. The conflict snapshot now retains the complete
six-field `EntryKey`; resolution re-fetches the authoritative entry list and
rebinds that key before adopting or writing. An exact product-store test
reorders two same-source duplicates and proves an ours resolution writes only
the intended alternative at its new index.
`MarkerController` caches per-entry generations, maps translation/source marks
into `Document3` spans, and invalidates those spans after edits. It now also
registers and unloads named plugin markers, rejects duplicate registrations,
and supports synchronous or asynchronous providers. Per-entry/per-marker
request tokens are invalidated by edits, `remarkOneMarker`, and unload, so an
expired callback cannot replace current ranges. `SpellCheckerMarker` calls the
real sidecar `spell.check` path, maps its UTF-16 token offsets to translation
spans, and learn/ignore each trigger one spell-only recomputation; exact tests
discard both an older-document callback and a pre-remark callback.
Asynchronous providers now run for every active and inactive entry in the
rendered page. Page-generation checks plus key-based cache retention expire all
in-flight results when filtering, reloading, editing, or lazy page contraction
removes an entry; an exact deferred-provider test proves callbacks from both
inactive page edges cannot repopulate cache or `Document3` spans.
The append-only cdylib host ABI now accepts executable Marker registrations.
`markers.list` discovers them through the sidecar and `markers.query` sends
the complete EntryKey plus editor context into the native callback. The
renderer registers each callback as an asynchronous provider, maps strict
UTF-16 output into `Document3`, unloads it with the React lifecycle, and
discards StrictMode-era discovery responses. The real example cdylib is built
and loaded in tests; exact sidecar and renderer assertions cover emoji
offsets, plugin metadata, tooltip/color, complete EntryKey, and unload. Native
Marker callbacks now run in five-second, short-lived sidecar worker processes;
an abort, signal exit, timeout, or malformed worker result becomes an error for
that `markers.query` only. Unit/RPC tests and the real Linux packaged Electron
E2E prove the long-running sidecar and renderer survive an actual callback
`abort` and can execute a subsequent callback. Electron now routes each native
Marker query through a dedicated short-lived sidecar, so a slow plugin worker
cannot serialize project navigation on the stateful sidecar. The packaged E2E
uses that product route to remove an inactive entry while its native callback
is still running, rejects all late results by page generation, and requires a
fresh callback before the entry can display a mark after returning.
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
models and strict Java-exported values. Eight Swing-facing docks now call the
shared **507-line** desktop controller path instead of embedding all behavior
in JSX. Exact product tests cover score-sorted fuzzy selection before
insert/overwrite, glossary insertion at the active `Document3` selection,
per-entry note undo/redo, priority-ordered comment providers, complete-key
multiple-translation navigation and default promotion, engine-sorted cyclic MT
selection, exact/stemmed dictionary focus, and structured segment-property
notification rows. Source-only editor navigation now rejects alternatives,
while alternative navigation compares all six Java `EntryKey` fields; target
paths preserve POSIX or Windows separators. These additions are desktop model
evidence, not a claim of Swing toolkit or Windows/macOS package parity.
Matches, glossary, notes, comments, multiple translations, MT, dictionary, and
segment properties now expose real pane popup actions through a shared
keyboard-dismissable/context-menu host. Hit/no-hit notification decisions are
centralized and visibly surfaced by pane headers. Segment selection and manual
MT/dictionary/completer loads use abortable latest-request controllers: an
older result cannot publish after a newer selection, and a cancelled multi-step
load stops before issuing its next sidecar query. Exact tests prove
stale-result rejection, notification dispatch, disabled popup actions, and the
store-level two-selection race.
Project LOAD/CREATE/CLOSE-style boundaries and entry activation now share
project plus entry generations across every asynchronous dock loader. Opening,
closing, reloading, or selecting immediately cancels and clears old pane work;
manual MT, dictionary, and completer publication validates both the project
and complete EntryKey, so an old result cannot cross into another project at
the same numeric index or even the same key. Project open also installs the
new first entry as a clean `Document3` before selection, preventing the old
draft from being misclassified and written into the newly opened project.
Those boundaries are now explicit events on one subscribable renderer bus:
LOAD, CREATE, CLOSE, RELOAD, ENTRY, and EXTERNAL_REFRESH publish before their
asynchronous work. External mutations from replace/script paths fetch the
authoritative entry list without committing the stale live document, rebind by
all six `EntryKey` fields, clamp the retained selection, and only then activate
new Dock loaders. Exact tests assert event order, changed-key payloads, no
`entry.set` during an external fix, and same-key cross-project cancellation.
Dock aborts now cross Electron IPC as request IDs and NDJSON
`$/cancelRequest` notifications. The sidecar reads cancellation concurrently
with request workers, returns exact `-32800`, stops search/dictionary/filter
publication cooperatively, and terminates in-flight MT and LanguageTool curl
processes. `issues.list` passes the same token through its per-entry checks and
LanguageTool calls; large text-filter reads check it per 64 KiB and every
filter parse/write boundary suppresses cancelled output. Exact contract tests
cancel direct LanguageTool, aggregated issues, and filter parse requests, then
prove the stateful sidecar remains responsive.
The same token now reaches RealProject source reload/compile/export loops,
deep XML/ZIP/Office parsing and atomic write-back, team transactions, and
aligner extraction/decoding. Sidecar `project.reload`, `project.compile`,
`team.sync`, `team.commit`, and `align.run` map cooperative cancellation to
the exact protocol error `-32800`. Request-scoped `$/progress` checkpoints now
make middle-of-operation protocol cancellation reproducible rather than a
pre-start race. The **12/12** sidecar contract waits until real product work has
started, then strictly checks `-32800`: reload restores the prior exact entry
list, compile leaves the complete prior target tree unchanged, team sync/commit
restore exact project and file-remote snapshots and remove `active.json`, and
align preserves the prior destination bytes. Compile stages every target and
TM export privately before one rollback-capable publish phase; align writes
through a cancellable sibling stage. Team mapping copies check cancellation per
file inside the existing transaction, and reload commits only its candidate
entry set.
Electron now carries those long operations through an explicit request id and
progress token. The main-process RPC client emits started/progress/cancelling/
cancelled/succeeded/failed events, preload forwards them, and Zustand exposes
the exact operation and stage to the status bar plus cancel controls in the
main, team, and aligner views. Reload, compile, team sync/commit, and align run
all use this path. Cancellation rejects the matching request before stale
success can publish; compile skips its post-operation stats/log, team skips
conflict/refresh publication, reload reactivates the rolled-back project
entry, and align keeps the prior bead model. Exact desktop tests cover the
NDJSON lifecycle, `$/progress` stage, renderer IPC state, and cancel outcome.
The real Linux `linux-unpacked` compile-cancellation E2E opens a **2400-file**
project, starts compile from the visible toolbar, waits for the exact
`project.compile.targets` status-bar stage, and cancels from the visible
control. The main-process client now remains `cancelling` until the sidecar
acknowledges the cooperative token with protocol error **-32800**, after which
the renderer shows `cancelled` with the retained stage. The E2E strictly
observes started/progress/cancelling/cancelled, proves the complete preexisting
target tree is byte-identical, finds no compile staging residue, and verifies
the same stateful sidecar still reports version **6.2.0** and all **2400**
entries. The same packaged process then invokes reload through native F5,
clicks the visible cancel control immediately after
`project.reload.sources`, and observes a second independent
started/progress/cancelling/cancelled trace. Only the sidecar's protocol
**-32800** acknowledgement makes reload terminal; the active complete
`EntryKey`, source, translation, and all **2400** sidecar entries remain exact,
and the sidecar still answers `sys.version`. This is Linux package evidence
only.
The same real Linux package now opens the visible Team window and cancels both
`team.sync` and source `team.commit` from the shared status-bar control in a
two-repository transaction: one root/main file remote plus one `/source/`
mapping file remote over **2400** project files. Both paths visibly retain
`cancelling` until the stateful sidecar returns protocol **-32800**, then enter
`cancelled` with the retained `team.mapping.copy` stage. Exact base64 file-tree
snapshots prove the project product tree and both remotes are byte-identical
after rollback; `.repositories/transactions/active.json` is gone. The dirty
active `Document3` retains its complete six-field `EntryKey`, exact translation,
and UTF-16 caret **26**, while the same sidecar still returns version **6.2.0**
and all **2400** entries. Late mapping progress after the cancel request is
discarded, and an operation that wins the race with cancellation is no longer
misreported as cancelled. This is Linux package evidence only.
Native filesystem watchers cover project/source/TM/glossary/dictionary inputs
on Linux without relying on recursive-watch support. They now install and
remove per-directory watchers as nested directories appear or disappear at
runtime. Independently, the sidecar scans its active project inputs and emits
`project.files-changed` NDJSON notifications for newly created, modified, or
deleted files; Electron merges those with native events in one debounce set.
Those events and successful team sync/conflict resolution call the sidecar's
`project.external-refresh`, which reloads project properties, project/external
TM, glossary, and sources before the existing EXTERNAL_REFRESH bus rebinds the
sole `Document3` and starts new Dock work. Sidecar contract tests exercise both
wire cancellation/responsiveness, proactive runtime-directory events, and
on-disk source/glossary adoption.
Reload, native/sidecar file changes, direct external refresh, and team
ours/theirs resolution now publish through one renderer rebind transaction.
Only a complete authoritative entry list plus statistics may replace the live
snapshot; the active entry, conflict rows, sole `Document3`, and selection are
rebound by all six `EntryKey` fields. A cancelled external refresh atomically
restores project properties, TM, glossary, and entries and publishes no
candidate list. External refresh is filesystem-read-only, avoiding recursive
watch notifications from directory creation.
Each forwarded proactive event now carries the renderer project generation and
its native/sidecar source set and per-path fingerprint. A queued event from an
older same-root project generation is rejected before refresh; a delayed second
channel carrying the same fingerprint is folded into the first renderer
transaction. Distinct fingerprints now enter one generation-scoped FIFO:
only after the current six-field transaction succeeds or receives protocol
**-32800** does the next batch start, and a generation change drops queued old
batches instead of publishing them into the new project. Sidecar writes bracket
the Rust scanner with begin/end snapshots, while Electron
suppresses matching native watcher echoes for the same write-source operation.
Electron also fingerprints project inputs around nested writes, so delayed
native `fs.watch` delivery is suppressed only while it still matches the
completed self-write. Exact real-filesystem desktop and actual sidecar
`project.save` tests prove saving does not feed back as an external mutation and
later real changes still publish.
The real Linux packaged cancellation run includes two YAML entries with the
same source but different file/id/path keys, then adds a lexically earlier file.
While the first merged native+sidecar refresh is visibly in source progress,
the run writes a second fingerprint and then cancels the first request. The
first request reaches started/progress/cancelling and becomes cancelled only
with protocol **-32800**, still exposing the **2400**-entry snapshot with the
exact wanted key, translation, and UTF-16 caret **29** while the decoy remains
untranslated. The queued fingerprint uses a distinct request id whose started
event occurs strictly after that cancelled terminal event; it succeeds
automatically, grows the project to **2401** entries, moves the wanted segment
from **#1001 to #1002**, and preserves the same key/translation/caret. This is
Linux package evidence only.
Team TMX rebase now identities occurrence-specific alternatives by all six
`EntryKey` fields rather than source text. Conflict persistence carries that
key through the visible ours/theirs row and the sidecar resolution call, and
the resolver updates only the matching TMX occurrence. The Linux packaged run
also imports a lexically earlier source through a real file team repository,
creates an ours/theirs conflict for the wanted duplicate, and selects the
visible keep-theirs control; the wanted segment remains active at its reordered
index with its UTF-16 caret while the same-source decoy stays untranslated and
the conflict list clears through the shared refresh transaction.
Packaged restart is assembled through
the actual main-process IPC registration: Electron's native no-argument
`app.relaunch()` preserves the original command line, then the handler stops
the sidecar and calls `app.exit(0)`. **3/3** lifecycle tests assert registration
and exact call order. The Linux E2E builds the release sidecar and real
`linux-unpacked` application, launches it under Xvfb, invokes the packaged
preload's `window.omegat.relaunch()`, and strictly verifies distinct old/new
browser and sidecar PIDs, preserved debug-port and unique-marker arguments,
and a ready renderer after restart. This is Linux package evidence only, not a
Windows or macOS E2E claim.
The Linux packaged editor/drop E2E now also blocks the first project's real
`mt.query` on a FIFO-backed recorded HTTP response, publishes the second
project's LOAD boundary while that request is pending, and switches through a
real dropped `omegat.project`. Releasing the old response cannot populate the
new project's MT Dock. The same packaged process then imports a real file,
commits a missing-tag translation, opens its file-scoped issue, and navigates
back to the exact entry. This is Linux Electron/CDP file-path evidence only.

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
Sync and project-file commit now check the shared cancellation token before and
between prepare, mapped copy/delete propagation, rebase, and publication
phases. Cancellation exits through `TeamError::Cancelled`, preserving the
transaction rollback path instead of publishing the remaining repositories.
Conflict resolution now uses the same persisted project transaction boundary
and cooperative token. `team.resolve` reports snapshot/write-back/queue
checkpoints, rolls TMX, prep state, conflict ordering, and the untouched
project tree back together, and returns protocol `-32800`; Electron therefore
keeps `cancelling` until the sidecar acknowledgement before publishing
`cancelled`. Two simultaneous non-default TMX conflicts with identical source
text are addressed only by their complete six-field `EntryKey`: cancelling one
retains both, then keep-theirs on the first and keep-ours on the second advance
the visible queue one item at a time. A persisted `capturing` or `mutating`
resolution journal is recovered before `project.open`, and the renderer reloads
only that opened project generation's persisted conflict queue. The Linux
packaged product path kills the sidecar during the real snapshot checkpoint,
restarts Electron, and verifies same-project queue recovery before completing
both visible resolutions. The P10 row remains `parity_gap`.

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
Native row dragging now continuously autoscrolls that real viewport through
`requestAnimationFrame`; speed is clamped to the remaining scroll extent, and
the nearest visible row becomes the drop focus until explicit before-first or
after-last boundaries are reached. The focusable table exposes the exact
row/column or boundary through `aria-activedescendant`, visually marks only a
Java-eligible drop target, and restores keyboard focus after drop or editor
Escape. The Linux `linux-unpacked` E2E opens the aligner from Electron's native
Tools menu, uses XTEST `mousedown`/mouse motion/`mouseup` (not CDP drag
injection), observes native `dragstart`/`dragover`, holds at the visible
viewport edge until rAF reaches `align-drop-bottom-source`, and strictly checks
the sidecar result: moving source row 0 to the bottom creates **81** visual rows
with the original target-only first bead and a source-only final bead.
`align.run` now passes the protocol cancellation token through extraction,
unit pairing, HMM construction, Viterbi/forward-backward decoding, and the
pre-write boundary, returning `-32800` without publishing a partial result.
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
`PACKAGING.md`). Plugin ABI is `omegat_plugin_register` (`PLUGIN_ABI.md`);
its executable Marker callback is loaded by the sidecar and registered in the
React editor, not merely listed in a manifest. A real `linux-unpacked`
application now proves packaged native Marker loading, tooltip rendering, and
callback-crash isolation; this is not a Windows/macOS package claim.
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
