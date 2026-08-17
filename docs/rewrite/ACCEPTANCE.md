# Acceptance (parity rewrite)

A feature is complete only when all of the following are true.

1. Engine tests assert against committed files under `fixtures/goldens/` (exported from Java tests or `tools/export_java_goldens`). “It parses” is not enough.
2. IPC types live in `crates/omegat-ipc` and the desktop types stay in sync.
3. If Java OmegaT 6.2 had a window, dock, or preference page, the Electron UI has working controls that call real RPC **and the sidecar consumes those prefs**. A muted category list is not an implementation.
4. `docs/rewrite/STATUS.md` says `parity` or a **quantified** remaining delta. `done (stub)` and a full-table `parity` without goldens are forbidden.
5. `cargo test --workspace` and desktop `npm test` / `tsc` pass.

## Golden layout

- `fixtures/goldens/filters/<id>/<case>.json` — `sources` (exact Java parse list), `options`, `empty_write` (`preserve_source` or a normalized hash), `translated` (`id`/`source` → expected substring in the written file).
- `fixtures/goldens/engine/srx.json` — language → input → sentence list.
- `fixtures/goldens/engine/tokens.json` — language → input → token/stem list from Java tokenizers.
- `fixtures/goldens/engine/fuzzy.json` — query/candidate pairs with Java `FuzzyMatcher` scores.

## Forbidden assertions

These do **not** count as goldens:

- `assert!(tested >= 8)` / `parsed_ok >= 40`
- `write(...).is_ok()` without reading the file back
- inserting `GOLDEN_T` and never asserting it appears
- `assert_ne!(tokenizer_id(lang), "")`

## Forbidden

- MVP / skeleton / placeholder panels
- `translate_mock` as the only path for a named MT engine
- Filter parse without write-back
- Marking a phase complete while tests do not open `fixtures/goldens/`
- Deleting `reference/java` before STATUS has zero `scaffold` rows

## Sidecar methods

Implemented methods must have a contract test (request/response shape). Missing methods are listed in STATUS as `parity_gap` until implemented — they must not return a fake success.
