# Spell dictionaries

The previous 30 toy `.dic` word lists (no `.aff`) were removed.

Product Hunspell pairs live in `reference/java/language-modules` for
**ca / es / fa / fr / ga / gl / pt / uk**. `spell.install` / `ensure_lang`
copies them into `config/spell/hunspell` on first use.

CI uses the small affix fixtures under `fixtures/spell/{hunspell,lucene,morfologik}`.
Those three backends must keep distinct word lists.

## Missing affix pairs (`parity_gap`)

These `language-modules` have no `.aff`/`.dic` in-tree. Download into
`config/spell` at first use, or ship a system Hunspell dictionary:

ar, ast, be, br, da, de, el, en, eo, it, ja, km, nl, pl, ro, ru, sk, sl,
sv, ta, tl, zh.
