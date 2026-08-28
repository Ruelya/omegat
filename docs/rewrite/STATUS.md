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

**2026-08-28 verification:** core selected suites **160 passed**, filters
**86 passed**, team **50 passed / 1 ignored**, script **10 passed**, CLI
**4 passed**, sidecar contract **36 passed** plus sidecar journal/watcher unit
**12 passed** and plugin filter **1 passed**, and desktop **25 files / 182 tests
passed** after a clean TypeScript check.
Structural honesty is **18/18**.
Project/team product transaction history now uses the shared immutable segmented
store: dual hot/manifest replicas, hash-prefix sparse lookup, bounded
`history.ndjson` recent projection, generational replacement, and post-manifest
directory-fsynced GC replace the former unbounded append stream. Unit coverage
fails closed on missing segments and equal-revision manifest/index conflicts,
recovers multiple orphan generations, resumes interrupted legacy import, and
rebases mutable state after moving a project without rewriting immutable segment
bytes. The complete Linux unpacked-package driver passed with a two-row recent
limit, consecutive process deaths after generation publication and after the
first GC unlink, live dual-Electron owner rejection, lost-receipt adoption plus
idempotent acknowledgement, five byte-identical immutable segments across a
project-directory move, and zero remaining orphans. Windows and macOS file-lock,
atomic-rename, directory-fsync, and packaged equivalents were not run because
this runner is Linux-only.
Shared-config terminal/dedupe history now uses that same
`omegat-core::segmented_history` implementation instead of maintaining a second
segment, manifest, sparse-index, and GC stack in the sidecar. The version-3
config envelope partitions exact batch retries, preserves one global config
FIFO across projects, and restartably imports legacy v1/v2 hot dedupe,
manifest/archive, recent, and active replicas before removing old files.
`history.ndjson` is a bounded write-ahead projection, so a terminal published
before either hot replica is recoverable without replaying its product write.
The real Linux unpacked-package mixed matrix passed: five legacy plus four new
config rows (including a one-hex SHA-256 prefix collision), strict
close → team commit → save → refresh order for project A, isolated save order
for project B, three globally ordered config batches, two consecutive owner
deaths, deleted-root replacement, and six byte-identical immutable project
segments across a pending-receipt directory move. It also passed all ten
history/compaction kill points, all sixteen hot/manifest replica
write/fsync/rename/parent-fsync kill points, and fail-closed dual corruption of
both hot and both manifest replicas with no product mutation. Windows and macOS
packaged locking, rename, and directory-persistence evidence was not run on this
Linux-only runner.
Config and project/team active transaction state now use one generic
`omegat-core::durable_fifo` implementation for scoped dual replicas,
monotonic revisions, OS locks, and dual-replica renderer-owner election;
`omegat-core` remains independent of `omegat-team`. Both domains restartably
import their former active and owner shapes, repair a stale/corrupt peer, and
fail closed when equal revisions disagree. Per-root active ownership and stable
detached generations provide cross-root round-robin dispatch without dropping
refresh tails, while replacement waiters recover through consecutive dead
owners.
The real Linux packaged durable-FIFO stress runner passed both constituent
multi-Electron drivers. It covers enqueue; all eight active recovery/primary
write, fsync, rename, and parent-fsync boundaries; product publication; ten
history/compaction kill points; sixteen history replica boundaries; renderer
acknowledgement and compaction; cancellation before lock, after lock, and after
rollback; cross-root order; and consecutive owner deaths. Windows and macOS
packaged file-lock, atomic-rename, directory-fsync, and Electron concurrency
were not run because this runner is Linux-only.
The raw NDJSON contract and real Linux packaged matrix also pass the
three-owner-death cancellation row: the third pre-existing waiter reads the
already-published terminal, all four logical callers receive **-32800**, and
the resolve envelope count remains zero.
The sidecar contract and real Linux package both exercise pre-kill contender
rejection at each product compaction checkpoint; the packaged result records
`pendingRejected` and `acknowledgementRejected` before the old PID exits.
The real Linux unpacked package restart E2E, including atomic refresh receipt
recovery and cross-project transaction isolation, also passes; Windows and macOS
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
Fingerprint batches are now persisted before renderer delivery in a sidecar
journal beside team transactions, with a config-scoped active-project pointer.
The journal records the Electron instance, renderer generation, project root,
ordered batch IDs, paths, fingerprints, and native/sidecar origins. A replacement
sidecar reopens the watched root and republishes the same FIFO head; a replacement
Electron process may adopt only the formerly active same-root queue and re-stamps
its new generation. Same-process generation changes, different roots, and
completed heads are discarded. The renderer acknowledges a head only after its
six-field rebind transaction succeeds, is coalesced, or the sidecar returns
**-32800**; process-exit errors leave it pending. Sidecar contract coverage kills
and replaces the process twice while checking FIFO order and stale-root/
generation rejection. The Linux packaged long-operation run kills a sidecar
during an external refresh while two complete-key conflicts remain visible,
then verifies the replacement sidecar refreshes and rebinds both conflict rows.
It separately kills Electron and its sidecar during another persisted refresh,
relaunches the package, and verifies same-project recovery without reviving
completed conflicts. Fingerprint replay also waits behind an active long
operation, and Team actions remain disabled until that operation is terminal,
so a delayed watcher batch cannot cancel or replace `team.resolve`. This remains
Linux-only evidence.
Conflict and fingerprint durability now share the same version-1 transaction
envelope: canonical project root, renderer generation, batch ID, status,
protocol error code, timestamp, and a typed payload. Team sync/commit/resolve
and external-refresh histories therefore record the same pending,
sidecar-committed, completed, cancelled, and request-cancelled (`-32800`)
semantics. Load paths reject a mismatched version/root and never revive
terminal envelopes; generation rollover cancels stale fingerprints.
`project.external-refresh` now persists a `sidecar_committed` checkpoint before
its successful response reaches Electron. If Electron dies before its ack, the
replacement renderer performs only a six-field rebind from the exact durable
result and then completes the durable head, rather than replaying the already
successful refresh or re-listing sidecar state. Sidecar and renderer fault-injection tests cover
this exact response/ack gap. The Linux package run also changes a watched
source, SIGKILLs Electron from the main-process boundary immediately after the
successful sidecar response, verifies the durable `sidecar_committed` head,
then restarts and observes the new `Document3` plus one completed batch with
exactly one `project.external-refresh` request.
The narrower product-result/checkpoint window is now closed by a shared atomic
JSON publisher: the exact refreshed entry list, project properties, statistics,
SHA-256 product receipt, and `sidecar_committed` state cross one file
fsync/rename plus parent-directory fsync while the sidecar session lock remains
held. A publish failure restores the prior in-memory session. Process-abort
injection on both sides of that rename proves that the pre-publish candidate
remains pending and is replayed once, while the post-publish result is rebound
directly from the envelope without another `project.external-refresh`,
`entry.list`, or `stats.get`.
Shared transaction-journal compaction now archives terminal,
renderer-acknowledged records before adopting a replacement process. It
preserves both an unacknowledged
`sidecar_committed` receipt and every pending FIFO tail, and it does not rewrite
an old terminal record to the replacement renderer generation. Sidecar contract
coverage injects an acknowledged old record ahead of an unacknowledged receipt
and pending tail, then proves stale-generation and cross-project queues cannot
be revived. Legacy-refresh migration rejects a version-1 envelope with an
unknown payload field and a future version-2 envelope without modifying or
archiving either invalid input.
The compaction fault matrix now terminates separate sidecar processes after the
terminal archive fsync and after the compacted queue's atomic rename. The first
failure leaves the original queue authoritative; the second leaves the compacted
queue authoritative. In both cases the next process receives the exact
unacknowledged receipt and pending tail, while only the acknowledged terminal
record can disappear.
A real Linux `linux-unpacked` run now parks the packaged sidecar at both of those
durable boundaries and externally SIGKILLs Electron's entire process group. At
the archive boundary the original acknowledged + unacknowledged + pending queue
remains authoritative; at the queue-rename boundary the compacted
unacknowledged + pending queue is already authoritative. In each scenario a
second Electron instance, deliberately using the same OmegaT config directory,
simultaneously recovers a different project's `entry.set` receipt and remains
responsive while the first is killed. Chromium profiles and config-scoped
active-project pointers are owner-isolated, so opening either root does not
cancel or adopt the other owner's queue. Restarting the first package while the
second stays live drains only its own receipt and FIFO tail, retains the exact
six-field `EntryKey` and one `Document3`, and leaves both histories free of the
other project's batch IDs.
The same packaged matrix creates one real team commit, two ordered refreshes,
and a trailing save in one project, then drops refresh acknowledgements until
SIGKILL. Restart does not replay the acknowledged team receipt, dispatches the
unacknowledged refresh plus its refresh/save tails in FIFO order, and archives
each terminal batch exactly once. Refresh state transitions and event
coalescing preserve the original dispatch key, and only the single shared
journal head may compete for dispatch. This is Linux-only evidence.
The matrix now repeats the lost-ack boundary separately for all three receipt
classes. For **team**, it first acknowledges an older refresh, drops the team
ack, SIGKILLs the package, then proves restart omits that older refresh and
dispatches team → refresh → refresh; the file-remote bytes and nanosecond mtime
stay unchanged, proving the committed team write was not replayed. For
**refresh**, the acknowledged team receipt stays absent while the exact
unacknowledged refresh → refresh → save sequence drains. For **save**, an
acknowledged team plus two acknowledged refreshes stay absent while the
unacknowledged save remains ahead of a later real glossary-watcher refresh.
Every named terminal receipt occurs exactly once in its backend history, all
three runs retain one `Document3` and the complete six-field `EntryKey`, and no
tail is starved. These are real Linux `linux-unpacked` assertions; Windows and
macOS were not run.
A fourth Linux packaged scenario injects SIGKILL after
`transaction.receipt.pending` has selected the unified team head but before
that envelope is delivered to the renderer. A marker records the selected
batch and old sidecar PID; the same Electron/renderer observes a distinct
replacement sidecar PID, whose first dispatched envelope is the same team
batch. Its refresh tails remain behind it, the team terminal ack occurs once,
and unchanged file-remote bytes/mtime prove recovery did not repeat product
write-back. The sidecar NDJSON contract independently performs the same
select-head → kill → reopen boundary and verifies the later refresh is still
pending until the recovered head is acknowledged.
A fifth Linux packaged scenario applies both boundaries to `project.close`.
It SIGKILLs the sidecar after the unified dispatcher selects the close head but
before renderer delivery, drops the replacement sidecar's close
acknowledgement, and then SIGKILLs Electron after a refresh tail has been
durably queued. The next package starts with no project argument, no active
renderer project, and no native project watcher. A config-scoped, read-only
receipt discovery step selects only a committed close root, adopts it into one
replacement generation, and dispatches close → refresh without binding either
receipt to another project. The renderer remains on the welcome screen while
the refresh tail runs through a temporary sidecar session, which is detached
without a second close/save. Exact byte and nanosecond-mtime snapshots prove
that recovery does not rewrite the close receipt's TMX or stable project tree;
each terminal history has one row. Explicitly reopening afterward restores the
same complete six-field wanted key and its one `Document3` translation while a
same-source decoy remains untranslated. This is Linux-only evidence.
A separate contract keeps two replacement sidecars alive concurrently: project
A recovers an `entry.set` receipt while project B recovers an external-refresh
receipt from the other durable queue. Each response is re-stamped only to its
own replacement generation; asking A's sidecar for B's root is rejected without
consuming A, and both receipts remain independently acknowledgeable.
Editor `entry.set`, explicit document save, and project-close TMX flush now use
the same version-1 snapshot/receipt/recovery state machine as team writes.
Every editor commit is keyed by the full six-field `EntryKey`; the TMX,
`omegat.project`, and last-entry files are fsynced before the SHA-256 product
manifest crosses the atomic envelope rename. A pre-receipt process death leaves
a pending envelope and restores the prior project snapshot on open, while a
post-receipt death verifies and preserves the committed product without
replaying the write. Electron scopes all three methods to the watched project
root/generation, suppresses their native watcher echoes, commits the sole live
`Document3` before Save or Close, and does not report a failed close as
successful. Sidecar contract coverage checks the complete-key alternative TMX
and all three durable receipts. A separate Linux `linux-unpacked` E2E SIGKILLs
the packaged Electron/sidecar process group before and after the receipt rename,
then repeats the post-receipt kill during project close; restart rolls back only
the pre-receipt translation and preserves each receipt-backed translation.
A separate real Linux `linux-unpacked` E2E leaves both pending fingerprint and
conflict envelopes in project A, SIGKILLs the packaged Electron process group
including its sidecar, and starts project B with the same config. B exposes only
its own complete `EntryKey`, sole `Document3`, and empty conflict list; A's
pending FIFO and conflict journal remain byte-identical and never enter B.
After a second packaged SIGKILL, reopening A drains A's refresh to completed
and recovers only A's snapshot and complete-key conflict. This is Linux-only
evidence.
Entry write, save, close, reload, compile, import, project update, team mapping,
team sync/commit/resolve, align run/write, and external refresh now expose the
same renderer receipt fields and use only
`transaction.receipt.pending` / `transaction.receipt.ack` over NDJSON.
Electron routes direct replies and restart recovery through one
`transaction:envelope` dispatcher and one preload acknowledgement API, scoped
by canonical project root, renderer generation, batch ID, and payload operation.
The renderer acknowledges only after the operation-specific state publication:
complete six-field rebind for committed product/refresh work and closed state
for project close. A lost acknowledgement therefore republishes the same
envelope after restart; a duplicate acknowledgement consults durable history
without replaying writes, and an unknown batch/operation is rejected. The real
Linux save/close, cross-project, and two-repository Git+file packaged recovery
runs exercise this shared dispatcher, including idempotent duplicate team
acknowledgements and zero post-receipt write replay.
Product/team and refresh work now persist in the same version-2
`.repositories/transactions/active.json` FIFO and `history.ndjson`; the
dispatcher no longer merges two durable backends only at query time. Former
version-2 refresh queues and histories migrate under the project transaction
lock, preserve their original durable order, and are removed only after the
shared queue/history are fsynced. Exact-batch retries make an interrupted
migration idempotent, while unknown payload fields and future envelope versions
leave the legacy input untouched. Enqueue, selected-head adoption, committed
result publication, cancellation, acknowledgement, history archival, and both
compaction fault boundaries now all use that shared journal. A sidecar contract
creates an older refresh ahead of a team receipt, then two refresh tails ahead
of a save receipt; it observes refresh → team → refresh → refresh → save with
one envelope per turn, exact payloads, one generation, and no starvation.
The product/team transaction store is now a version-2 FIFO journal whose
`batches` retain multiple envelopes in durable insertion order; a version-1
single-envelope `active.json` is read as its first row and migrated on the next
state transition. A committed but unacknowledged close therefore no longer
blocks a later save from being durably appended, while the unified dispatcher
still publishes only the close head before that save and any refresh tail.
Dispatch ownership is persisted separately under the same project transaction
lock with canonical root, Electron app instance, process ID, renderer
generation, and claim ID. On Linux, another replacement cannot read or
acknowledge the shared head while that owner PID is live; it can take over only
after the owner exits.
`project.reload`, `project.compile`, `project.import`, `project.update`,
`team.mapping`, and `align.write` now use the same local product transaction
state machine as save/close/editor writes. Their project and external product
paths are snapshotted before mutation: compile covers target plus exported TM
directories, import covers its source destination, property changes cover old
and newly configured project paths, and align write covers its destination.
Cancellation or a pre-receipt failure restores both the in-memory session and
durable paths; a successful mutation publishes one `sidecar_committed` manifest
receipt before returning. Electron scopes and serializes these calls, assigns
direct receipts to the initiating renderer action, and acknowledges only after
the operation-specific state is published. The real Linux product-compaction
matrix queues all six operations between an entry receipt and team/save/refresh
tails, kills two successive elected owners before delivery, then drains the
exact FIFO once. Stable project bytes, TMX and align-output bytes/mtimes, and the
file remote prove receipt recovery did not replay a committed write.
The remaining project-root writers now join that same boundary:
`glossary.add`, `search.replace`, project `spell.ignore` / `spell.learn`,
destination-bearing `tmx.export`, `wiki.import`, and durable `script.run` /
`script.slot`. Their in-memory TM, external-TM, glossary, and spell state is
checkpointed with the entry list; project and external output paths are restored
on cancellation or pre-receipt failure. Each scoped success returns the exact
shared-journal receipt, and Electron suppresses watcher echoes, serializes the
caller-managed reply, refreshes affected renderer state, and acknowledges only
after publication. Global preferences (including filter and segmentation
fields), persisted aligner settings, and `spell.install` now use a separate
config-scoped `transactions/shared-config` FIFO and OS lock. Recursive merge
patches apply only leaves changed from each renderer's loaded snapshot, so a
stale process cannot erase an unrelated field committed by another process.
Failures retain the last valid in-memory preferences, and no config batch is
written to either project's journal.
The shared journal keeps an atomically written same-value recovery copy. A
corrupt `active.json` is repaired only from a valid copy; two corrupt copies are
rejected without product mutation, while a failed post-mutation active publish
restores the product snapshot and leaves a recoverable journal copy. Legacy
refresh queue, history, and config active-owner migration is idempotent when
interrupted after the new destinations become durable but before old files are
unlinked.
A real Linux `linux-unpacked` writer matrix now drives glossary add, replace,
wiki import, TMX export, and script execution through visible renderer controls.
For each writer it SIGKILLs the complete Electron/sidecar process group before
and after atomic product publication, then verifies rollback/commit bytes,
committed nanosecond mtimes, one exact terminal history row, and the recovered
visible state. A second packaged scenario interleaves glossary → team commit →
replace → external refresh → wiki → save → TMX export → script → close in one
durable FIFO. It rejects a live competing process, kills the owner, drops the
replacement owner's glossary acknowledgement, kills that process too, and
requires a third process to drain the exact order once while every external
file's bytes and mtime remain unchanged.
The same package displays prefs and spell persistence errors under real
read-only directory permissions and injected `rename(2)` / `fsync(2)` failures.
The live file remains byte- and mtime-identical at each failure boundary; a
clean restart retains the prior locale and still exposes the unignored spell
issue without replay. The sidecar contract separately pauses after durable
legacy config-owner publication and after durable shared-journal publication,
terminates the real sidecar process at each point, and proves the final
migration has one copy of every legacy batch/history row and the expected
per-app owners. This evidence was run on Linux only; Windows and macOS packages
were not run.
A real dual-Electron/dual-project Linux matrix now holds that config FIFO at a
durable owner checkpoint, queues a second visible preferences write, SIGKILLs
the first process group, and verifies locale plus font fields merge in FIFO
order. It separately kills the owner after terminal history is durable but
before response cleanup; a visible segmentation edit and a concurrent visible
file-filter edit survive with one history row for the lost response. Concurrent
persisted aligner settings and spell installation share the same config queue,
while exact batch/operation assertions keep all config rows out of both project
journals.
The same packaged run terminates real processes after the preferences
candidate fsync, destination rename, and parent-directory fsync. Recovery
replays the pending merge and removes the one pre-rename candidate without
leaving hidden temporaries. Spell installation stages and fsyncs the `.aff`/
`.dic` pair, then the matrix terminates after staging fsync, the first rename,
and destination-directory fsync; each restart repairs a complete pair and
removes every staging directory. The full old+new writer matrix passes through
`npm run test:e2e:writer-recovery:linux`. Windows and macOS runners were not
available and those package paths were not run.
The shared-config FIFO is now explicitly **v2**. Startup upgrades v1 primary or
recovery `active` journals, v1 terminal history, complete v1 dedupe indexes, and
the older no-index layout under the same OS lock. A partially completed
migration can contain v2 replicas, v1 peers, and a renamed but not yet manifested
archive segment; restart validates and adopts that segment without duplicating
or losing a terminal result. `active.json` still publishes first to a same-value
`active.recovery.json`, chooses the highest valid revision, repairs one missing
or corrupt peer, and refuses product mutation when both replicas are invalid.

