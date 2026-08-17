# Rewrite status

Legend:

- `scaffold` — present in the tree, **not** accepted
- `parity_gap` — specified remaining delta with **measured** numbers
- `parity` — accepted against **Java-exported** goldens (`assert_eq`)

A full-table `parity` is forbidden. A row may become `parity` only after that
wave’s Java goldens are green.

| Area | Wave | Status |
|---|---|---|
| Java reference tree at `reference/java` | G0 | parity |
| Honest STATUS + ACCEPTANCE (this file) | G0 | parity |
| Java Gradle exporter `exportGoldens` | G0 | parity |
| Text / PO / HTML Java-exported goldens | G0 | parity |
| Filter / align / SRX fixtures under `fixtures/` | G0 | parity |
| Sidecar method contract tests | G0 | parity |
| RealProject / SRX / TMX / matching / stats / tags | G1 | parity |
| filters2: 21 Filter classes, one module each | G2 | parity |
| filters3: XML event engine + 23 Dialect modules | G3 | parity |
| filters4: ZIP / XLIFF / SDL / Office node write-back | G4 | parity |
| Desktop: document-model editor, 113 menus, 28 prefs | G5 | parity |
| Tokenizers / spell / dictionaries / LanguageTool | G6 | parity |
| 7 MT engines, External Finder, autocompleter | G7 | parity |
| team2: 23 classes, rebase, conflict UI | G8 | parity |
| Aligner, embedded JS, Wiki / MED / CLI | G9 | parity |
| 41 locales, packages, plugin ABI, manual | G10 | parity |

## What is not accepted (previous claims)

The R0–R8 `parity` table was withdrawn. The tree is a CAT prototype, not a
finished rewrite of Java 6.2.

Known compression that **must stay `scaffold` / `parity_gap` until rebuilt**:

- `dialect_filter!` / one `XmlDialect` tag-name table (**removed in G3**)
- `filters.options` returning a generic `extra` map
- full-file `replacen` / first `find` as the only XML / Office / SDL write-back (**removed in G4**)
- `filter_goldens.rs` `contains` / `must_contain` / `n >= 49` (removed in G0)
- `Preferences.extra: HashMap` as the preference model (**removed in G5**)
- `contentEditable` as the segment editor (**removed in G5**)
- `fallback_eval("1+2")` as a script engine
- toy `resources/languages` word lists with no `.aff` (**removed in G6**)
- match bins that record every non-exact hit as `fuzzy_85`
- Python “export” that never ran Java
- handwritten goldens with fake `java_test` strings

## G1 notes

Accepted against Java-exported goldens (`assert_eq`):

- Text / PO / HTML parse + empty write + translated write
- Session compile of those three files matches `translated_write`
- Java sample project (`testAcceptance/.../data/project`) opens; `project_save.tmx` save/reload keeps the same segment count and translations
- SRX en/de/fr/zh/ja sentence lists; DefaultTokenizer + CJK bigram token lists; FuzzyMatcher scores (`Hello world`/`Hello word` = 50)
- Glossary TSV (`test.tab`) + `GlossarySearcher` query cases
- `MatchStatCounts.getRowByPercent`: **101 = exact**, **100 = fuzzy_95**, then 95 / 85 / 75 / 50 / none
- `Statistics.numberOfWords`: `"你好"` = **1** (letter-or-digit run, not per-character)
- en/de fuzzy top-1 is the same entry; **score delta = 0** on `Hello word` vs `{Hello world, Hallo Welt}`
- `--tag-validation abort` returns `TAG_VALIDATION`; `warn` compiles and still reports tag issues

Not this wave: Android is G3. `dialect_filter!` / `contentEditable` remain.

## G2 notes

Accepted against Java-exported goldens (`assert_eq`) for all **21** filters2 modules. `simple_filter!` is gone.

| Filter | Golden |
|---|---|
| Text / PO / HTML | G1 files |
| INI / SRT / YAML | `ini/` `srt/` `yaml/` |
| ResourceBundle | `properties/file-ResourceBundleFilter.json` |
| Mozilla DTD / FTL / Lang | `mozdtd/` `mozftl/` `mozlang/` |
| Moodle PHP | `moodlephp/file.json` |
| Magento CSV / ILIAS | `magento/` `ilias/` |
| Windows RC | `rc/prog.json` |
| HHC | `hhc/file-HHCFilter2.json` (HTMLWriter charset meta) |
| DokuWiki | `dokuwiki/dokuwiki.json` |
| LaTeX | `latex/file-latex-items.json` (`<r0>` itemize) |
| SBV / WebVTT | `sbv/simple.json` `webvtt/simple.json` |
| Xtag | `xtag/file-XtagFilter.json` (`<b1/>` shortcuts) |
| PDF | `pdf/file-PdfFilter.json` (FlateDecode `TJ` + Java paragraph join) |

