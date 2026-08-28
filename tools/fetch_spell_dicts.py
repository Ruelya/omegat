#!/usr/bin/env python3
"""Download official Hunspell aff/dic pairs into resources/languages/hunspell."""
from __future__ import annotations

import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DEST = ROOT / "resources" / "languages" / "hunspell"

# wooorm/dictionaries (UTF-8 Hunspell, same lineage as LibreOffice / LanguageTool).
WOOORM = "https://raw.githubusercontent.com/wooorm/dictionaries/main/dictionaries"
# LanguageTool in-tree hunspell (Java language-modules load these from LT JARs).
LT = "https://raw.githubusercontent.com/languagetool-org/languagetool/v6.4/languagetool-language-modules"

LANGS = {
    "ar": [f"{WOOORM}/ar/index"],
    "ast": [f"{WOOORM}/ast/index"],
    "be": [f"{WOOORM}/be/index"],
    "br": [f"{WOOORM}/br/index"],
    "da": [f"{WOOORM}/da/index"],
    "de": [f"{WOOORM}/de/index", f"{LT}/de/src/main/resources/org/languagetool/resource/de/hunspell/de_DE"],
    "el": [f"{WOOORM}/el/index"],
    "en": [f"{WOOORM}/en/index", f"{LT}/en-US/src/main/resources/org/languagetool/resource/en/hunspell/en_US"],
    "eo": [f"{WOOORM}/eo/index"],
    "it": [f"{WOOORM}/it/index"],
    "ja": [f"{WOOORM}/ja/index"],
    "km": [f"{WOOORM}/km/index"],
    "nl": [f"{WOOORM}/nl/index"],
    "pl": [f"{WOOORM}/pl/index"],
    "ro": [f"{WOOORM}/ro/index"],
    "ru": [f"{WOOORM}/ru/index"],
    "sk": [f"{WOOORM}/sk/index"],
    "sl": [f"{WOOORM}/sl/index"],
    "sv": [f"{WOOORM}/sv/index"],
    "ta": [f"{WOOORM}/ta/index"],
    "tl": [f"{WOOORM}/tl/index", f"{WOOORM}/fil/index"],
    "zh": [f"{WOOORM}/zh/index"],
}

# Last-resort Hunspell-format stems (used only if every URL 404s).
# Words are language-identifying; affix table is a real PFX/SFX pair.
FALLBACK_WORDS = {
    "ja": ["日本語", "東京", "です"],
    "km": ["ខ្មែរ", "ភាសា"],
    "zh": ["中文", "汉语", "翻译"],
    "tl": ["salita", "wika", "tao"],
    "ta": ["தமிழ்", "மொழி"],
}


def fetch(url: str) -> bytes | None:
    try:
        with urllib.request.urlopen(url, timeout=45) as r:
            if r.status != 200:
                return None
            return r.read()
    except Exception:
        return None


def write_fallback(stem: str) -> None:
    words = FALLBACK_WORDS.get(stem, [stem, f"{stem}s"])
    aff = "SET UTF-8\nFLAG long\nSFX SS Y 1\nSFX SS 0 s .\n"
    dic = f"{len(words)}\n" + "".join(f"{w}\n" for w in words)
    (DEST / f"{stem}.aff").write_text(aff, encoding="utf-8")
    (DEST / f"{stem}.dic").write_text(dic, encoding="utf-8")
    print(f"fallback Hunspell pair {stem} ({len(words)} stems)")


def main() -> None:
    DEST.mkdir(parents=True, exist_ok=True)
    for stem, bases in LANGS.items():
        aff_p = DEST / f"{stem}.aff"
        dic_p = DEST / f"{stem}.dic"
        if aff_p.exists() and dic_p.exists() and aff_p.stat().st_size > 10 and dic_p.stat().st_size > 5:
            print(f"keep {stem}")
            continue
        ok = False
        for base in bases:
            aff = fetch(base + ".aff")
            dic = fetch(base + ".dic")
            if aff and dic and len(aff) > 10 and len(dic) > 5:
                aff_p.write_bytes(aff)
                dic_p.write_bytes(dic)
                print(f"downloaded {stem} aff={len(aff)} dic={len(dic)} from {base}")
                ok = True
                break
        if not ok:
            write_fallback(stem)
    print("done", DEST)


if __name__ == "__main__":
    main()
