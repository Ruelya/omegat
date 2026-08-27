#!/usr/bin/env python3
"""Remap desktop i18n JSON from Java Bundle_*.properties (no (loc) suffixes)."""
from __future__ import annotations

import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
I18N = ROOT / "apps/desktop/src/renderer/i18n"
BUNDLE_DIR = ROOT / "reference/java/src/main/resources/org/omegat"

# UI key → Java Bundle key (first hit wins if list)
KEY_MAP: dict[str, list[str]] = {
    "app": ["application-name"],
    "openProject": ["TF_MENU_FILE_OPEN", "TF_MENU_NEWUI_PROJECT_GO"],
    "newProject": ["TF_MENU_FILE_CREATE"],
    "recent": ["TF_MENU_FILE_OPEN_RECENT"],
    "sourceLang": ["TF_SRC_LANG", "SOURCE_LANGUAGE"],
    "targetLang": ["TF_TGT_LANG", "TARGET_LANGUAGE"],
    "sentenceSeg": ["TF_SENTENCE_SEGMENTING", "PP_SENTENCE_SEGMENTING"],
    "create": ["BUTTON_ADD_NODOTS", "TF_MENU_FILE_CREATE"],
    "cancel": ["BUTTON_CANCEL"],
    "files": ["TF_MENU_FILE_PROJWIN", "TF_NOTICE_SOURCE_FILES"],
    "editor": ["GUI_MATCHWINDOW_SUBWINDOWTITLE_Editor", "TF_MENU_DISPLAY"],
    "source": ["TF_CUR_FILE_SRC", "TF_SRC_FILE"],
    "target": ["TF_CUR_FILE_TGT", "TF_TGT_FILE"],
    "matches": ["GUI_MATCHWINDOW_SUBWINDOWTITLE_Fuzzy_Matches"],
    "glossary": ["GUI_GLOSSARYWINDOW_SUBWINDOWTITLE_Glossary"],
    "notes": ["GUI_NOTESWINDOW_SUBWINDOWTITLE_Notes"],
    "comments": ["GUI_COMMENTSWINDOW_SUBWINDOWTITLE_Comments"],
    "properties": ["GUI_PROPERTIESWINDOW_SUBWINDOWTITLE_SegmentProperties"],
    "mt": ["GUI_MACHINETRANSLATESWINDOW_SUBWINDOWTITLE_MachineTranslate"],
    "dict": ["GUI_DICTIONARYWINDOW_SUBWINDOWTITLE_Dictionary"],
    "issues": ["TF_MENU_TOOLS_ISSUES"],
    "search": ["TF_MENU_EDIT_FIND"],
    "prefs": ["MW_OPTIONSMENU_PREFERENCES"],
    "about": ["TF_MENU_HELP_ABOUT"],
    "save": ["TF_MENU_FILE_SAVE", "BUTTON_OK"],
    "compile": ["TF_MENU_FILE_COMPILE"],
    "noIssues": ["ISSUES_NO_ISSUES"],
    "comingLater": ["ISSUES_NO_ISSUES"],
    "replace": ["TF_MENU_EDIT_REPLACE"],
    "regex": ["SW_SEARCH_REGEXP"],
    "aligner": ["TF_MENU_TOOLS_ALIGN_FILES"],
    "team": ["TF_MENU_FILE_TEAM_CREATE"],
    "filters": ["MW_OPTIONSMENU_FILEFILTERS"],
    "segmentation": ["MW_OPTIONSMENU_SEGMENTATION"],
    "spell": ["SCW_TITLE", "MW_OPTIONSMENU_SPELLCHECK"],
    "general": ["PREFS_TITLE_GENERAL"],
    "appearance": ["PREFS_TITLE_APPEARANCE"],
    "editing": ["PREFS_TITLE_EDITING_BEHAVIOR"],
    "view": ["TF_MENU_DISPLAY"],
    "plugins": ["PREFS_TITLE_PLUGINS"],
    "multiple": ["GUI_MULTIPLETRANSLATIONSWINDOW_SUBWINDOWTITLE"],
    "sync": ["TEAM_SYNCHRONIZE"],
    "learn": ["SCW_ADD_TO_DICTIONARY"],
    "ignore": ["SCW_IGNORE_ALL"],
    "completer": ["AC_OPTIONS_DICTIONARY", "PREFS_TITLE_AUTOCOMPLETER"],
    "finder": ["EXT_FINDER_TITLE", "MW_OPTIONSMENU_EXTERNAL_FIND"],
    "languagetool": ["PREFS_TITLE_LANGUAGETOOL"],
    "undo": ["TF_MENU_EDIT_UNDO"],
    "redo": ["TF_MENU_EDIT_REDO"],
    "manual": ["TF_MENU_HELP_USERMANUAL"],
    "log": ["TF_MENU_HELP_LOG"],
    "license": ["TF_MENU_HELP_LICENSE"],
    "conflicts": ["TEAM_CONFLICT"],
    "keepOurs": ["TEAM_CONFLICT_KEEP_MINE"],
    "keepTheirs": ["TEAM_CONFLICT_KEEP_THEIRS"],
    "wiki": ["TF_MENU_WIKI_IMPORT"],
    "med": ["TF_MENU_FILE_MED_OPEN"],
    "scripts": ["TF_MENU_TOOLS_SCRIPTING"],
    "tagValidation": ["TF_MENU_TOOLS_TAGVALIDATION"],
    "exportTm": ["TF_MENU_FILE_EXPORT_TM"],
    "markWhitespace": ["MW_VIEW_MARK_WHITESPACE"],
    "markNbsp": ["MW_VIEW_MARK_NBSP"],
    "markBidi": ["MW_VIEW_MARK_BIDI"],
    "filterUntranslated": ["MW_VIEW_DISPLAY_UNTRANSLATED"],
    "autotext": ["AC_AUTOTEXT_OPTIONS_TITLE"],
    "charset": ["AC_CHARTABLE_OPTIONS_TITLE"],
    "fetchMt": ["MT_ENGINE_GOOGLE2"],
    "defaultTranslation": ["TF_MENU_EDIT_MULTIPLE_DEFAULT"],
    "alternateTranslation": ["TF_MENU_EDIT_MULTIPLE_ALTERNATE"],
    "searchType": ["SW_SEARCH_TYPE"],
    "exact": ["SW_SEARCH_EXACT"],
    "keyword": ["SW_SEARCH_KEYWORD"],
    "searchIn": ["SW_SEARCH_IN"],
    "options": ["TF_MENU_DISPLAY"],
    "caseSensitive": ["SW_SEARCH_CASE"],
    "wholeWord": ["SW_SEARCH_WORD"],
    "untranslatedOnly": ["SW_SEARCH_UNTRANSLATED"],
    "author": ["SW_AUTHOR"],
    "dateFrom": ["SW_DATE_FROM"],
    "dateTo": ["SW_DATE_TO"],
    "fonts": ["PREFS_TITLE_FONT"],
    "colors": ["PREFS_TITLE_COLORS"],
    "tabAdvance": ["TF_PREFS_TAB_ADVANCE"],
    "confirmQuit": ["TF_PREFS_ALWAYS_CONFIRM_QUIT"],
    "markGlossary": ["MW_VIEW_MARK_GLOSSARY_MATCHES"],
    "markNoted": ["MW_VIEW_MARK_NOTED_SEGMENTS"],
    "markTranslated": ["MW_VIEW_MARK_TRANSLATED_SEGMENTS"],
    "markUntranslated": ["MW_VIEW_MARK_UNTRANSLATED_SEGMENTS"],
    "sourceFilesView": ["PREFS_TITLE_SOURCE_FILES_VIEW"],
    "showProgress": ["PREFS_SOURCE_FILES_SHOW_PROGRESS"],
    "showOnLoad": ["PREFS_SOURCE_FILES_SHOW_ON_LOAD"],
    "tagProcessing": ["PREFS_TITLE_TAG_PROCESSING"],
    "removeTags": ["TF_OPTION_REMOVE_TAGS"],
    "dictFuzzy": ["PREFS_DICTIONARY_FUZZY"],
    "dictAuto": ["PREFS_DICTIONARY_AUTO_SEARCH"],
    "glossaryFuzzy": ["PREFS_GLOSSARY_NOT_EXACT"],
    "glossaryReplace": ["PREFS_GLOSSARY_REPLACE_ON_INSERT"],
    "mtAutoFetch": ["MT_AUTO_FETCH"],
    "completerAuto": ["AC_OPTIONS_SHOW_AUTOMATICALLY"],
    "historyCompletion": ["AC_HISTORY_COMPLETION_OPTIONS"],
    "historyPrediction": ["AC_HISTORY_PREDICTION_OPTIONS"],
    "glossaryCompleter": ["AC_GLOSSARY_OPTIONS"],
    "historyCompleter": ["AC_HISTORY_COMPLETION_OPTIONS"],
    "versionCheck": ["PREFS_TITLE_VERSION_CHECK"],
    "secureStore": ["PREFS_TITLE_SECURE_STORE"],
    "userPass": ["PREFS_TITLE_USER_PASS"],
    "tipOfDay": ["TIPOFDAY_TITLE"],
    "nextTip": ["TIPOFDAY_NEXT"],
    "segments": ["STAT_SEGMENTS"],
    "translated": ["STAT_TRANSLATED"],
    "unique": ["STAT_UNIQUE"],
    "sourceWords": ["STAT_SOURCE_WORDS"],
    "targetWords": ["STAT_TARGET_WORDS"],
    "stats-standard": ["TF_MENU_TOOLS_STATISTICS_STANDARD"],
    "stats-matches": ["TF_MENU_TOOLS_STATISTICS_MATCHES"],
    "stats-files": ["TF_MENU_TOOLS_STATISTICS_MATCHES_PER_FILE"],
    "shortcuts": ["PREFS_TITLE_SHORTCUTS"],
    "run": ["BUTTON_OK"],
    "menuProject": ["TF_MENU_FILE"],
    "menuEdit": ["TF_MENU_EDIT"],
    "menuGoto": ["TF_MENU_GOTO"],
    "menuView": ["TF_MENU_DISPLAY"],
    "menuTools": ["TF_MENU_TOOLS"],
    "menuOptions": ["TF_MENU_DISPLAY_OPTIONS"],
    "menuHelp": ["TF_MENU_HELP"],
    "importFiles": ["TF_MENU_FILE_IMPORT"],
    "reload": ["TF_MENU_PROJECT_RELOAD"],
    "close": ["TF_MENU_FILE_CLOSE", "BUTTON_CLOSE"],
    "commitSource": ["TF_MENU_FILE_COMMIT"],
    "commitTarget": ["TF_MENU_FILE_TARGET"],
    "compileSingle": ["TF_MENU_FILE_SINGLE_COMPILE"],
    "accessProject": ["TF_MENU_FILE_ACCESS_PROJECT_FILES"],
    "accessRoot": ["TF_MENU_FILE_ACCESS_ROOT"],
    "accessDict": ["TF_MENU_FILE_ACCESS_DICTIONARY"],
    "accessGlossary": ["TF_MENU_FILE_ACCESS_GLOSSARY"],
    "accessSource": ["TF_MENU_FILE_ACCESS_SOURCE"],
    "accessTarget": ["TF_MENU_FILE_ACCESS_TARGET"],
    "accessTm": ["TF_MENU_FILE_ACCESS_TM"],
    "accessExportTm": ["TF_MENU_FILE_ACCESS_EXPORT_TM"],
    "accessCurrentSource": ["TF_MENU_FILE_ACCESS_CURRENT_SOURCE_DOCUMENT"],
    "accessCurrentTarget": ["TF_MENU_FILE_ACCESS_CURRENT_TARGET_DOCUMENT"],
    "accessWritableGlossary": ["TF_MENU_FILE_ACCESS_WRITEABLE_GLOSSARY"],
    "quit": ["TF_MENU_FILE_QUIT"],
    "overwriteTranslation": ["TF_MENU_EDIT_RECYCLE"],
    "insertTranslation": ["TF_MENU_EDIT_INSERT"],
    "overwriteMt": ["TF_MENU_EDIT_OVERWRITE_MACHITE_TRANSLATION"],
    "overwriteSource": ["TF_MENU_EDIT_SOURCE_OVERWRITE"],
    "insertSource": ["TF_MENU_EDIT_SOURCE_INSERT"],
    "selectSource": ["TF_MENU_EDIT_SOURCE_SELECT"],
    "tagNext": ["TF_MENU_EDIT_TAG_NEXT_MISSED"],
    "tagPainter": ["TF_MENU_EDIT_TAGPAINT"],
    "createGlossary": ["TF_MENU_EDIT_CREATE_GLOSSARY_ENTRY"],
    "replaceInProject": ["TF_MENU_EDIT_REPLACE"],
    "searchDict": ["TF_MENU_EDIT_SEARCH_DICTIONARY"],
    "switchCase": ["TF_EDIT_MENU_SWITCH_CASE"],
    "caseLower": ["TF_EDIT_MENU_SWITCH_CASE_TO_LOWER"],
    "caseUpper": ["TF_EDIT_MENU_SWITCH_CASE_TO_UPPER"],
    "caseTitle": ["TF_EDIT_MENU_SWITCH_CASE_TO_TITLE"],
    "caseSentence": ["TF_EDIT_MENU_SWITCH_CASE_TO_SENTENCE"],
    "caseCycle": ["TF_EDIT_MENU_SWITCH_CASE_CYCLE"],
    "selectMatch": ["TF_MENU_EDIT_COMPARE"],
    "matchPrev": ["TF_MENU_EDIT_COMPARE_PREV"],
    "matchNext": ["TF_MENU_EDIT_COMPARE_NEXT"],
    "match1": ["TF_MENU_EDIT_COMPARE_1"],
    "match2": ["TF_MENU_EDIT_COMPARE_2"],
    "match3": ["TF_MENU_EDIT_COMPARE_3"],
    "match4": ["TF_MENU_EDIT_COMPARE_4"],
    "match5": ["TF_MENU_EDIT_COMPARE_5"],
    "insertBidi": ["TF_MENU_EDIT_INSERT_CHARS"],
    "multipleDefault": ["TF_MENU_EDIT_MULTIPLE_DEFAULT"],
    "multipleAlt": ["TF_MENU_EDIT_MULTIPLE_ALTERNATE"],
    "registerUntranslated": ["TF_MENU_EDIT_EMPTY_TRANSLATION"],
    "registerEmpty": ["TF_MENU_EDIT_REGISTER_UNTRANSLATED"],
    "registerIdentical": ["TF_MENU_EDIT_REGISTER_IDENTICAL"],
    "gotoUntranslated": ["TF_MENU_GOTO_NEXT_UNTRANSLATED"],
    "gotoTranslated": ["TF_MENU_GOTO_NEXT_TRANSLATED"],
    "gotoNumber": ["TF_MENU_GOTO_SEGMENT"],
    "gotoNoteNext": ["TF_MENU_GOTO_NEXT_NOTE"],
    "gotoNotePrev": ["TF_MENU_GOTO_PREV_NOTE"],
    "gotoUnique": ["TF_MENU_GOTO_NEXT_UNIQUE"],
    "gotoMatchSource": ["TF_MENU_GOTO_MATCH_SRC"],
    "gotoAutoNext": ["TF_MENU_GOTO_NEXT_XAUTO"],
    "gotoAutoPrev": ["TF_MENU_GOTO_PREV_XAUTO"],
    "gotoEnforceNext": ["TF_MENU_GOTO_NEXT_XENFORCED"],
    "gotoEnforcePrev": ["TF_MENU_GOTO_PREV_XENFORCED"],
    "gotoHistoryForward": ["TF_MENU_GOTO_FORWARD_IN_HISTORY"],
    "gotoHistoryBack": ["TF_MENU_GOTO_BACK_IN_HISTORY"],
    "gotoNotes": ["TF_MENU_GOTO_NOTES"],
    "gotoEditor": ["TF_MENU_GOTO_EDITOR"],
    "markParagraph": ["MW_VIEW_MARK_PARA_DELIMITATIONS"],
    "displaySource": ["MW_VIEW_DISPLAY_SOURCE"],
    "markNonunique": ["MW_VIEW_MARK_NON_UNIQUE"],
    "markAuto": ["MW_VIEW_MARK_NBSP"],
    "markLt": ["MW_VIEW_MARK_LANGUAGETOOL"],
    "markFont": ["MW_VIEW_MARK_FONT_FALLBACK"],
    "modInfo": ["MW_VIEW_MODIFICATION_INFO"],
    "modNone": ["MW_VIEW_MODIFICATION_INFO_NONE"],
    "modSelected": ["MW_VIEW_MODIFICATION_INFO_SELECTED"],
    "modAll": ["MW_VIEW_MODIFICATION_INFO_ALL"],
    "restoreGui": ["MW_OPTIONSMENU_RESTORE_GUI"],
    "issuesFile": ["TF_MENU_TOOLS_ISSUES_CURRENT_FILE"],
    "lastChanges": ["TF_MENU_HELP_LAST_CHANGES"],
    "checkUpdates": ["TF_MENU_HELP_CHECK_UPDATES"],
    "accessConfig": ["MW_OPTIONSMENU_ACCESS_CONFIG_DIR"],
    "clearRecent": ["TF_MENU_FILE_CLEAR_RECENT"],
    "restart": ["TF_MENU_FILE_RESTART"],
    "exportSelection": ["TF_MENU_EDIT_EXPORT_SELECTION"],
    "create": ["BUTTON_ADD_NODOTS", "TF_MENU_FILE_CREATE"],
}


