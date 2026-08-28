# Third-party notices

OmegaT itself is GNU GPL v3 or later. The rewrite links the following third-party components. Consult each project for its full license text.

## Rust (workspace)

| Crate | Role | License (upstream) |
|---|---|---|
| serde / serde_json | IPC and file JSON | MIT OR Apache-2.0 |
| thiserror / anyhow | Error types | MIT OR Apache-2.0 / MIT OR Apache-2.0 |
| clap | CLI | MIT OR Apache-2.0 |
| quick-xml / roxmltree | XML | MIT |
| regex | SRX / search | MIT OR Apache-2.0 |
| walkdir / globset | File walk | MIT OR Apache-2.0 / Unlicense OR MIT |
| unicode-segmentation | Tokenization | MIT OR Apache-2.0 |
| csv | Glossary / CSV filter | MIT OR Apache-2.0 |
| html-escape | HTML filter | MIT OR Apache-2.0 |
| zip | Office / ODF containers | MIT |
| fs2 | Project lock | MIT OR Apache-2.0 |
| sha2 / hex | Digests | MIT OR Apache-2.0 |
| encoding_rs | Legacy encodings | MIT OR Apache-2.0 |
| once_cell / log / env_logger | Utilities | MIT OR Apache-2.0 / MIT OR Apache-2.0 / MIT OR Apache-2.0 |
| similar | Diff / align | Apache-2.0 |
| urlencoding | External finder URLs | MIT |

Exact versions are pinned in the root `Cargo.toml` / `Cargo.lock`.

## Desktop (npm)

| Package | Role | License (upstream) |
|---|---|---|
| electron / electron-vite / electron-builder | Shell and packager | MIT |
| react / react-dom | UI | MIT |
| zustand | State | MIT |
| @phosphor-icons/react | Icons | MIT |
| @fontsource/ibm-plex-sans | UI font | OFL-1.1 |
| @fontsource/ibm-plex-mono | Mono font | OFL-1.1 |
| vite / @vitejs/plugin-react | Bundler | MIT |
| typescript | Types | Apache-2.0 |
| vitest | Tests | MIT |

See `apps/desktop/package-lock.json` for the resolved tree (including Chromium, which ships with Electron under BSD-style and other licenses; see Electron’s `LICENSES.chromium.html`).
