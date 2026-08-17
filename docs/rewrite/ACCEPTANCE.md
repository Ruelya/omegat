# Acceptance (parity rewrite)

A feature is complete only when all of the following are true.

1. Engine tests assert against Java fixtures under `fixtures/` or goldens generated from `reference/java`.
2. IPC types live in `crates/omegat-ipc` and the desktop types stay in sync.
3. If Java OmegaT 6.2 had a window, dock, or preference page, the Electron UI has working controls that call real RPC. A muted category list is not an implementation.
4. `docs/rewrite/STATUS.md` says `parity` or a **quantified** remaining delta. `done (stub)` is forbidden.
5. `cargo test --workspace` and desktop `npm test` / `tsc` pass.

## Forbidden

- MVP / skeleton / placeholder panels
- `translate_mock` as the only path for a named MT engine
- Filter parse without write-back
- Marking a phase complete while tests do not open `fixtures/`
- Deleting `reference/java` before STATUS has zero `scaffold` rows

## Sidecar methods

Implemented methods must have a contract test (request/response shape). Missing methods are listed in STATUS as `parity_gap` until implemented — they must not return a fake success.
