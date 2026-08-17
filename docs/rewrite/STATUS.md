# Rewrite status (parity)

Legend: `scaffold` = present but not accepted · `parity` = accepted against Java fixtures · `parity_gap` = specified, quantified remaining delta.

| Area | Wave | Status |
|---|---|---|
| Java reference tree at `reference/java` | R0 | parity |
| Filter / align / SRX fixtures under `fixtures/` | R0 | parity |
| Golden export tool + committed `fixtures/goldens/` | H0 | parity |
| Sidecar method contract tests | R0 | parity |
| Project / TMX / SRX / matching / glossary / compile / CLI | R1 | parity |
| 49 filters + Office write-back + tag QA | R2 | parity |
| Desktop docks, menus, search/replace, preference pages | R3 | scaffold |
| Spell backends, dictionaries, LanguageTool HTTP, Issues | R4 | scaffold |
| 7 MT engines, External Finder, autocompleter | R5 | scaffold |
| Team sync + rebase + conflict UI | R6 | scaffold |
| Aligner, scripts, Wiki/MED/convert, CLI close-out | R7 | scaffold |
| 41 authored UI locales, packaging, plugin cdylib load | R8 | scaffold |

## Quantified remaining deltas

- R1 matching: token-Levenshtein scores on committed `fixtures/goldens/engine/fuzzy.json` are exact (en `Hello world`/`Hello word` = 50). CJK n-gram vs Lucene CJKTokenizer not yet measured on a held-out set — record when R4 tokenizers land.
- R1 TMX: `omegat`/`level1`/`level2` fields (`changeid`, `creationid`, dates, `prop` file/id, level2 `bpt`/`ept`/`ph`) are asserted in unit tests; Java `project_save.tmx` fixture round-trips entry count. Whitespace-only serialization differences vs StAX are allowed.
- R2 XML: inline tags become OmegaT shortcuts (`<f0>`, `<x0>` for `xliff:g`). Empty write keeps the original tree (Java `translateXML`). Office write replaces `w:t` / `text:p` node ranges, not file-wide `replacen`. `sniff_xml` no longer defaults unknown XML to Android.
- See later waves as they leave `scaffold`. Do not mark `parity` while tests use `>= N` or skip read-back of translations.

## Sidecar methods

Present and contract-tested: `sys.version`, `sys.capabilities`, `sys.plugins`, `prefs.get`, `prefs.set`, `project.create|open|close|save|compile|props|convert`, `entry.list|get|set`, `matches.query`, `glossary.query|add`, `search.run`, `search.replace`, `stats.get`, `issues.list`, `filters.list`, `filters.options`, `mt.query`, `dict.query`, `completer.query`, `spell.learn`, `spell.ignore`, `tmx.export`, `languagetool.check`, `finder.run`, `team.sync`, `team.conflicts`, `script.run`, `script.slots`, `align.run`, `aligner.configure`, `wiki.import`, `med.open`.

## Intentional non-goals (not feature cuts)

- Java JAR plugins are not loaded. Replacement: `omegat-plugin.toml` + cdylib (`docs/rewrite/PLUGIN_ABI.md`).
- Groovy is not executed. Replacement: JS event hooks with the same binding surface (`docs/rewrite/MIGRATION.md`).
- LanguageTool is not an embedded JAR. Replacement: HTTP `v2/check`.
- PDF write-back matches Java `PdfFilter` (text extract; sidecar `.txt`).
