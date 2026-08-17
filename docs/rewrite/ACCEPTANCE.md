# Acceptance (parity rewrite)

A Java class is complete only when **all** of the following are true.
Missing one item means the class is not done. Existing `assert_eq` goldens
that cover a handful of exported cases are a **floor**, not a completion proof.

## Completion definition (every Java class)

1. **Full Java test set.** Every `public void test*` on that class’s Java
   `*Test` is written by `exportGoldens` as its own JSON. `java_test` is
   `org.omegat…#method`. Classes with no Java test must gain an
   `ExportGoldens` fixture that calls the class API. “No test, so only a
   module file” is forbidden.
2. **Assertions.** Segment lists / token lists / write-back text / dialect
   tag sets / menu ids use **`assert_eq`**. The only allowed deltas are
   recorded whitespace / tag-order / measured CJK n-gram score gaps, and
   those numbers must live in that format’s `parity_gap`.
3. **Modules.** One Java `*Filter` / `*Dialect` / `*Controller` /
   `*Tokenizer` / `*Marker` / editor concrete class = one Rust or TypeScript
   **file**. The file existing is not completion.
4. **Algorithm source.** Control flow must follow that Java class (or the
   Lucene Analyzer it constructs). “Shared engine + swap id” is not a port.
5. **Options.** Every key from that class’s dialog is a typed option and is
   **read** by `parse` / `write`. CI diffs the Java-exported option-key list;
   a nonempty set fails.
6. **UI.** Every Java dock / window / preference page has controls the
   sidecar or renderer consumes. Writing `extra` is not an implementation.
   `className="placeholder"` is not an implementation.
7. **Structural gates** (`tools/honesty/check.sh`) must be green for that
   wave’s items. Ungreen: do not mark the row `parity`, do not start the
   next wave.
8. **STATUS.** Only `scaffold` / numbered `parity_gap` / `parity`. A
   full-table `parity` is forbidden until P12 (zero `scaffold` rows and
   gates green). Any `stems::identity` or HTML regex main path forces the
   matching row to `scaffold`.

IPC types live in `crates/omegat-ipc`. Desktop types stay in sync.
`cargo test --workspace` and desktop `npm test` / `tsc` are the product
checks. `./gradlew` is for local golden export only.

## Structural honesty gates

`tools/honesty/check.sh` fails CI when any of these hold:

- `stems::identity` in `crates/omegat-core/src/tokenize/lucene_*.rs`
- Product code contains `fallback_eval` / `contentEditable` /
  `translate_mock` / `dialect_filter!` / `simple_filter!`
- `className="placeholder"` under `apps/desktop/src`
- `Command::new("git")` on the `omegat-team` **product** path (tests may
  spawn a bare repo with the `git` binary)
- HTML `parse` uses a block-tag regex as the only splitter; missing
  `filter_visitor.rs`; `HTMLFilter2Test` goldens not all `assert_eq`
- `fixtures/goldens/engine/dialect_tags.json` (Java export) ≠ each
  `*_dialect.rs` tag set
- Each `Lucene*Tokenizer` in `tokens.json` lacks **NONE + GLOSSARY +
  MATCHING**, or the input is Latin `"Hello worlds running"` (English-family
  excepted)
- A Java `*FilterTest#test*` has no golden file; plugin registration is
  “49 Java plugin id directories exist”, not `n >= 49`
- `IEditor` method names (Java export) minus TS/Rust implementation table
  is nonempty
- 120 `*ActionPerformed` names minus `JAVA_MENU_ACTIONS` is nonempty, or a
  menu test only asserts that a `switch` case exists
- Each `apps/desktop/src/renderer/i18n/*.json` key set ≠ `en.json`; a key
  that exists in `Bundle_xx.properties` still equals the English string
  (`en` excepted)
- STATUS table is all `parity` before P12 gates are green
- `python3 tools/export_java_goldens/check_provenance.py` fails

P0 commits these gates **red** on the current tree. Later waves may turn
only their own items green.

## Golden layout

- `fixtures/goldens/filters/<id>/<case>.json`
  - `id`, `fixture`, `java_test`, `exported_by`, `options`
  - `sources` — exact Java parse list
  - `ids` — when the Java test records ids
  - `empty_write_text` — Java `translateFile` with no translations
  - `translated.source` / `translated.translation`
  - `translated_write` — full Java output with that one translation
- `fixtures/goldens/engine/srx.json` — language → input → sentence list
- `fixtures/goldens/engine/tokens.json` — tokenizer × stemming ×
  **language text** → token / stem list
- `fixtures/goldens/engine/fuzzy.json` — query / candidate → Java score
- `fixtures/goldens/engine/dialect_tags.json` — each filters3 Dialect
  paragraph / intact / out_of_turn / attrs / constraints
- `fixtures/goldens/engine/ieditor_methods.json` — `IEditor` method names
- `fixtures/goldens/engine/menu_actions.json` — `*ActionPerformed` names
- `fixtures/goldens/engine/preference_keys.json` — 25 controllers → prefs keys
- `fixtures/goldens/engine/filter_tests.json` — every `*FilterTest#test*`

`exported_by` must be `org.omegat.tools.ExportGoldens`.

## Forbidden assertions (do not count as goldens)

- `contains` / `must_contain` on write-back
- `assert!(n >= 49)` / `tested >= 8` / `parsed_ok >= 40`
- `write(...).is_ok()` without reading the file back
- inserting `GOLDEN_T` and never asserting the Java-exported target text
- `assert_ne!(tokenizer_id(lang), "")`
- `is_ok` as the only check
- a `java_test` that is not a real method name
- a single `NONE` + `"Hello worlds running"` case standing in for a
  Lucene Analyzer
- “the `switch` has this case” standing in for a menu action

## Forbidden

- MVP / skeleton / placeholder panels
- `stems::identity` / shared slavic/romance/nordic suffix tables /
  hard-coded golden word lists as a tokenizer
- HTML/HHC block-tag regex as the only parser
- Dialect tag sets shorter than the Java export
- `translate_mock` as the only path for a named MT engine
- `Command::new("git")` as `GITRemoteRepository2`
- `Preferences.extra` as a writable save model
- Filter parse without write-back
- Marking a phase complete while tests do not open `fixtures/goldens/`
- Deleting `reference/java` before STATUS has zero `scaffold` rows
- A Python / shell “export” that does not execute Java
- Keeping handwritten goldens as green after they have been voided
- Marking the next wave `parity` while this wave’s gates are red