`dedupe.json`/`dedupe.recovery.json` are now a bounded hot index (latest **64**
terminal batches in production), not an ever-growing full-result file. Evicted
results move in FIFO order to immutable, SHA-256-named v2 archive segments.
`manifest.json`/`manifest.recovery.json` durably describe those segments; a
segment reaches stable storage before either manifest advances, and the hot
index is pruned only after both manifest publication steps. `history.ndjson`
remains a bounded **64**-row diagnostic projection. Any archived batch can still
return its exact original success or failure, while a reused identity with a
different operation or payload is rejected before product mutation.

Archive lookup no longer deserializes every historical terminal result at
startup. The dual manifest carries a complete four-hex SHA-256 batch-prefix
index whose entries select only candidate immutable segments; collisions can
cause an extra segment read but cannot hide a batch. Older v2 manifests receive
the index under the same process lock. Missing referenced segments fail before
product mutation, while a selected segment's descriptor and content hash are
verified during its streaming point query. Equal-revision manifest replicas
with different contents are rejected instead of choosing one.

Small immutable segments are merged into a new generation without modifying
the predecessor. All replacement segments reach stable storage first, then the
replacement manifest crosses both recovery and primary durable-replacement
steps. Only after those replicas agree may predecessor files be unlinked and
their directory fsynced. A death during staging discards only the unreferenced
future generation; a death after either manifest replacement repairs the peer
before GC; repeated deaths during GC resume from the remaining unreferenced
predecessors. Moving the complete config directory rebases active, hot-index,
history, manifest, and sidecar in-memory paths while leaving content-addressed
archive bytes unchanged.