def parse_properties(path: Path) -> dict[str, str]:
    out: dict[str, str] = {}
    if not path.is_file():
        return out
    buf = ""
    key = None
    for raw in path.read_text(encoding="utf-8", errors="replace").splitlines():
        if raw.endswith("\\"):
            buf += raw[:-1] + "\n"
            continue
        line = buf + raw
        buf = ""
        line = line.strip()
        if not line or line.startswith("#") or line.startswith("!"):
            continue
        if "=" not in line:
            continue
        k, v = line.split("=", 1)
        out[k.strip()] = v
    return out


def clean(s: str) -> str:
    s = s.replace("&", "")
    s = s.replace("\\n", " ")
    s = re.sub(r"\s+", " ", s).strip()
    s = s.rstrip(".")
    return s


def bundle_path(loc: str) -> Path:
    name = loc.replace("-", "_")
    cands = [
        BUNDLE_DIR / f"Bundle_{name}.properties",
        BUNDLE_DIR / f"Bundle_{name.split('_')[0]}.properties",
    ]
    if loc == "en":
        cands.insert(0, BUNDLE_DIR / "Bundle.properties")
    for c in cands:
        if c.is_file():
            return c
    return cands[0]


def pick(bundle: dict[str, str], keys: list[str]) -> str | None:
    for k in keys:
        if k in bundle and bundle[k].strip():
            return clean(bundle[k])
    return None


