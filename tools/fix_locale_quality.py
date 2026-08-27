#!/usr/bin/env python3
"""Replace leftover English menu phrases with Bundle translations."""
from __future__ import annotations

import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
I18N = ROOT / "apps/desktop/src/renderer/i18n"
BUNDLE = ROOT / "reference/java/src/main/resources/org/omegat"
ALIGN_BUNDLE = ROOT / "reference/java/aligner/src/main/resources/org/omegat/gui/align"

KEY_MAP = {
    "recent": "TF_MENU_FILE_OPEN_RECENT",
    "clearRecent": "TF_MENU_FILE_CLEAR_RECENT",
}

ALIGN_KEY_MAP = {
    "alignMerge": "ALIGNER_MENU_EDIT_MERGE",
    "alignSplit": "ALIGNER_MENU_EDIT_SPLIT",
    "alignUp": "ALIGNER_MENU_EDIT_MOVEUP",
    "alignDown": "ALIGNER_MENU_EDIT_MOVEDOWN",
}

FALLBACK = {
    "ar": {"recent": "فتح الأخير..."},
    "cy": {"recent": "Agor diweddar..."},
    "da": {"recent": "Åbn seneste..."},
    "el": {"recent": "Άνοιγμα πρόσφατων...", "clearRecent": "Εκκαθάριση μενού"},
    "eo": {"recent": "Malfermi lastatempajn..."},
    "gl": {"recent": "Abrir recentes..."},
    "id": {"recent": "Buka terbaru...", "clearRecent": "Kosongkan proyek terbaru"},
    "ko": {"recent": "최근 항목 열기..."},
    "sh": {"recent": "Otvori nedavne...", "clearRecent": "Obriši nedavne projekte"},
    "sk": {"recent": "Otvoriť nedávne...", "clearRecent": "Vymazať nedávne projekty"},
    "sl": {"recent": "Odpri nedavne...", "clearRecent": "Počisti nedavne projekte"},
    "sq": {"recent": "Hap të fundit..."},
}

ALIGN_FALLBACK = {
    "ar": {"alignMerge": "دمج", "alignSplit": "تقسيم", "alignUp": "تحريك لأعلى", "alignDown": "تحريك لأسفل"},
    "be": {"alignMerge": "Аб’яднаць", "alignSplit": "Падзяліць", "alignUp": "Уверх", "alignDown": "Уніз"},
    "ca": {"alignMerge": "Fusiona", "alignSplit": "Divideix", "alignUp": "Mou amunt", "alignDown": "Mou avall"},
    "co": {"alignMerge": "Fusione", "alignSplit": "Divide", "alignUp": "Move in sù", "alignDown": "Move in ghjò"},
    "cs": {"alignMerge": "Sloučit", "alignSplit": "Rozdělit", "alignUp": "Přesunout nahoru", "alignDown": "Přesunout dolů"},
    "cy": {"alignMerge": "Cyfuno", "alignSplit": "Hollti", "alignUp": "Symud i fyny", "alignDown": "Symud i lawr"},
    "da": {"alignMerge": "Flet", "alignSplit": "Opdel", "alignUp": "Flyt op", "alignDown": "Flyt ned"},
    "de": {"alignMerge": "Zusammenführen", "alignSplit": "Teilen", "alignUp": "Nach oben", "alignDown": "Nach unten"},
    "el": {"alignMerge": "Συγχώνευση", "alignSplit": "Διαίρεση", "alignUp": "Μετακίνηση πάνω", "alignDown": "Μετακίνηση κάτω"},
    "eo": {"alignMerge": "Kunfandi", "alignSplit": "Disigi", "alignUp": "Movi supren", "alignDown": "Movi malsupren"},
    "es": {"alignMerge": "Combinar", "alignSplit": "Dividir", "alignUp": "Mover arriba", "alignDown": "Mover abajo"},
    "eu": {"alignMerge": "Batu", "alignSplit": "Zatitu", "alignUp": "Eraman gora", "alignDown": "Eraman behera"},
    "fi": {"alignMerge": "Yhdistä", "alignSplit": "Jaa", "alignUp": "Siirrä ylös", "alignDown": "Siirrä alas"},
    "fr": {"alignMerge": "Fusionner", "alignSplit": "Scinder", "alignUp": "Monter", "alignDown": "Descendre"},
    "gl": {"alignMerge": "Combinar", "alignSplit": "Dividir", "alignUp": "Mover arriba", "alignDown": "Mover abaixo"},
    "hr": {"alignMerge": "Spoji", "alignSplit": "Podijeli", "alignUp": "Pomakni gore", "alignDown": "Pomakni dolje"},
    "hu": {"alignMerge": "Összevonás", "alignSplit": "Felosztás", "alignUp": "Mozgatás fel", "alignDown": "Mozgatás le"},
    "ia": {"alignMerge": "Fusionar", "alignSplit": "Divider", "alignUp": "Mover in alto", "alignDown": "Mover in basso"},
    "id": {"alignMerge": "Gabung", "alignSplit": "Pisah", "alignUp": "Naik", "alignDown": "Turun"},
    "it": {"alignMerge": "Unisci", "alignSplit": "Dividi", "alignUp": "Sposta su", "alignDown": "Sposta giù"},
    "ja": {"alignMerge": "結合", "alignSplit": "分割", "alignUp": "上へ", "alignDown": "下へ"},
    "ko": {"alignMerge": "병합", "alignSplit": "분할", "alignUp": "위로", "alignDown": "아래로"},
    "mfe": {"alignMerge": "Fusionn", "alignSplit": "Separe", "alignUp": "Monte", "alignDown": "Desann"},
    "nl": {"alignMerge": "Samenvoegen", "alignSplit": "Splitsen", "alignUp": "Omhoog", "alignDown": "Omlaag"},
    "no": {"alignMerge": "Slå sammen", "alignSplit": "Del", "alignUp": "Flytt opp", "alignDown": "Flytt ned"},
    "pl": {"alignMerge": "Scal", "alignSplit": "Podziel", "alignUp": "W górę", "alignDown": "W dół"},
    "pt": {"alignMerge": "Unir", "alignSplit": "Dividir", "alignUp": "Mover para cima", "alignDown": "Mover para baixo"},
    "pt-BR": {"alignMerge": "Mesclar", "alignSplit": "Dividir", "alignUp": "Mover para cima", "alignDown": "Mover para baixo"},
    "ru": {"alignMerge": "Объединить", "alignSplit": "Разделить", "alignUp": "Вверх", "alignDown": "Вниз"},
    "sc": {"alignMerge": "Unire", "alignSplit": "Dividere", "alignUp": "Mòvere in susu", "alignDown": "Mòvere in josso"},
    "sh": {"alignMerge": "Spoji", "alignSplit": "Podeli", "alignUp": "Pomeri gore", "alignDown": "Pomeri dole"},
    "sk": {"alignMerge": "Zlúčiť", "alignSplit": "Rozdeliť", "alignUp": "Posunúť nahor", "alignDown": "Posunúť nadol"},
    "sl": {"alignMerge": "Združi", "alignSplit": "Razdeli", "alignUp": "Premakni gor", "alignDown": "Premakni dol"},
    "sq": {"alignMerge": "Bashko", "alignSplit": "Ndaj", "alignUp": "Lëviz lart", "alignDown": "Lëviz poshtë"},
    "sv": {"alignMerge": "Sammanfoga", "alignSplit": "Dela", "alignUp": "Flytta upp", "alignDown": "Flytta ner"},
    "tk": {"alignMerge": "Birleşdir", "alignSplit": "Böl", "alignUp": "Ýokaryk", "alignDown": "Aşak"},
    "tr": {"alignMerge": "Birleştir", "alignSplit": "Böl", "alignUp": "Yukarı", "alignDown": "Aşağı"},
    "uk": {"alignMerge": "Об’єднати", "alignSplit": "Розділити", "alignUp": "Вгору", "alignDown": "Вниз"},
    "zh-CN": {"alignMerge": "合并", "alignSplit": "拆分", "alignUp": "上移", "alignDown": "下移"},
    "zh-TW": {"alignMerge": "合併", "alignSplit": "分割", "alignUp": "上移", "alignDown": "下移"},
}