The real Linux `linux-unpacked` evidence now covers the earlier **8**
`active.json`/`history.ndjson` replacement boundaries plus **16** primary and
recovery dedupe/manifest candidate-write, candidate-fsync, rename, and
parent-directory-fsync deaths, and all **4** immutable-segment write/fsync/
rename/directory-fsync deaths. Every restart verifies one ordered terminal
result, bounded hot/history state, same-value replicas, immutable archive bytes,
no hidden candidate, and byte/mtime-stable product state. ENOSPC is injected at
both dedupe and manifest publication, EACCES at manifest publication, and a real
`0555` transaction directory verifies permission denial before product
mutation. A dual-Electron rolling-upgrade case starts from v1 active/history
with no index, kills the first process after archive rename, proves the second
pre-existing process cannot bypass its lock, kills that process after recovered
compaction, then has a third process finish migration and return an arbitrary
old batch's exact result. The cross-platform packaged driver resolves Linux,
Windows, and macOS layouts and process-tree termination, but Windows and macOS
file-lock/replacement package evidence was **not run** because this runner is
Linux-only.
The same packaged command now additionally rejects a missing segment, selected
segment hash tampering, and same-revision manifest conflict without changing
the preferences product; it moves a populated config directory and returns an
archived result exactly; and it SIGKILLs three consecutive GC owners after one
predecessor unlink each. Every owner observes byte-identical generation-1
manifest replicas before deletion, and the fourth process removes the final
predecessor without replaying the product. This Linux matrix passes through
`npm run test:e2e:shared-config-v2:linux`; corresponding Windows and macOS lock,
rename, and directory-fsync evidence was not run because no such runner was
available.
A new real Linux `linux-unpacked` matrix drives project properties, repository
mapping, file-filter options, and segmentation settings exclusively through
visible controls. It externally SIGKILLs Electron at both sides of each relevant
product receipt and then performs one clean project close/reopen. Project
properties and mapping follow project rollback/commit, while the config-scoped
filter and SRX preferences remain durable even when the following
`project.reload` receipt rolls back. Windows and macOS package behavior are not
claimed by this matrix. The eight injected receipt boundaries plus close/reopen
pass through `npm run test:e2e:config-receipt-recovery:linux`.
Electron serializes all receipt-bearing RPCs and excludes active caller-managed
receipts from the recovery channel. Recovery delivery is identity-deduplicated
for one renderer lifecycle, but the delivery set is cleared when the renderer
reloads or the stateful sidecar exits. A durable head delivered immediately
before sidecar death is therefore republished by the replacement sidecar and
cannot permanently stall its FIFO tail. `project.open` and
`project.recovery.detach` also participate in watcher-write suppression, so
their `.lock` changes cannot synthesize a duplicate external refresh.
A Linux packaged owner-takeover matrix now SIGKILLs the entire Electron process
group after its durable close-head claim but before
`transaction:envelope` delivery. A single replacement Electron process adopts
that same head under a new claim and generation, while a simultaneous contender
remains responsive but cannot dispatch or acknowledge it. The recovered
detached queue drains close → `team.commit` (`commit-target`) → save → refresh;
the dead claim never advances a tail, every terminal receipt occurs once, and
exact remote bytes/nanosecond mtime plus the stable TMX/project tree prove that
neither the team write nor another product write was replayed. Explicit reopen
retains the complete six-field wanted key, its sole `Document3` translation,
and the untranslated same-source decoy. The sidecar contract independently
keeps the same close → team → save product-journal order and unchanged remote
and TMX mtimes through replacement ownership. The complete package, owner, and
compaction matrix passes through
`npm run test:e2e:compaction-dual-recovery:linux`. Windows and macOS owner
liveness/package behavior were not run.
Shared `active.json` v2 compaction retains the durable product fault boundaries
after refresh migration. It idempotently archives each acknowledged terminal
row and fsyncs `history.ndjson` plus its parent before changing the queue; it
then atomically renames and parent-fsyncs the compacted shared queue.
At `after_archive_fsync`, SIGKILL leaves the original terminal → unacknowledged
entry receipt → team receipt → save receipt queue authoritative. At
`after_queue_rename`, the compacted unacknowledged entry → team → save queue is
already authoritative. The sidecar contract kills a separate process at each
boundary, verifies one terminal archive row, FIFO-head retention, and takeover
from the dead durable owner. The real Linux packaged matrix independently parks
and SIGKILLs the Electron process group at both product boundaries, then proves
that, while the original owner PID is still alive, a simultaneous same-root
contender receives neither the envelope nor an acknowledgement: both pending
and ack requests are rejected and leave the shared queue, refresh tail, and
durable claim unchanged. Only after external SIGKILL and confirmed
owner exit does one replacement claim take over. The matrix keeps a pending
refresh behind the product head and proves the shared global FIFO drains entry
→ team → save → refresh exactly once without changing TMX or file-remote
bytes/mtime. The sidecar contract applies the same pre-kill pending/ack rejection
at both checkpoints and retains its contender while the replacement claims.
Both paths retain the exact six-field duplicate `EntryKey`, one `Document3`
translation surface, and an untranslated same-source decoy. This is Linux-only
evidence; Windows and macOS were not run.
The owner matrix now also starts **two replacement Electron processes
concurrently**, only after `/proc/<old-owner-pid>` has disappeared. It repeats
that race with `project.close`, `team.commit`, and `project.save` as the durable
product head, each followed by a refresh tail. The project transaction lock
selects exactly one new claim ID: only that process receives the head and drains
product → refresh, while the losing process records zero renderer envelopes and
both its pending query and acknowledgement are rejected. Each scenario also
rejects a contender while the old owner is live. Product and refresh histories
contain one terminal row per batch, complete six-field duplicate keys and the
sole `Document3` translation survive, and exact TMX plus file-remote bytes/mtime
prove no product or team write was replayed. A sidecar contract performs the
same simultaneous two-process election for all three product-head classes.
These assertions pass in the real Linux `linux-unpacked` package; Windows and
macOS were not run.
The product-compaction matrix now goes further at both `after_archive_fsync`
and `after_queue_rename`: after confirming the old owner PID has exited, it
starts **three** replacement Electron processes concurrently. Exactly one
durable claim wins, both losers have pending and acknowledgement requests
rejected, and no process has delivered an envelope when the winner is parked.
That first winner is then SIGKILLed before renderer delivery; a second,
independent wave of **three** replacements automatically elects exactly one new
claim for the same product head. Only the second winner drains entry → team →
save → refresh, with one terminal row per batch and no TMX/file-remote replay.
The sidecar contract uses an earlier boundary immediately after the atomic
owner claim but before compaction and FIFO-head lookup: it kills that first
claimant before an NDJSON result exists, confirms the product queue is
byte-identical, and performs the same three-way second election at both
compaction checkpoints.
A separate packaged election now uses a real Git main repository plus a file
mapping for a committed `team.sync` product head. Three simultaneous
replacements produce one claim and two zero-envelope losers; recovery preserves
the exact Git HEAD, file-remote bytes and nanosecond mtime, TMX, six-field
`EntryKey`, and sole `Document3`, then dispatches the refresh tail only after
the team head. The close, file-team commit, and save election cases also run
with three replacements. These are real Linux `linux-unpacked` assertions;
Windows and macOS were not run.
A further real Linux packaged election now runs symmetrically from the visible
keep-theirs **and keep-ours** controls for `team.resolve` conflicts backed by a
real Git main repository and a separate file mapping. In each case four live
replacement Electron processes compete for the committed resolve head. The
first claim winner is SIGKILLed before renderer delivery; the three
already-running losers observe that exact PID exit and retry without a process
relaunch. The second winner is also SIGKILLed after its claim and before
delivery; its two surviving losers observe the new PID and perform a third
election. Exactly one third owner receives one envelope, the final loser is
rejected, and no extra process is launched. Every non-winner records zero
envelopes. The single terminal history row, unchanged stable project tree and
TMX mtime, unchanged Git HEAD, and unchanged file-remote bytes/mtime prove that
neither owner death replayed a committed product write. The winner rebinds the
complete six-field wanted key to the selected ours/theirs translation in one
`Document3` surface, while the same-source decoy stays untranslated.
Before each election, the same package cancels one committed resolve after its
owner claim but before renderer delivery. Its operation trace remains
`cancelling` until the sidecar returns `-32800`, then becomes `cancelled`;
exact TMX/conflict bytes are restored, no Git or file-remote write occurs, the
same-source decoy remains untouched, and no cancelled envelope survives.
The sidecar contract independently runs keep-ours through the same two owner
deaths and third election. It also cancels a contender while that contender is
waiting on a live owner, verifies that the durable claim does not change, then
cancels the committed head after claim and confirms one request-cancelled
history row with no later delivery. These are real Linux assertions; Windows
and macOS owner-liveness/package behavior were not run.
Cancellation now first atomically replaces the committed resolve receipt with
an undispatchable `cancellation_pending` intent that preserves its original
global FIFO key. Only then does it durably restore TMX and conflict state and
publish `request_cancelled` with **-32800**. Restart recovery compensates that
intent idempotently, including process death after the intent queue rename,
after restored-product fsync, and after the terminal queue rename. A resolve
receipt may also be cancelled while save, close, and team-sync receipts remain
ahead of it: those heads still drain in their original order, while the resolve
tail is never selected.
A real Linux packaged matrix now parks cancellation separately after the intent
queue rename, after the durable rollback fsync, and after the terminal queue
rename. At every boundary the UI still says `cancelling` until protocol
**-32800**, then the Electron owner and sidecar are externally SIGKILLed
together. Three replacement Electron processes observe the restored
conflict/TMX, but none receives the cancelled resolve envelope and exactly one
durable dispatcher owner remains. The rollback-fsync case sends a second
wave of **three packaged Electron processes concurrently** against the same
`cancellation_pending` row. The first cancellation owner parks while holding
the OS product lock; both losers emit their wait checkpoints before that
owner's complete packaged process group is externally SIGKILLed. Kernel lock
release lets exactly one of those already-running losers take over without a
relaunch, while the other observes the idempotent terminal. Both survivors
receive protocol **-32800**, only one `renderer-rollback-durable` checkpoint
and one terminal row exist, neither caller claims the resolve dispatcher, and
all envelope traces stay empty. The raw NDJSON contract independently kills
its first sidecar owner only after the second caller has entered the lock wait;
that same waiting process takes over, the killed logical caller retries the
same idempotency key, both logical calls end at **-32800**, and the sidecar
remains responsive with no resolve envelope or second rollback pass.
The FIFO-tail intent row now keeps three packaged cancellation callers blocked
on the original owner's OS lock before that owner is killed. The first
already-waiting loser selected by kernel lock release performs the sole real
TMX/conflict rollback, then is SIGKILLed at `after_rollback_fsync`. The second
loser remains in its original RPC, observes `renderer-rollback-durable`,
publishes the sole terminal, and is also SIGKILLed at
`after_terminal_queue_rename`, where `request_cancelled` is durable and visible
but its RPC result has not returned. The third original loser stays blocked
through both deaths, then reads that published terminal and returns **-32800**
without a takeover marker, another rollback, another terminal, a process
relaunch, or a resolve envelope. Retrying the three killed logical calls makes
all four callers converge on **-32800**. At the direct
`after_rollback_fsync` row, the selected loser still publishes only the
terminal without rewriting the already durable rollback. The consecutive row
has two ordered takeover markers but exactly one rollback-durable row and one
terminal, and every cancellation caller emits zero resolve envelopes. The raw
NDJSON contract and real Linux packaged FIFO matrix run the same nonempty
`team.sync` → save → close prefix through all three owner deaths while retaining
the complete six-field wanted/decoy keys and single `Document3` translation
surface.
The terminal-rename case also SIGKILLs packaged process groups at
`after_archive_fsync` and `after_queue_rename`; restart sees one archived
request-cancelled row and an empty compacted queue.
The packaged FIFO-tail case is now a seven-row combined matrix: real
`team.sync` → save → close receipts stay ahead of resolve while SIGKILL occurs
at each of `after_intent_queue_rename`, `after_rollback_fsync`,
`after_terminal_queue_rename`, `after_archive_fsync`, and
`after_queue_rename`, plus two consecutive-owner rows in which the third
pre-existing cancellation waiter reaches `after_archive_fsync` or
`after_queue_rename` itself. That waiter is released from the exact durable
checkpoint and returns **-32800** without a cancellation takeover marker,
another rollback, another terminal publication, or a resolve envelope. The raw
NDJSON contract runs the same two waiter/compaction combinations. In both
surfaces all four logical cancellation calls converge on **-32800**, one
rollback-durable row and one request-cancelled row remain, and the product
journal keeps the exact `team.sync` → save → close FIFO prefix while archiving
only the cancelled resolve tail.
In every row exactly one replacement delivers those three
heads in order, losing pending/ack calls are rejected, and every process
delivers zero resolve envelopes. In the first two rows, the cancellation
takeover completes before replacement dispatch, so FIFO recovery cannot hide a
missing or duplicate product rollback. Exact Git HEAD, file-remote bytes/mtime,
complete six-field wanted and decoy keys, and the single `Document3` surface
remain unchanged. These assertions run through
`npm run test:e2e:compaction-dual-recovery:linux`; this is Linux-only evidence,
and Windows and macOS were not run.
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
continues after the holder exits.
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
All transactional `team.resolve`, mapped multi-repository `team.sync`, and
`team.commit` paths now publish a SHA-256 manifest of the committed project
tree, prep state, file remotes, Git versions, and root-Git HEAD inside the same
version-1 envelope as the terminal commit decision. Pending recovery still
restores snapshots and compensates published repositories; recovery that sees
the atomically published receipt removes only transaction state and preserves
the committed TMX/conflict result. A real child-process abort immediately after
receipt publication proves the resolution is neither rolled back nor replayed.
The real Linux `linux-unpacked` path now also drives the visible `team.sync` and
source `team.commit` controls against one main plus one mapped file repository.
It SIGKILLs the Electron/sidecar process group on both sides of each receipt
rename. Pre-rename recovery restores the user project tree and both remotes
byte-for-byte and removes `active.json`; post-rename/pre-renderer-ack recovery
preserves the one receipt-backed product without replaying remote writes. All
four interruptions retain the wanted duplicate's six-field `EntryKey`, leave
the same-source decoy untranslated, keep one `Document3` surface, and leave the
active UTF-16 caret on that wanted segment. This remains Linux-only evidence.
The suite is now **41 passed / 1 ignored**; the preserved SVN binary prerequisite
remains the single ignore.

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
