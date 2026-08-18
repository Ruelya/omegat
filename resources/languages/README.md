# Spell dictionaries

Product Hunspell pairs:

- `reference/java/language-modules` ships **ca / es / fa / fr / ga / gl / pt / uk**.
- `resources/languages/hunspell` ships the other language-module stems
  (**ar, ast, be, br, da, de, el, en, eo, it, ja, km, nl, pl, ro, ru, sk, sl,
  sv, ta, tl, zh**). See `SOURCES.md`. Official wooorm / LanguageTool /
  LibreOffice pairs are truncated to 2000 `.dic` stems in-tree. `ast` / `be` /
  `tl` / `ja` / `km` / `zh` are Hunspell-format UTF-8 stem lists (Java has no
  in-tree aff/dic for those modules either).

`spell.install` / `ensure_lang` copies a stem into `config/spell/hunspell`.

CI affix tests still use the small `fixtures/spell/{hunspell,lucene,morfologik}`
sets so the three backends keep distinct word lists.