def decode_prop_value(raw: str) -> str:
    raw = raw.replace(r"\n", "\n")
    return re.sub(r"\\u([0-9a-fA-F]{4})", lambda m: chr(int(m.group(1), 16)), raw)


def load_bundle_from(dir_path: Path, loc: str) -> dict[str, str]:
    name = f"Bundle_{loc.replace('-', '_')}.properties"
    path = dir_path / name
    if not path.is_file():
        return {}
    out: dict[str, str] = {}
    for line in path.read_text(encoding="latin-1").splitlines():
        line = line.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue
        k, v = line.split("=", 1)
        out[k.strip()] = decode_prop_value(v)
    return out


def load_bundle(loc: str) -> dict[str, str]:
    return load_bundle_from(BUNDLE, loc)


def strip_mnemonic(s: str) -> str:
    return s.replace("&", "").replace("_", "")


def main() -> None:
    for p in sorted(I18N.glob("*.json")):
        if p.name == "en.json":
            continue
        loc = p.stem
        data = json.loads(p.read_text(encoding="utf-8"))
        bundle = load_bundle(loc)
        align = load_bundle_from(ALIGN_BUNDLE, loc)
        changed = False
        for ui_key, bundle_key in KEY_MAP.items():
            if bundle_key in bundle:
                val = strip_mnemonic(bundle[bundle_key]).strip()
                if val and data.get(ui_key) != val:
                    data[ui_key] = val
                    changed = True
            elif loc in FALLBACK and ui_key in FALLBACK[loc]:
                if data.get(ui_key) != FALLBACK[loc][ui_key]:
                    data[ui_key] = FALLBACK[loc][ui_key]
                    changed = True
        for ui_key, bundle_key in ALIGN_KEY_MAP.items():
            if bundle_key in align:
                val = strip_mnemonic(align[bundle_key]).strip()
                if val:
                    data[ui_key] = val
                    changed = True
            elif loc in ALIGN_FALLBACK and ui_key in ALIGN_FALLBACK[loc]:
                data[ui_key] = ALIGN_FALLBACK[loc][ui_key]
                changed = True
            elif ui_key not in data:
                data[ui_key] = ALIGN_FALLBACK.get(loc, {}).get(ui_key) or ui_key
                changed = True
        if changed:
            p.write_text(json.dumps(data, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
            print("updated", p.name)


if __name__ == "__main__":
    main()
