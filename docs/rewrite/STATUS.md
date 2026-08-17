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
| Honest STATUS + ACCEPTANCE (this file) | G0 | scaffold |
| Java Gradle exporter `exportGoldens` | G0 | parity |
| Text / PO / HTML Java-exported goldens | G0 | parity |
| Filter / align / SRX fixtures under `fixtures/` | G0 | scaffold |
| Sidecar method contract tests | G0 | scaffold |
| RealProject / SRX / TMX / matching / stats / tags | G1 | parity |
| filters2: 21 Filter classes, one module each | G2 | parity |
| filters3: XML event engine + 23 Dialect modules | G3 | parity |
| filters4: ZIP / XLIFF / SDL / Office node write-back | G4 | scaffold |
| Desktop: document-model editor, 113 menus, 28 prefs | G5 | scaffold |
| Tokenizers / spell / dictionaries / LanguageTool | G6 | scaffold |
| 7 MT engines, External Finder, autocompleter | G7 | scaffold |
| team2: 23 classes, rebase, conflict UI | G8 | scaffold |
| Aligner, embedded JS, Wiki / MED / CLI | G9 | scaffold |
| 41 locales, packages, plugin ABI, manual | G10 | scaffold |

## What is not accepted (previous claims)

The R0–R8 `parity` table was withdrawn. The tree is a CAT prototype, not a
finished rewrite of Java 6.2.

Known compression that **must stay `scaffold` / `parity_gap` until rebuilt**:

- `dialect_filter!` / one `XmlDialect` tag-name table (**removed in G3**)
- `filters.options` returning a generic `extra` map
- full-file `replacen` / first `find` as the only XML / Office / SDL write-back
- `filter_goldens.rs` `contains` / `must_contain` / `n >= 49` (removed in G0)
- `Preferences.extra: HashMap` as the preference model
- `contentEditable` as the segment editor
- `fallback_eval("1+2")` as a script engine
- toy `resources/languages` word lists with no `.aff`
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

Not this wave: filters4 `Xliff1Filter` / `Xliff2Filter` / `SdlXliff` / Office node write-back (G4). `office.rs` G4 stubs remain unregistered.

## G0 notes

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

Methods exist on the wire. That is not parity. Missing or stubbed behaviour
stays `scaffold` until the owning wave’s goldens are green.

## Intentional non-goals (must still have a full replacement)

- Java JAR plugins are not loaded. Replacement: `omegat-plugin.toml` + cdylib.
- Groovy is not executed. Replacement: embedded JS with the Java binding surface.
- LanguageTool is not an embedded JAR. Replacement: HTTP `v2/check`, with an
  `severity=info` downgrade item when no URL is configured.