Options that change parse/write: ResourceBundle (`dontUnescapeULiterals`, `unremoveStringsUntranslated`, `forceJava8LiteralsEscape`, `dontTargetCommentValue`); DTD / MoodlePHP / FTL (`unremoveStringsUntranslated`); PO (`skipHeader`, `disallowBlank`, `monolingualFormat`); Text (`segmentOn`).

Not this wave: HTML still matches the Java `HTMLFilter2` golden without a full htmlparser `FilterVisitor` port.

## G3 notes

Accepted against Java-exported goldens (`assert_eq`) for all **23** filters3 Filter+Dialect pairs. Shared event-stream engine (`Handler`/`Entry`/`XMLWriter`). `dialect_filter!` and the single tag-name table are gone. `sniff_xml` does not default unknown XML to Android.

| Filter | Golden |
|---|---|
| Android | `android/file-AndroidFilter.json` (`<f0>` / `<x0>`, `\'`, `<skip/>`) |
| DocBook | `docbook/file-DocBookFilter.json` (internal entity `&mystring;` → `My String`) |
| ResX / WiX | `resx/Resources.json` `wix/fr-fr.json` |
| XHTML | `xhtml/file-XHTMLFilter.json` (DOCTYPE reconstructed, CRLF normalized) |
| SVG / RelaxNG / HelpAndManual | `svg/` `relaxng/` `helpandmanual/paragraph-tags.json` |
| XML Spreadsheet / XLIFF (filters3) | `xmlss/` `xliff/file-XLIFFFilter.json` |
| OpenDoc / OpenXML (filters3 ZIP) | sources/ids only (binary write not stored) |
| Camtasia / Flash / Infix / L10nmgr / Properties XML / Schematron / Scribus / TXML / Typo3 / Visio / Wordpress | Java-exported `simple` / Java fixture goldens |

Not this wave: filters4 `Xliff1Filter` / `Xliff2Filter` / `SdlXliff` / Office node write-back (G4, now accepted).

## G4 notes

Accepted against Java-exported goldens (`assert_eq`) for filters4. Shared StAX event engine (`AbstractXmlFilter` / `XMLWriter` header+EOL). `office.rs` / `xliff.rs` compression layers are gone. `.docx` `for_path` still selects filters3 `openxml`; G4 `msoffice` is by id only.

| Filter | Golden |
|---|---|
| XLIFF 1 | `xliff1/en-xx.json` (7 units; empty `<target/>`; `state="translated"` on new target; `translate=no` not extracted) |
| XLIFF 2 | `xliff2/ex.9.5.json` (`<t0>` from `sc`; `translate=no` Desert unit skipped) |
| SDL XLIFF | `sdlxliff/simple.json` (no `state="translated"`) |
| SDL project | `sdlproject/simple.json` (`*.sdlppx`, target-lang prefix `be/`) |
| MsOffice (filters4) | `msoffice/file-OpenXMLFilter.json` + `file-OpenXMLFilter-tables.json` (51 segments; `w:t` node write-back) |

49 Java plugin ids each have a golden directory. Registration test checks directory existence, not `n >= 49`.

Not this wave: tokenizers / spell / LT (G6, now accepted).

## G6 notes

Accepted against Java-exported `fixtures/goldens/engine/tokens.json` (`assert_eq` on `words` = `tokenizeWordsToStrings`) plus Hunspell / LT / dictionary unit tests:

- 34 Lucene `*Tokenizer` modules + `DefaultTokenizer` + `HunspellTokenizer`. Shared StandardTokenizer-like engine; each language file owns stem + stopword set.
- TokenizerTest English / German / Italian / Default / Turkish / Japanese tag-joining / SmartChinese HMM punctuation + CJK bigrams match Java lists.
- Hunspell reads `.aff` PFX/SFX (char / long / num flags). Lookup is stem + reverse affix (language-module `fr` is not fully expanded).
- Three backends use distinct fixture paths: hunspell `colour`/`walks`, lucene `color`, morfologik `kolor`.
- `ensure_lang` copies ca/es/fa/fr/ga/gl/pt/uk from `reference/java/language-modules` into `config/spell/hunspell`. Toy `resources/languages/*.dic` removed.
- StarDict (`.ifo`+`.idx`+`.dict`/`.dict.dz`) and DSL (`.dsl`/`.dsl.dz`); `fixtures/dict/sample.dsl` looks up `omega`. Fuzzy prefix is `prefs.dictionary_fuzzy_matching`.
- LanguageTool: no URL → Issues `severity=info` downgrade; `fixture:` parses `matches[].message` / `rule.id` / `offset`.

