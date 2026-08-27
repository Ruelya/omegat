# OmegaT rewrite

The default application is React + Vite + Electron with a Rust sidecar. The Java/Swing/Gradle tree was removed in phase P9.

- Desktop: `apps/desktop` (`npm run dev`)
- Sidecar: `cargo run -p omegat-sidecar`
- CLI: `cargo run -p omegat-cli -- translate <project>`
- Design: `apps/desktop/DESIGN.md` and `skills/design-taste-frontend/SKILL.md`
- Status: `docs/rewrite/STATUS.md`
- Manual: `docs/manual/en.md`
- Packaging: `docs/rewrite/PACKAGING.md`
- Plugin ABI: `docs/rewrite/PLUGIN_ABI.md`

License: GNU GPL v3+ (`LICENSE`).
