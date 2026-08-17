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
| Desktop docks, menus, search/replace, preference pages | R3 | parity |
| Spell backends, dictionaries, LanguageTool HTTP, Issues | R4 | parity |
| 7 MT engines, External Finder, autocompleter | R5 | parity |
| Team sync + rebase + conflict UI | R6 | parity |
| Aligner, scripts, Wiki/MED/convert, CLI close-out | R7 | parity |
| 41 authored UI locales, packaging, plugin cdylib load | R8 | scaffold |

## Quantified remaining deltas

- R1 matching: token-Levenshtein scores on committed `fixtures/goldens/engine/fuzzy.json` are exact (en `Hello world`/`Hello word` = 50). CJK n-gram vs Lucene CJKTokenizer not yet measured on a held-out set — record when R4 tokenizers land.
- R1 TMX: `omegat`/`level1`/`level2` fields (`changeid`, `creationid`, dates, `prop` file/id, level2 `bpt`/`ept`/`ph`) are asserted in unit tests; Java `project_save.tmx` fixture round-trips entry count. Whitespace-only serialization differences vs StAX are allowed.
- R2 XML: inline tags become OmegaT shortcuts (`<f0>`, `<x0>` for `xliff:g`). Empty write keeps the original tree (Java `translateXML`). Office write replaces `w:t` / `text:p` node ranges, not file-wide `replacen`. `sniff_xml` no longer defaults unknown XML to Android.
- R3 desktop: nine docks follow `DockingDefaults.xml` ratios (persisted as `extra.docking_layout`); 25 preference pages write keys the sidecar or renderer consume; search RPC includes notes/comments/keyword/author/date/preview. Native menu maps Java `MainMenuShortcuts.properties`.
- R5: each of the 7 engines has `fixtures/mt/<id>/recorded.json` (request URL/headers + response). Auth headers match Java (`X-HTTP-Method-Override`, Watson Basic + opt-out, Yandex Bearer). History predictor is a next-word model, not a translation-prefix scan. Offline without a fixture is an error and does not invent a translation.
- R4 spell: CI uses `fixtures/spell/{hunspell,lucene,morfologik}` (affix-expanded `walk`/`walks`). Full ca/es/fa/fr/ga/gl/pt/uk `aff`/`dic` stay in `reference/java/language-modules` and copy via `spell.install` — not vendored (uk.dic 8.2M). en/de have no language-module pair. LT without `languagetool_url` emits one `severity=info` issue, never an empty “clean” list.
- R7 aligner: HEAPWISE is filter-extract + SRX + length HMM (not `split_whitespace`). PARSEWISE / ID match Java `AlignerTest` fixtures under `fixtures/align`. Viterbi is min-cost; Forward-Backward is a posterior path with 1-0/0-1 (not an alias). CHAR/WORD + Normal/Poisson calculators. GUI table supports merge/split/up/down via `align.edit`. Scripts bind `project`/`editor`/`glossary`/`console`/`mainWindow`/`Core`; `entry_activated` can change the current translation. MED unzips to a project tree. `omegat --help` lists Java legacy flags including `--alignDir` and `--mode`.
- R6 team: `sync` is prepare → rebase (TMX **and** glossary) → commit/push. Mappings + includes/excludes are parsed from `omegat.project` and applied. Git uses `.repositories/<sanitized-url>/` as the mapped working copy (`fetch` + `reset --hard` to origin, then copy, commit, `push`). HTTP downloads the URL (`file://` or `curl`) into rebase. File repos copy the tree. Two-client git tests: different segments merge; same segment yields `{source,ours,theirs}` and `team.resolve`. SVN checkout/update/commit is implemented; the integration test no-ops when `svn`/`svnadmin` are missing (this environment has neither).
- See later waves as they leave `scaffold`. Do not mark `parity` while tests use `>= N` or skip read-back of translations.

## Sidecar methods

Present and contract-tested: `sys.version`, `sys.capabilities`, `sys.plugins`, `prefs.get`, `prefs.set`, `project.create|open|close|save|compile|reload|props|convert`, `entry.list|get|set`, `matches.query`, `glossary.query|add`, `search.run`, `search.replace`, `stats.get`, `issues.list`, `filters.list`, `filters.options`, `mt.query`, `dict.query`, `completer.query`, `spell.learn`, `spell.ignore`, `spell.install`, `tmx.export`, `languagetool.check`, `finder.run`, `team.sync`, `team.commit`, `team.conflicts`, `team.resolve`, `script.run`, `script.slots`, `align.run`, `align.edit`, `aligner.configure`, `wiki.import`, `med.open`.

## Intentional non-goals (not feature cuts)

- Java JAR plugins are not loaded. Replacement: `omegat-plugin.toml` + cdylib (`docs/rewrite/PLUGIN_ABI.md`).
- Groovy is not executed. Replacement: JS event hooks with the same binding surface (`docs/rewrite/MIGRATION.md`).
- LanguageTool is not an embedded JAR. Replacement: HTTP `v2/check`.
- PDF write-back matches Java `PdfFilter` (text extract; sidecar `.txt`).
