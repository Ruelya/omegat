# OmegaT user manual

OmegaT is a keyboard-first CAT workstation. This manual describes the Rust + Electron build. The historic Java HTML manual still lives under `reference/java` (generated DocBook / `release/index.html`) and can be opened from Help when bundled.

## Install

- **From source:** install Rust stable and Node.js 22, then follow the repository `README.md`.
- **Linux:** CI produces unsigned `deb`, `rpm`, `tar.gz`, and a `dir` tree via `electron-builder`.
- **Windows:** unsigned NSIS installer (`pack-windows` job).
- **macOS:** unsigned DMG (`pack-macos` job). A release manager signs and notarizes outside CI.

The engine is a sidecar binary (`omegat-sidecar`). The desktop shell never reads project files itself. UI language follows the OS locale (41 catalogs). Arabic (`ar`) is right-to-left; native menus use the same catalogs.

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

- The editor shows source and target for the current segment. Tags are protected; view marks (whitespace, NBSP, bidi, glossary, TM/MT origin) follow Preferences.
- **Enter** commits the segment and moves forward.
- **Ctrl/Cmd+I** inserts the best fuzzy match. Fuzzy 1–5 have menu accelerators.
- **Ctrl/Cmd+N** / **Ctrl/Cmd+P** move to the next/previous segment.
- **Ctrl/Cmd+S** saves `project_save.tmx` (plus `.bak`).
- **Ctrl/Cmd+D** compiles into `target/`.
- **Ctrl/Cmd+F** opens search (exact / keyword / regex, notes, comments, author, dates, replace preview).

Nine docks: Editor, Matches, Glossary, Dictionary, Machine translation, Notes, Comments, Multiple translations, Segment properties. Files and Issues are separate windows. Layout is persisted.

## Preferences

Twenty-five pages write keys the sidecar consumes: general, appearance, fonts, colours, saving, editing, TM matches, view, source files, filters, segmentation, shortcuts, spellchecker, LanguageTool, dictionary, glossary, machine translation, autocompleter (glossary / autotext / character table / history completion / history prediction), external finder, team, secure store, version check, plugins. Changing the UI language rebuilds the native menu.

## Command line

```bash
omegat translate <project>
omegat stats <project>
omegat pseudo <project>
omegat search <project> <query>
omegat align --alignDir <dir> --output out.tmx source.txt target.txt
omegat team init <project>
omegat script path.js --project <dir>
omegat --help
```

Legacy flags: `--mode`, `--no-team`, `--config-dir`, `--config-file`, `--resource-bundle`, `--disable-project-locking`, `--disable-location-save`, `--source-pattern`, `--pseudotranslatetmx`, `--pseudotranslatetype`, `--alignDir`, `--output-file`, `--stats-type`, `--script`, `--tag-validation abort|warn`.

## Team projects

Four repository types: file, HTTP (real download), git, SVN. `omegat.project` `<mapping>` / includes / excludes are applied. Working copies live under `.repositories/<sanitized-url>/`. Sync is **prepare → rebase (TMX and glossary) → commit/push**. Same-segment conflicts keep both sides; the desktop dialog offers Keep ours / Keep theirs / manual. `--no-team` stays local. Two-client git tests cover merge and conflict.

## Aligner

mALIGNa modes: HEAPWISE (filter extract + SRX + length HMM), PARSEWISE (same filter on both sides), ID (segment id). Viterbi is min-cost; Forward-Backward is a posterior path (not an alias). CHAR/WORD counters with Normal/Poisson calculators. The GUI table can merge, split, move rows, and export TMX that R1 can open.

## Scripts

JavaScript bindings match Java `AbstractScriptRunner`: `project`, `editor`, `glossary`, `console`, `mainWindow`, `Core`. Callable methods include current-segment read/write, insert/overwrite, jump, save, compile, glossary add/query, `console.println`. Six event directories and twelve slots. `--script` on the CLI. Groovy is not executed; see `docs/rewrite/MIGRATION.md`.

## Filters, tags, Wiki, MED

The 49 Java filter classes (plus extra JSON/CSV/Markdown) register by dialect and options. Compile-time tag QA reports missing, extra, order, duplicate, malformed, orphaned, and whitespace issues. Wiki import reads MediaWiki XML pages into `source/`. MED packages are zip archives unpacked onto a project tree.

## Machine translation, finder, autocompleter

Seven engines (Google v2, IBM Watson, MyMemory machine/human, Apertium, Yandex Cloud, Belazar) use the Java URL/auth headers. Credentials go to the OS keychain or encrypted prefs. Recorded HTTP fixtures live under `fixtures/mt/<engine>/`. External Finder reads the existing finder XML. Autocompleter classes: Glossary, Autotext, Character table, History completer, History predictor (next-word model), Tags.

## LanguageTool, spelling, dictionaries

LanguageTool is HTTP `v2/check`. If no URL is set, Issues shows a `severity=info` downgrade item — never an empty “clean” list. Hunspell reads `.aff`/`.dic` (real files from `reference/java/language-modules`, or download-on-first-use). Lucene-Hunspell and Morfologik use different resource paths. StarDict (`.ifo`/`.idx`/`.dict`/`.dict.dz`) and DSL (including `.dsl.dz`) are supported.

## Plugins

Java JAR plugins are not loaded. A plugin is `omegat-plugin.toml` + a `cdylib` that exports `omegat_plugin_register` and registers Filter / MT / Tokenizer callbacks. The example plugin (`crates/omegat-example-plugin`) appears in `filters.list` and parses `fixtures/plugin/sample.example`. See `docs/rewrite/PLUGIN_ABI.md`.

## Help and license

OmegaT is GNU GPL v3+. Third-party notices: `THIRD_PARTY.md`. This Markdown manual is shipped in the package (`docs/manual`). The Java HTML set under `reference/java` remains the long-form reference until it is fully ported.
