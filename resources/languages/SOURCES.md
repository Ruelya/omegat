# Hunspell pair provenance

`ensure_lang` copies `{stem}.aff` + `{stem}.dic` from this folder (or from
`reference/java/language-modules` for ca/es/fa/fr/ga/gl/pt/uk).

| Stem | Source | Notes |
|---|---|---|
| br, da, de, el, en, eo, it, nl, pl, ro, ru, sk, sl, sv | wooorm/dictionaries `index.aff` / `index.dic` | official Hunspell; `.dic` kept to 2000 stems in-tree |
| ar | LanguageTool v6.4 `resource/ar/hunspell/ar` | `.dic` kept to 2000 stems |
| ta | LibreOffice dictionaries `ta_IN` | `.dic` kept to 2000 stems |
| ast, be, tl, ja, km, zh | Hunspell-format UTF-8 stem lists | Java modules load LanguageTool JARs; no in-tree aff/dic there either |

Full upstream dictionaries remain downloadable from those URLs. CI affix
logic still uses `fixtures/spell/{hunspell,lucene,morfologik}`.
