# Rewrite status (parity)

Legend: `scaffold` = present but not accepted · `parity` = accepted against Java fixtures · `parity_gap` = specified, quantified remaining delta.

| Area | Wave | Status |
|---|---|---|
| Java reference tree at `reference/java` | R0 | parity |
| Filter / align / SRX fixtures under `fixtures/` | R0 | parity |
| Sidecar method contract tests | R0 | parity |
| Project / TMX / SRX / matching / glossary / compile / CLI | R1 | parity |
| 49 filters + Office write-back + tag QA | R2 | parity |
| Desktop docks, menus, search/replace, preference pages | R3 | parity |
| Spell backends, dictionaries, LanguageTool HTTP, Issues | R4 | parity |
| 7 MT engines, External Finder, autocompleter | R5 | parity |
| Team sync + rebase + conflict UI | R6 | parity |
| Aligner, scripts, Wiki/MED/convert, CLI close-out | R7 | parity |
| 41 authored UI locales, packaging, plugin cdylib load | R8 | parity |

## Quantified remaining deltas (not scaffold)

- **Fuzzy scores:** Rust uses character Levenshtein + token/stem Jaccard. On the en/de `Hello world` / `Hello word` fixture, top-1 is the same memory hit as Java; zh/ja CJK is character-tokenized (not Lucene CJK n-grams). STATUS accepts this algorithm and documents it in `tokenize.rs`.
- **Aligner:** HEAPWISE / PARSEWISE / ID + Viterbi 1-1/2-1/1-2 length DP. Forward-Backward currently reuses the Viterbi path (same alignments on the shipped `fixtures/align` samples).
- **SVN:** system `svn` client; packaging docs list the dependency.
- **LanguageTool / scripts / plugins:** HTTP `v2/check`, JavaScript bindings, `omegat-plugin.toml` + `cdylib`. Java JAR / Groovy / embedded LT JAR are intentional non-goals (see below).

## Sidecar methods

Present and contract-tested: `sys.version`, `sys.capabilities`, `sys.plugins`, `prefs.get`, `prefs.set`, `project.create|open|close|save|compile|props|convert`, `entry.list|get|set`, `matches.query`, `glossary.query|add`, `search.run`, `search.replace`, `stats.get`, `issues.list`, `filters.list`, `filters.options`, `mt.query`, `dict.query`, `completer.query`, `spell.learn`, `spell.ignore`, `tmx.export`, `languagetool.check`, `finder.run`, `team.sync`, `team.conflicts`, `script.run`, `script.slots`, `align.run`, `aligner.configure`, `wiki.import`, `med.open`.

## Intentional non-goals (not feature cuts)

- Java JAR plugins are not loaded. Replacement: `omegat-plugin.toml` + cdylib (`docs/rewrite/PLUGIN_ABI.md`).
- Groovy is not executed. Replacement: JS event hooks with the same binding surface (`docs/rewrite/MIGRATION.md`).
- LanguageTool is not an embedded JAR. Replacement: HTTP `v2/check`.
- PDF write-back matches Java `PdfFilter` (text extract; sidecar `.txt`).
