# Spell dictionaries

The previous 30 toy `.dic` word lists (no `.aff`) were removed in G6.

Product Hunspell pairs live in `reference/java/language-modules` for
**ca / es / fa / fr / ga / gl / pt / uk**. `spell.install` / `ensure_lang`
copies them into `config/spell/hunspell` on first use.

CI uses the small affix fixtures under `fixtures/spell/{hunspell,lucene,morfologik}`.
Those three backends must keep distinct word lists.

Languages without an affix pair in the tree (en, de, ja, …) stay
`parity_gap`: download into `config/spell` at first use, or ship a
system Hunspell dictionary.
