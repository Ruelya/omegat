# Packaging

```bash
cargo build --release -p omegat-sidecar -p omegat-cli
cd apps/desktop
npm ci
npm run build
npm run dist
```

electron-builder reads `apps/desktop/package.json`. The sidecar binary is copied from `target/release/` (`omegat-sidecar` or `omegat-sidecar.exe`) into `extraResources`. The user manual is copied from `docs/manual/`.

| Platform | Target | Notes |
|---|---|---|
| Linux | `dir`, `tar.gz`, `deb`, `rpm` | CI smoke builds unsigned `dir` + `tar.gz` |
| Windows | `nsis`, `dir` | Code signing is a manual release step |
| macOS | `dmg`, `dir` | Notarization is a manual release step |

CI produces **unsigned** artifacts only. Signing certificates and Apple notarization stay off the default pipeline.

Set `OMEGAT_CONFIG_DIR` to override `~/.omegat`.

Sidecar lookup order in the Electron main process:

1. `process.resourcesPath/omegat-sidecar[.exe]` (packaged)
2. `target/release/omegat-sidecar[.exe]`
3. `target/debug/omegat-sidecar[.exe]`
