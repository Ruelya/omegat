# Acceptance (parity rewrite)

A feature is complete only when **all** of the following are true.

1. **Java goldens.** Committed files under `fixtures/goldens/` were written by
   running Java (`reference/java` Gradle task `exportGoldens`, class
   `org.omegat.tools.ExportGoldens`). The JSON `java_test` field is a real
   method (`org.omegat…#testName`). “It parses” is not enough.
2. **Assertions.** Segment lists use `assert_eq`. Empty-write output is compared
   to the Java-exported text after documented normalisation (line endings only,
   unless a `parity_gap` records a measured whitespace / tag-order delta).
   Translated write-back must match the Java-exported target text, or the
   translation must appear at the recorded node / offset — not “somewhere in
   the file”.
3. **Modules.** One Java `*Filter` / `*Dialect` / `*Controller` / `*Tokenizer`
   is one Rust or TypeScript file. A shared XML **event-stream** engine is
   allowed. A shared tag-name array is not a dialect.
4. **Options.** Every key from that Java class’s options dialog is a typed
   option and is **read** by `parse` / `write`.
5. **UI.** Every Java dock, window, or preference page has controls that the
   sidecar or renderer consumes. Writing `extra` is not an implementation.
6. **STATUS.** Only `scaffold`, a numbered `parity_gap`, or `parity`. A
   full-table `parity` is forbidden. A wave must not be marked `parity` and
   the next wave must not start until that wave’s goldens are green.
7. IPC types live in `crates/omegat-ipc`. Desktop types stay in sync.
8. `cargo test --workspace` and desktop `npm test` / `tsc` are the product
   checks. `./gradlew` is for local golden export only.

## Golden layout

- `fixtures/goldens/filters/<id>/<case>.json`
  - `id`, `fixture`, `java_test`, `exported_by`, `options`
  - `sources` — exact Java parse list
  - `ids` — when the Java test records ids
  - `empty_write_text` — Java `translateFile` with no translations
    (bilingual: blank allowed; monolingual: source echoed)
  - `translated.source` / `translated.translation`
  - `translated_write` — full Java output with that one translation
- `fixtures/goldens/engine/srx.json` — language → input → sentence list
- `fixtures/goldens/engine/tokens.json` — language → input → token / stem list
- `fixtures/goldens/engine/fuzzy.json` — query / candidate → Java score

## Forbidden assertions (do not count as goldens)

- `contains` / `must_contain` on write-back
- `assert!(n >= 49)` / `tested >= 8` / `parsed_ok >= 40`
- `write(...).is_ok()` without reading the file back
- inserting `GOLDEN_T` and never asserting the Java-exported target text
- `assert_ne!(tokenizer_id(lang), "")`
- `is_ok` as the only check
- a `java_test` that is not a real method name (for example
  `"org.omegat.filters dialect/table for android"`)

## Forbidden

- MVP / skeleton / placeholder panels
- `translate_mock` as the only path for a named MT engine
- Filter parse without write-back
- Marking a phase complete while tests do not open `fixtures/goldens/`
- Deleting `reference/java` before STATUS has zero `scaffold` rows
- A Python / shell “export” that does not execute Java
- Keeping handwritten goldens as green after they have been voided
