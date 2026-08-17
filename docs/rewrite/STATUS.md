# Rewrite status

| Area | Phase | Status |
|---|---|---|
| Cargo workspace, JSON-RPC, plugin registry | P0 | done |
| taste-skill + DESIGN.md | P0 | done |
| Project / TMX / SRX / matching / glossary / compile / CLI | P1 | done |
| Electron workstation UI | P2 | done |
| Remaining text + simple XML filters | P3 | done |
| Office / ODF / XLIFF2 / SDL / PDF + tag QA | P4 | done (PDF writes sidecar `.txt`) |
| Tokenizers / spell / LT / dictionaries / TM export levels | P5 | done (LT is optional HTTP stub) |
| MT / completer / finder hooks | P6 | done (network MT gated) |
| Team Git/SVN/HTTP/file + TMX rebase | P7 | done (SVN needs system `svn`) |
| Aligner, JS scripts, search/replace, CLI close-out | P8 | done |
| i18n + packaging + retire Java | P9 | in progress |

## Known deltas vs Java OmegaT

- Fuzzy scores use Unicode tokens + light stemming, not the full Lucene analyzer matrix.
- PDF compile cannot rewrite binary PDFs; it writes `*.pdf.txt`.
- LanguageTool is an optional external HTTP service, not an embedded JAR.
- Scripts are JavaScript (Node), not Groovy. Event directory names are unchanged.
- Java plugin JARs are not loaded. New plugins use `omegat-plugin.toml` / JSON manifests.
- Office write-back is a first-pass ZIP/XML rewrite; complex markup may need STATUS follow-up.
