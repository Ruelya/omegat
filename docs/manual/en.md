# OmegaT user manual

OmegaT is a keyboard-first CAT workstation. This manual describes the Rust + Electron build.

## Install

- **From source:** install Rust stable and Node.js 22, then follow the repository `README.md`.
- **Linux package:** unpack the CI `tar.gz` or install the `deb`/`rpm` produced by `electron-builder`.
- **Windows / macOS:** use the NSIS installer or DMG. Builds from CI are unsigned unless a release manager signs them.

The engine is a sidecar binary (`omegat-sidecar`). The desktop shell never reads project files itself.

## Create or open a project

1. Start the desktop app (`npm run dev` in development, or the packaged `OmegaT` binary).
2. **Open project** and choose a folder that contains `omegat.project`.
3. **New project** sets source/target languages, the project root, and sentence segmentation.

Existing Java-era project directories open without conversion. Unknown XML in `omegat.project` is preserved.

Standard folders:

| Path | Role |
|---|---|
| `source/` | Source files |
| `target/` | Compiled translations |
| `omegat/project_save.tmx` | Working translation memory |
| `tm/` | Reference TMX (`auto/`, `enforce/`, `mt/`, `penalty-*`) |
| `glossary/glossary.txt` | Writable TSV glossary |
| `dictionary/` | StarDict / DSL dictionaries |

## Translate

- The editor shows source and target for the current segment.
- **Enter** commits the segment and moves forward.
- **Ctrl/Cmd+I** inserts the best fuzzy match.
- **Ctrl/Cmd+N** / **Ctrl/Cmd+P** move to the next/previous segment.
- **Ctrl/Cmd+S** saves `project_save.tmx` (plus `.bak`).
- **Ctrl/Cmd+D** compiles into `target/`.
- **Ctrl/Cmd+F** opens search.

Matches, glossary hits, notes, comments, segment properties, and issues appear in the side panes.

## Preferences

Preferences cover appearance, save behaviour, TM matching, file filters, spellchecker, LanguageTool URL, dictionaries, glossary options, machine translation, autocompleter, external finder, team, and plugins. UI language follows the OS locale and can be overridden in Preferences (41 catalogs; `ar` is right-to-left).

## Command line

```bash
omegat translate <project>
omegat stats <project>
omegat pseudo <project>
omegat search <project> <query>
omegat align --output out.tmx source.txt target.txt
omegat team init <project>
omegat --help
```

Legacy `--mode console-*` flags are accepted. `--no-team` skips repository sync. `--config-dir` overrides `~/.omegat`.

## Team projects

Git, SVN (system `svn`), HTTP, and file mappings from `omegat.project` `repositories` are supported. Sync is prepare → rebase → commit. Same-segment conflicts open in the desktop UI. `--no-team` stays local.

## Scripts

Scripts are JavaScript. Drop a file under `scripts/js/<event>/` or pass `--script`. Event names match the historic set: `application_startup`, `application_shutdown`, `project_changed`, `entry_activated`, `new_file`, `new_word`. Groovy sources are not executed; see `docs/rewrite/MIGRATION.md`.

## Filters and tags

Text, HTML, PO, XLIFF, Office, ODF, PDF (text extract), and the other built-in filters register by extension and XML sniff. Compile-time tag QA reports missing, extra, order, duplicate, malformed, orphaned, and whitespace issues.

## Machine translation and LanguageTool

MT engines are opt-in. Network calls are disabled unless `OMEGAT_MT_NETWORK=1`. Credentials go to the OS keychain or encrypted prefs. LanguageTool is an external HTTP service; if it is down, editing continues.

## Help and license

OmegaT is GNU GPL v3+. Third-party notices: `THIRD_PARTY.md`. Plugin ABI: `docs/rewrite/PLUGIN_ABI.md`.
