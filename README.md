# OmegaT

OmegaT is a free computer-assisted translation workstation (GNU GPL v3+).

This repository is the **Rust + Electron** application. The former Java 21 / Swing / Gradle tree has been retired.

- Desktop: React 19 + Vite + Electron
- Engine: Rust sidecar over newline-delimited JSON-RPC 2.0 (stdio)
- CLI: the same `omegat-core` crate
- Project files stay compatible: `omegat.project`, `omegat/project_save.tmx`, `source/`, `target/`, `tm/`, `glossary/`

## Quick start

Requirements: Rust stable (see `rust-toolchain.toml`), Node.js 22+.

```bash
cargo test --workspace
cargo build -p omegat-sidecar -p omegat-cli

cd apps/desktop
npm install
npm run typecheck
npm test
npm run dev
```

Headless CLI:

```bash
cargo run -p omegat-cli -- translate /path/to/project
cargo run -p omegat-cli -- stats /path/to/project
cargo run -p omegat-cli -- --help
```

There is no `./gradlew`. Java is not part of the default build.

## Layout

| Path | Role |
|---|---|
| `crates/omegat-ipc` | JSON-RPC types and error codes |
| `crates/omegat-core` | Project, TMX, SRX, matching, glossary, stats |
| `crates/omegat-filters` | File filters |
| `crates/omegat-team` | Git / SVN / HTTP / file repositories |
| `crates/omegat-script` | JavaScript event hooks |
| `crates/omegat-plugin` | Manifest schema and built-in registry |
| `crates/omegat-sidecar` | Desktop engine process |
| `crates/omegat-cli` | `omegat` CLI |
| `apps/desktop` | Electron shell |
| `fixtures/` | Golden files for filters and TMX |
| `docs/manual/` | User manual |
| `docs/rewrite/` | Status, packaging, plugin ABI |

## Design

Desktop UI follows `apps/desktop/DESIGN.md` and `skills/design-taste-frontend/SKILL.md`: IBM Plex, ink-ochre accent, high density, keyboard first.

## Packaging

See [docs/rewrite/PACKAGING.md](docs/rewrite/PACKAGING.md). Unsigned Linux `tar.gz` / `dir` images can be produced in CI. Windows NSIS and macOS DMG signing/notarization are documented, not automated.

## Compatibility notes

- Existing OmegaT project directories open without conversion.
- Java plugin JARs are not loaded. New plugins use `omegat-plugin.toml` (see [docs/rewrite/PLUGIN_ABI.md](docs/rewrite/PLUGIN_ABI.md)).
- Scripts are JavaScript, not Groovy. Event directory names are unchanged.
- LanguageTool is an optional external HTTP service.
- Known engine deltas: [docs/rewrite/STATUS.md](docs/rewrite/STATUS.md).

## License

GNU General Public License v3 or later. See `LICENSE` and `THIRD_PARTY.md`.
