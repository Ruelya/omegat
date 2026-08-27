# Spell fixtures

CI Hunspell/Lucene/Morfologik dictionaries. They are small on purpose.

- `hunspell/` — real `.aff` suffix rules (`walk` → `walks`/`walking`/`walked`) plus en/fr/de stems.
- `lucene/` — different word list (`color`, not `colour`) so the three backends disagree on the same misspelling set.
- `morfologik/` — `.dict.txt` word list (`kolor`).

Full Hunspell `aff`/`dic` pairs for ca/es/fa/fr/ga/gl/pt/uk stay in `reference/java/language-modules`. Call `spell.install` (or `omegat_core::spell::ensure_lang`) to copy them into `config/spell/hunspell` on first use. Do not vendor the multi-megabyte `.dic` files into this tree.