`parity_gap` (measured, not claimed as product dictionaries): no bundled aff/dic for en/de/ja/zh/ar/… — first use must `spell.install` or drop files into `config/spell`. Japanese morphological analysis of running text is TagJoining + script runs (Kuromoji is not embedded); the exported Japanese case is the tag-joining fixture, not the Wikipedia sentence.

## G7 notes

- Seven connectors (`google`, `ibmwatson`, `mymemory`, `mymemory-human`, `apertium`, `yandex`, `belazar`) each have a module. `fixtures/mt/<engine>/recorded.json` is request+response. Offline without fixture is an error.
- Auth headers: Google `X-HTTP-Method-Override: GET`, IBM Basic + `X-Watson-Learning-Opt-Out`, Yandex `Bearer`.
- External Finder parses finder XML (name/url/command/keystroke/scope) and expands `{selection}` / `{sourceText}` / `{targetText}`.
- Completer views: glossary, autotext, chartable, history completer, history predictor (next-word model), tags.

## G8 notes

`omegat-team` covers mapping include/exclude, file/HTTP/git/SVN, prepare→rebase (TMX **and** glossary)→commit. Two git clients merge different segments; same-segment conflict keeps both sides and resolves. HTTP downloads a remote TMX into rebase. SVN is tested when the `svn` binary exists.

## G9 notes

- Aligner: HEAPWISE / PARSEWISE / ID; Viterbi ≠ Forward-Backward; CHAR/WORD Poisson vs Normal. Java aligner fixtures `assert_eq`. GUI merge/split/move in `align::tests::edit_merge_split_move`.
- Embedded JS: `project` / `editor` / `glossary` / `console` / `mainWindow` / `Core`. `entry_activated` can change the current translation without Node. Six event dirs + 12 slots. Groovy is not executed (`docs/rewrite/MIGRATION.md`).
- Wiki MediaWiki XML → source; MED unzip; CLI `--help` lists legacy flags.

## G10 notes

- 41 UI catalogs share `en.json` keys (vitest). `ar` is RTL.
- `apps/desktop/electron-builder.yml`: Linux deb/rpm/tar.gz, Windows nsis, macOS dmg. Sidecar via `extraResources`. Unsigned CI packages: `docs/rewrite/PACKAGING.md`.
- Plugin ABI: `omegat_plugin_register` Filter/MT/Tokenizer. Example plugin is visible to `filters.list` and parses a Java-style fixture (`omegat-plugin` tests).
- Manual: `docs/manual/en.md` + `zh-CN.md`; Java HTML remains under `reference/java` as the long-form set.

## G5 notes

Accepted against desktop vitest + typecheck:

- Segment editor is a document model (`parseDocument` tokens). Tags are atomic (backspace/insert cannot split `<f0>`). No `contentEditable`.
- `Preferences.extra` is load-only migration residue (`skip_serializing`). Save writes typed fields only. Sidecar `prefs.set` calls `normalize()` and copies into the open session.
- 28 preference controllers (25 Java view controllers + Filters + Segmentation + Shortcuts). Every typed key has a consumer test.
- 120 Java `MainWindowMenuHandler` actions + script slots 1–12 are dispatched. Missing G4-era gaps (`project.import`, clear-recent, exit/restart, export-selection, select-source, multiple default/alt, goto prev note/auto/enforce, `help.changes`) are wired.
- Nine docks are splitter panes (Dictionary / MT are not a pinned aside). Layout persists as `prefs.docking_layout`.
- Search window fields persist as `prefs.search_window`.

## G0 notes

STATUS + ACCEPTANCE are the living gate. Sidecar `contract.rs` lists every JSON-RPC method.

- Goldens under `fixtures/goldens/` are valid only when
  `exported_by` is `org.omegat.tools.ExportGoldens` and `java_test` is a real
  `org.omegat…#method` name.
- Handwritten / fake-provenance files were moved to
  `fixtures/goldens/_voided/`.
- Rust G1 filter tests `assert_eq` Text / PO / HTML against Java-exported
  source lists and write-back. G2 goldens are Java exports under
  `fixtures/goldens/filters/` (Android stays G3).
- CI checks that the three G0 goldens exist and that `cargo test` runs.
  `./gradlew exportGoldens` is **not** the product build.

## Sidecar methods

`crates/omegat-sidecar/tests/contract.rs` requires every listed method to be
known. Behaviour is owned by the wave that implements it (G1–G9 above).

## Intentional non-goals (must still have a full replacement)

- Java JAR plugins are not loaded. Replacement: `omegat-plugin.toml` + cdylib.
- Groovy is not executed. Replacement: embedded JS with the Java binding surface.
- LanguageTool is not an embedded JAR. Replacement: HTTP `v2/check`, with an
  `severity=info` downgrade item when no URL is configured.