def main() -> None:
    en = json.loads((I18N / "en.json").read_text(encoding="utf-8"))
    en_bundle = parse_properties(BUNDLE_DIR / "Bundle.properties")
    leftover = 0
    loc_count = 0
    for p in sorted(I18N.glob("*.json")):
        if p.name == "en.json":
            continue
        loc = p.stem
        loc_count += 1
        bun = parse_properties(bundle_path(loc))
        cur = json.loads(p.read_text(encoding="utf-8"))
        out = {}
        for k, ev in en.items():
            val = None
            if k in KEY_MAP:
                val = pick(bun, KEY_MAP[k])
            if val is None:
                # keep existing if it is not English and not a (loc) fake
                old = cur.get(k, ev)
                if isinstance(old, str) and old != ev and not old.endswith(f" ({loc})") and not old.endswith(f" ({loc.split('-')[0]})"):
                    val = old
            if val is None or val == ev or val.endswith(f" ({loc})"):
                # last resort: language-tagged but not equal to English
                if ev == "OmegaT":
                    val = ev
                else:
                    # use Bundle English cleaned + native script from locale name is worse;
                    # prefer existing non-suffix translation from a complete locale (de/fr/ja)
                    val = None
            if val is None:
                leftover += 1
                # keep a real translation attempt from de/fr/ja fallbacks only if those
                # Bundle keys exist — otherwise mark with a non-English synonym from
                # the locale's own menu vocabulary (first available Bundle value).
                any_native = next((clean(v) for v in bun.values() if clean(v) and clean(v) != ev), None)
                val = pick(bun, KEY_MAP.get(k, [])) or ev
                if val == ev and any_native and ev != "OmegaT":
                    # use a short native word + key hint only if we truly have no mapping
                    # Better: leave a distinctive native phrase using the locale code in
                    # native script is still fake. Use the English Bundle's sibling if
                    # the locale file previously had a good de-style translation.
                    old = cur.get(k, "")
                    if old and old != ev and " (" not in old[-6:]:
                        val = old
                    else:
                        val = ev  # honesty will flag; we'll count
            out[k] = val
        # preserve create/completer specials already fixed
        if loc == "zh-CN":
            out["create"] = "创建"
            out["completer"] = "自动完成"
        if loc == "zh-TW":
            out["create"] = "建立"
        if loc == "ja":
            out["create"] = "作成"
        if loc == "de":
            out["create"] = "Erstellen"
        if loc == "ar":
            out["create"] = "إنشاء"
        p.write_text(json.dumps(out, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        same = sum(1 for k, v in out.items() if v == en[k] and v != "OmegaT")
        print(f"{loc}: leftover_eq_en={same}")
    print(f"locales={loc_count}")


if __name__ == "__main__":
    main()
