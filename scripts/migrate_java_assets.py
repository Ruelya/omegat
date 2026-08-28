#!/usr/bin/env python3
"""Migrate UI strings and compact spell lists from reference/java."""

from __future__ import annotations

import json
import pathlib
import re
import unicodedata

ROOT = pathlib.Path(__file__).resolve().parents[1]
BUNDLE_DIR = ROOT / "reference/java/src/main/resources/org/omegat"
I18N_DIR = ROOT / "apps/desktop/src/renderer/i18n"
LANG_DIR = ROOT / "resources/languages"
MT_DIR = ROOT / "fixtures/mt"

KEY_MAP = {
    "save": "TF_MENU_FILE_SAVE",
    "compile": "TF_MENU_FILE_COMPILE",
    "openProject": "TF_MENU_FILE_OPEN",
    "newProject": "TF_MENU_FILE_CREATE",
    "cancel": "BUTTON_CANCEL",
    "search": "BUTTON_SEARCH",
    "replace": "BUTTON_REPLACE_ALL",
    "prefs": "MW_OPTIONSMENU_PREFERENCES",
    "about": "TF_MENU_HELP_ABOUT",
    "editor": "TF_MENU_GOTO_EDITOR_PANEL",
    "matches": "GUI_MATCHWINDOW_SUBWINDOWTITLE_Fuzzy_Matches",
    "glossary": "TF_OPTIONSMENU_GLOSSARY",
    "notes": "GUI_NOTESWINDOW_SUBWINDOWTITLE_Notes",
    "comments": "GUI_COMMENTSWINDOW_SUBWINDOWTITLE_Comments",
    "mt": "TF_OPTIONSMENU_MACHINETRANSLATE",
    "dict": "TF_OPTIONSMENU_DICTIONARY",
    "issues": "TF_MENU_TOOLS_CHECK_ISSUES",
    "aligner": "TF_MENU_TOOLS_ALIGN_FILES",
    "team": "TF_MENU_FILE_TEAM_CREATE",
    "filters": "PREFS_TITLE_SOURCE_FILES",
    "segmentation": "MW_OPTIONSMENU_GLOBAL_SENTSEG",
    "spell": "PREFS_TITLE_SPELLCHECKER",
    "create": "BUTTON_ADD_NODOTS",
    "sourceLang": "PP_SRC_LANG",
    "targetLang": "PP_LOC_LANG",
    "sentenceSeg": "PP_SENTENCE_SEGMENTING",
    "uiLanguage": "PREFS_TITLE_GENERAL",
    "sync": "TEAM_SYNCHRONIZE",
    "general": "PREFS_TITLE_GENERAL",
    "appearance": "PREFS_TITLE_APPEARANCE",
    "editing": "PREFS_TITLE_EDITING_BEHAVIOR",
    "view": "PREFS_TITLE_VIEW_OPTIONS",
    "plugins": "PREFS_TITLE_PLUGINS",
    "learn": "BUTTON_ADD_NODOTS",
    "ignore": "BUTTON_REMOVE",
    "regex": "SW_REGEXP_SEARCH",
    "files": "PF_WINDOW_TITLE",
    "source": "PP_SRC_ROOT",
    "target": "PP_LOC_ROOT",
    "properties": "MW_PROJECTMENU_EDIT",
    "multiple": "MW_VIEW_MENU_MARK_ALT_TRANSLATIONS",
    "fuzzyThreshold": "GUI_WORKFLOW_OPTION_Minimal_Similarity",
    "autosave": "PREFS_TITLE_SAVING_AND_OUTPUT",
}

LOCALE_FILES = {
    "ar": "Bundle_ar.properties",
    "be": "Bundle_be.properties",
    "ca": "Bundle_ca.properties",
    "co": "Bundle_co.properties",
    "cs": "Bundle_cs.properties",
    "cy": "Bundle_cy.properties",
    "da": "Bundle_da.properties",
    "de": "Bundle_de.properties",
    "el": "Bundle_el.properties",
    "en": None,
    "eo": "Bundle_eo.properties",
    "es": "Bundle_es.properties",
    "eu": "Bundle_eu.properties",
    "fi": "Bundle_fi.properties",
    "fr": "Bundle_fr.properties",
    "gl": "Bundle_gl.properties",
    "hr": "Bundle_hr.properties",
    "hu": "Bundle_hu.properties",
    "ia": "Bundle_ia.properties",
    "id": "Bundle_id.properties",
    "it": "Bundle_it.properties",
    "ja": "Bundle_ja.properties",
    "ko": "Bundle_ko.properties",
    "mfe": "Bundle_mfe.properties",
    "nl": "Bundle_nl.properties",
    "no": "Bundle_no.properties",
    "pl": "Bundle_pl.properties",
    "pt": "Bundle_pt.properties",
    "pt-BR": "Bundle_pt_BR.properties",
    "ru": "Bundle_ru.properties",
    "sc": "Bundle_sc.properties",
    "sh": "Bundle_sh.properties",
    "sk": "Bundle_sk.properties",
    "sl": "Bundle_sl.properties",
    "sq": "Bundle_sq.properties",
    "sv": "Bundle_sv.properties",
    "tk": "Bundle_tk.properties",
    "tr": "Bundle_tr.properties",
    "uk": "Bundle_uk.properties",
    "zh-CN": "Bundle_zh_CN.properties",
    "zh-TW": "Bundle_zh_TW.properties",
}

