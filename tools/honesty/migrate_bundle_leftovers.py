#!/usr/bin/env python3
"""Replace leftover English desktop strings with Bundle_*.properties translations."""

from __future__ import annotations

import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
I18N = ROOT / "apps/desktop/src/renderer/i18n"
BUNDLE_DIR = ROOT / "reference/java/src/main/resources/org/omegat"

KEY_TO_BUNDLE = {
    "glossary": ["TF_OPTIONSMENU_GLOSSARY", "GUI_MATCHWINDOW_SUBWINDOWTITLE_Glossary", "PREFS_TITLE_GLOSSARY"],
    "notes": ["TF_MENU_GOTO_NOTES_PANEL", "GUI_NOTESWINDOW_SUBWINDOWTITLE_Notes"],
    "mt": ["TF_OPTIONSMENU_MACHINETRANSLATE", "PREFS_TITLE_MACHINE_TRANSLATION"],
    "prefs": ["MW_OPTIONSMENU_PREFERENCES", "PREFERENCES_TITLE_NO_SELECTION"],
    "root": ["PFC_OMEGAT_PROJECT", "PP_SAVE_PROJECT_FILE"],
    "replace": ["SW_REPLACE", "BUTTON_REPLACE_ALL"],
    "regex": ["SW_REGEXP_SEARCH"],
    "aligner": ["TF_MENU_TOOLS_ALIGN_FILES"],
    "team": ["TEAM_NEW_HEADER", "TF_MENU_TEAM_DOWNLOAD"],
    "completer": ["PREFS_TITLE_AUTOCOMPLETER", "MW_OPTIONSMENU_AUTOCOMPLETE"],
    "charset": ["AC_CHARTABLE_VIEW", "PREFS_TITLE_AUTOCOMPLETER_CHARTABLE"],
    "historyCompletion": ["AC_HISTORY_COMPLETION_VIEW"],
    "historyPrediction": ["AC_HISTORY_PREDICTION_VIEW"],
    "useDefault": ["TF_MENU_EDIT_DEFAULT_TRANSLATION"],
    "createAlt": ["TF_MENU_EDIT_CREATE_ALT"],
    "sourceFiles": ["TF_MENU_FILE_SOURCE"],
    "colours": ["PREFS_TITLE_COLORS", "MW_OPTIONSMENU_VIEW_COLORS"],
    "savingOutput": ["PREFS_TITLE_SAVING_AND_OUTPUT"],
    "fuzzyThreshold": ["MW_OPTIONSMENU_TM_MATCHES"],
    "markAlt": ["MW_OPTIONSMENU_VIEW_MARK_ALT"],
    "sync": ["TF_MENU_TEAM_SYNC"],
    "finder": ["PREFS_TITLE_EXTERNALFINDER"],
    "autotext": ["PREFS_TITLE_AUTOCOMPLETER_AUTOTEXT"],
    "recent": ["TF_MENU_FILE_RECENT"],
    "editing": ["PREFS_TITLE_EDITOR", "MW_OPTIONSMENU_EDITING"],
    "autosave": ["PREFS_TITLE_SAVING_AND_OUTPUT"],
    "multiple": ["MW_OPTIONSMENU_VIEW_MARK_ALT"],
    "completerAuto": ["PREFS_AUTOCOMPLETER_AUTOMATICALLY"],
    "view": ["MW_VIEW_MENU"],
    "goto": ["TF_MENU_GOTO"],
    "tools": ["TF_MENU_TOOLS"],
    "options": ["TF_MENU_OPTIONS"],
    "help": ["TF_MENU_HELP"],
    "file": ["TF_MENU_FILE"],
    "edit": ["TF_MENU_EDIT"],
}


def parse_bundle(path: Path) -> dict[str, str]:
    out: dict[str, str] = {}
    if not path.is_file():
        return out
    pending_key = None
    pending = ""
    for raw in path.read_text(encoding="utf-8", errors="replace").splitlines():
        if pending_key is not None:
            pending += raw.rstrip("\\")
            if not raw.endswith("\\"):
                out[pending_key] = unescape(pending)
                pending_key = None
                pending = ""
            continue
        if not raw or raw.lstrip().startswith("#") or "=" not in raw:
            continue
        k, _, v = raw.partition("=")
        if v.endswith("\\"):
            pending_key = k.strip()
            pending = v[:-1]
        else:
            out[k.strip()] = unescape(v)
    return out


def unescape(v: str) -> str:
    s = (
        v.replace("\\n", "\n")
        .replace("\\t", "\t")
        .replace("\\:", ":")
        .replace("\\!", "!")
        .replace("\\#", "#")
    )
    def repl(m: re.Match[str]) -> str:
        return chr(int(m.group(1), 16))
    return re.sub(r"\\u([0-9a-fA-F]{4})", repl, s)


def strip_mnemonic(s: str) -> str:
    s = re.sub(r"&(?=\S)", "", s)
    s = re.sub(r"<[^>]+>", "", s)
    return s.strip()


def bundle_path(stem: str) -> Path:
    loc = stem.replace("-", "_")
    if loc == "zh_CN":
        loc = "zh_CN"
    elif loc == "zh_TW":
        loc = "zh_TW"
    elif loc == "pt_BR":
        loc = "pt_BR"
    return BUNDLE_DIR / f"Bundle_{loc}.properties"


def main() -> None:
    en_bundle = parse_bundle(BUNDLE_DIR / "Bundle.properties")
    en = json.loads((I18N / "en.json").read_text(encoding="utf-8"))
    phrases = {v for v in en.values() if isinstance(v, str) and v and v != "OmegaT"}
    changed = 0
    for path in sorted(I18N.glob("*.json")):
        if path.name == "en.json":
            continue
        data = json.loads(path.read_text(encoding="utf-8"))
        bundle = parse_bundle(bundle_path(path.stem))
        if not bundle:
            continue
        for key, val in list(data.items()):
            if not isinstance(val, str) or val == "OmegaT":
                continue
            ev = en.get(key)
            leftover = val in phrases and val != ev
            if not leftover:
                continue
            translated = None
            for bk in KEY_TO_BUNDLE.get(key, []):
                if bk in bundle:
                    translated = strip_mnemonic(bundle[bk])
                    break
            if not translated:
                # invert: find English bundle keys whose value matches leftover
                for bk, bv in en_bundle.items():
                    if strip_mnemonic(bv) == val and bk in bundle:
                        translated = strip_mnemonic(bundle[bk])
                        break
            if translated and translated != val and translated != "OmegaT":
                data[key] = translated
                changed += 1
        path.write_text(json.dumps(data, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(f"migrated {changed} leftover strings from Bundle properties")


if __name__ == "__main__":
    main()