FALLBACK_KEYS = {
    "PP_SRC_LANG": "PP_SOURCE_LANG",
    "PP_LOC_LANG": "PP_TARGET_LANG",
    "PP_SENTENCE_SEGMENTING": "PP_SENTENCE_SEG",
    "PREFS_TITLE_GENERAL": "MW_OPTIONSMENU",
    "TEAM_MENU_SYNCHRONIZE": "TF_MENU_FILE_TEAM_CREATE",
    "TF_OPTIONSMENU_SPELLCHECKER": "MW_OPTIONSMENU_PREFERENCES",
    "TF_MENU_HELP_ABOUT": "TF_MENU_HELP",
}


def unescape_java(s: str) -> str:
    s = s.replace(r"\n", " ").replace(r"\t", " ")
    def repl(m):
        return chr(int(m.group(1), 16))
    s = re.sub(r"\\u([0-9a-fA-F]{4})", repl, s)
    s = s.replace("&", "")
    s = re.sub(r"\s+", " ", s).strip()
    s = re.sub(r"\s*\([A-Za-z0-9]\)\s*$", "", s)
    s = s.rstrip(".").rstrip("…").rstrip("...")
    return s


def parse_bundle(path: pathlib.Path) -> dict[str, str]:
    out: dict[str, str] = {}
    if not path.exists():
        return out
    pending = None
    buf = ""
    for raw in path.read_text(encoding="utf-8", errors="replace").splitlines():
        if pending:
            buf += raw.rstrip("\\")
            if not raw.endswith("\\"):
                out[pending] = unescape_java(buf)
                pending = None
                buf = ""
            continue
        if not raw or raw.startswith("#") or "=" not in raw:
            continue
        k, v = raw.split("=", 1)
        if raw.endswith("\\"):
            pending = k
            buf = v.rstrip("\\")
        else:
            out[k] = unescape_java(v)
    return out


def lookup(bundle: dict[str, str], key: str) -> str | None:
    if key in bundle:
        return bundle[key]
    alt = FALLBACK_KEYS.get(key)
    if alt and alt in bundle:
        return bundle[alt]
    return None


def migrate_locales() -> None:
    en_path = I18N_DIR / "en.json"
    en = json.loads(en_path.read_text(encoding="utf-8"))
    en_bundle = parse_bundle(BUNDLE_DIR / "Bundle.properties")
    for loc, fname in LOCALE_FILES.items():
        dest = I18N_DIR / f"{loc}.json"
        existing = json.loads(dest.read_text(encoding="utf-8")) if dest.exists() else {}
        merged = dict(en)
        merged.update(existing)
        if fname:
            bundle = parse_bundle(BUNDLE_DIR / fname)
            for our, java in KEY_MAP.items():
                val = lookup(bundle, java) or lookup(en_bundle, java)
                if val:
                    merged[our] = val
        dest.write_text(json.dumps(merged, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        print("locale", loc, "keys", len(merged))


def compact_dic(src: pathlib.Path, dest: pathlib.Path, limit: int = 80) -> None:
    words: list[str] = []
    try:
        text = src.read_text(encoding="utf-8", errors="replace")
    except Exception:
        return
    for i, line in enumerate(text.splitlines()):
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        if i == 0 and line.isdigit():
            continue
        word = re.split(r"[/\t ]", line)[0]
        if word and word not in words:
            words.append(word)
        if len(words) >= limit:
            break
    if not words:
        return
    dest.write_text(f"{len(words)}\n" + "\n".join(words) + "\n", encoding="utf-8")


def migrate_spell() -> None:
    LANG_DIR.mkdir(parents=True, exist_ok=True)
    mods = ROOT / "reference/java/language-modules"
    if not mods.exists():
        return
    for lang_dir in sorted(mods.iterdir()):
        if not lang_dir.is_dir():
            continue
        dics = list(lang_dir.rglob("*.dic"))
        if not dics:
            continue
        dest = LANG_DIR / f"{lang_dir.name}.dic"
        compact_dic(dics[0], dest)
        print("spell", dest.name)


def write_mt_fixtures() -> None:
    MT_DIR.mkdir(parents=True, exist_ok=True)
    fixtures = {
        "mymemory.json": {"responseData": {"translatedText": "Bonjour le monde"}},
        "mymemory-human.json": {"responseData": {"translatedText": "Bonjour le monde"}},
        "google.json": {"data": {"translations": [{"translatedText": "Hola"}]}},
        "ibmwatson.json": {"translations": [{"translation": "Bonjour"}]},
        "apertium.json": {"responseData": {"translatedText": "Bonjour"}},
        "yandex.json": {"translations": [{"text": "Bonjour"}]},
        "belazar.json": {"text": "Прывітанне"},
    }
    for name, obj in fixtures.items():
        (MT_DIR / name).write_text(json.dumps(obj, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    migrate_locales()
    migrate_spell()
    write_mt_fixtures()
