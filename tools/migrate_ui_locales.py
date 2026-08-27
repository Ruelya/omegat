#!/usr/bin/env python3
"""Map each desktop UI key to a Java Bundle_* key and rewrite locale JSON.

`create` is never taken from BUTTON_ADD (that string is 「添加」/Add).
English `en.json` is the key authority; other locales are filled from Java.
"""

from __future__ import annotations

import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
BUNDLE_DIR = ROOT / "reference/java/src/main/resources/org/omegat"
TIP_DIR = ROOT / "reference/java/tipoftheday/src/main/resources/org/omegat/gui/tipoftheday"
I18N = ROOT / "apps/desktop/src/renderer/i18n"

# UI key → Java Bundle.properties key. Never map `create` to BUTTON_ADD.
KEY_MAP: dict[str, str | None] = {
    "app": None,
    "welcomeLead": None,
    "openProject": "TF_MENU_FILE_OPEN",
    "newProject": "TF_MENU_FILE_CREATE",
    "recent": "TF_MENU_FILE_OPEN_RECENT",
    "sourceLang": "PP_SRC_LANG",
    "targetLang": "PP_LOC_LANG",
    "sentenceSeg": "PP_SENTENCE_SEGMENTING",
    "create": None,
    "cancel": "BUTTON_CANCEL",
    "files": "TF_MENU_FILE_PROJWIN",
    "editor": "PREFS_TITLE_EDITING_BEHAVIOR",
    "source": "PP_SRC_ROOT",
    "target": "PP_LOC_ROOT",
    "matches": "GUI_MATCHWINDOW_SUBWINDOWTITLE_Fuzzy_Matches",
    "glossary": "PREFS_TITLE_GLOSSARY",
    "notes": "GUI_NOTESWINDOW_SUBWINDOWTITLE_Notes",
    "comments": "GUI_COMMENTSWINDOW_SUBWINDOWTITLE_Comments",
    "properties": "MW_PROJECTMENU_EDIT",
    "mt": "PREFS_TITLE_MACHINE_TRANSLATION",
    "dict": "PREFS_TITLE_DICTIONARY",
    "issues": "ISSUES_WINDOW_TITLE",
    "search": "TF_MENU_EDIT_FIND",
    "prefs": "MW_OPTIONSMENU_PREFERENCES",
    "about": "TF_MENU_HELP_ABOUT",
    "save": "TF_MENU_FILE_SAVE",
    "compile": "TF_MENU_FILE_COMPILE",
    "comingLater": None,
    "noIssues": None,
    "firstRun": None,
    "tip": None,
    "uiLanguage": None,
    "root": "TF_MENU_FILE_ACCESS_ROOT",
    "replace": "BUTTON_REPLACE_ALL",
    "regex": "SW_REGEXP_SEARCH",
    "aligner": "TF_MENU_TOOLS_ALIGN_FILES",
    "team": "TF_MENU_FILE_TEAM_CREATE",
    "filters": "TF_MENU_DISPLAY_GLOBAL_FILTERS",
    "segmentation": "MW_OPTIONSMENU_GLOBAL_SENTSEG",
    "spell": "PREFS_TITLE_SPELLCHECKER",
    "general": "PREFS_TITLE_GENERAL",
    "appearance": "PREFS_TITLE_APPEARANCE",
    "editing": "PREFS_TITLE_EDITING_BEHAVIOR",
    "view": "MW_VIEW_MENU",
    "plugins": "PREFS_TITLE_PLUGINS",
    "fuzzyThreshold": "EXT_TMX_FUZZY_THRESHOLD_KEY",
    "autosave": "PREFS_TITLE_SAVING_AND_OUTPUT",
    "multiple": "MW_VIEW_MENU_MARK_ALT_TRANSLATIONS",
    "sync": "TEAM_SYNCHRONIZE",
    "learn": "SC_ADD_TO_DICTIONARY",
    "ignore": "SC_IGNORE_ALL",
    "completer": "PREFS_TITLE_AUTOCOMPLETER",
    "finder": "PREFS_TITLE_GLOBAL_EXTERNALFINDER",
    "languagetool": "PREFS_TITLE_LANGUAGETOOL",
    "glossaryStem": "PREFS_GLOSSARY_STEMMING",
    "insertBest": "TF_MENU_EDIT_INSERT",
    "nextSeg": "TF_MENU_EDIT_NEXT",
    "prevSeg": "TF_MENU_EDIT_PREV",
    "undo": "TF_MENU_EDIT_UNDO",
    "redo": "TF_MENU_EDIT_REDO",
    "manual": "TF_MENU_HELP_CONTENTS",
    "log": "TF_MENU_HELP_LOG",
    "license": "LICENSEDIALOG_TITLE",
    "conflicts": "CONFLICT_DIALOG_TITLE",
    "keepOurs": "CONFLICT_DIALOG_BUTTON_MINE",
    "keepTheirs": "CONFLICT_DIALOG_BUTTON_THEIRS",
    "wiki": "TF_MENU_WIKI_IMPORT",
    "med": "TF_MENU_FILE_MED_OPEN",
    "convert": None,
    "scripts": "TF_MENU_TOOLS_SCRIPTING",
    "tagValidation": "PREFS_TITLE_TAG_PROCESSING",
    "exportTm": "PP_EXPORT_TM_LEVEL1",
    "markWhitespace": "MW_VIEW_MENU_MARK_WHITESPACE",
    "markNbsp": "MW_VIEW_MENU_MARK_NBSP",
    "markBidi": "MW_VIEW_MENU_MARK_BIDI",
    "filterUntranslated": "SW_SEARCH_UNTRANSLATED",
    "autotext": "PREFS_TITLE_AUTOCOMPLETER_AUTOTEXT",
    "charset": "PREFS_TITLE_AUTOCOMPLETER_CHAR_TABLE",
    "fetchMt": "PREFS_MT_AUTO_FETCH",
    "defaultTranslation": "MULT_MENU_DEFAULT",
    "alternateTranslation": "MULT_MENU_MULTIPLE",
    "searchType": None,
    "exact": "SW_EXACT_SEARCH",
    "keyword": "SW_WORD_SEARCH",
    "searchIn": "SW_SEARCH_IN_BOX",
    "options": "MW_OPTIONSMENU",
    "caseSensitive": "SW_CASE_SENSITIVE",
    "wholeWord": "SW_WHOLE_WORDS",
    "untranslatedOnly": "SW_SEARCH_UNTRANSLATED",
    "author": "SW_AUTHOR",
    "dateFrom": "SW_CHANGED_AFTER",
    "dateTo": "SW_CHANGED_BEFORE",
    "replacePreview": "BUTTON_REPLACE",
    "fonts": "PREFS_TITLE_FONT",
    "fontUi": None,
    "fontEditor": None,
    "colors": "PREFS_TITLE_COLORS",
    "tabAdvance": None,
    "confirmQuit": "MW_OPTIONSMENU_ALWAYS_CONFIRM_QUIT",
    "matchesStem": "EXT_TMX_SORT_KEY_SCORE",
    "markGlossary": "MW_VIEW_GLOSSARY_MARK",
    "markNoted": "MW_VIEW_MENU_MARK_NOTED_SEGMENTS",
    "markTranslated": "TF_MENU_DISPLAY_MARK_TRANSLATED",
    "markUntranslated": "TF_MENU_DISPLAY_MARK_UNTRANSLATED",
    "sourceFilesView": "PREFS_TITLE_SOURCE_FILES",
    "showProgress": "PREFS_SHOW_PROJECT_FILES_PROGRESS",
    "showOnLoad": "PF_OPEN_SOURCE_FILES",
    "tagProcessing": "PREFS_TITLE_TAG_PROCESSING",
    "removeTags": "PP_REMOVE_TAGS",
    "dictFuzzy": "PREFS_DICTIONARY_FUZZY",
    "dictAuto": "PREFS_DICTIONARY_AUTO_SEARCH",
    "ignoreCase": "PREFS_GLOSSARY_REQUIRE_SIMILAR_CASE",
    "glossaryFuzzy": "TF_OPTIONSMENU_GLOSSARY_FUZZY",
    "glossaryReplace": "PREFS_GLOSSARY_REPLACE_ON_INSERT",
    "mtAutoFetch": "PREFS_MT_AUTO_FETCH",
    "completerAuto": "MW_OPTIONSMENU_AUTOCOMPLETE_SHOW_AUTOMATICALLY",
    "historyCompletion": "MW_OPTIONSMENU_AUTOCOMPLETE_HISTORY_COMPLETION",
    "historyPrediction": "MW_OPTIONSMENU_AUTOCOMPLETE_HISTORY_PREDICTION",
    "glossaryCompleter": "PREFS_TITLE_AUTOCOMPLETER_GLOSSARY",
    "historyCompleter": "PREFS_TITLE_AUTOCOMPLETE_HISTORY",
    "passphrase": "TEAM_PASSPHRASE_FIRST",
    "versionCheck": "PREFS_TITLE_VERSION_CHECK",
    "secureStore": "PREFS_TITLE_SECURE_STORE",
    "userPass": "PREFS_TITLE_PROXY_LOGIN",
    "masterPassword": "PREFS_SECURE_STORE_MASTER_PASSWORD_LABEL",
    "tipOfDay": "TipOfTheDay.dialogTitle",
    "nextTip": "TipOfTheDay.nextTipText",
    "segments": "CT_STATS_Segments",
    "translated": "SW_SEARCH_TRANSLATED",
    "unique": "CT_STATS_Unique",
    "sourceWords": "CT_STATS_Words",
    "targetWords": "CT_STATS_Words",
    "stats-standard": "CT_STATSSTANDARD_WindowHeader",
    "stats-matches": "CT_STATSMATCH_WindowHeader",
    "stats-files": "CT_STATSMATCH_PER_FILE_WindowHeader",
    "shortcuts": None,
    "run": "SCW_RUN_SCRIPT",
    "menuProject": "TF_MENU_FILE",
    "menuEdit": "TF_MENU_EDIT",
    "menuGoto": "MW_GOTOMENU",
    "menuView": "MW_VIEW_MENU",
    "menuTools": "TF_MENU_TOOLS",
    "menuOptions": "MW_OPTIONSMENU",
    "menuHelp": "TF_MENU_HELP",
    "importFiles": "TF_MENU_FILE_IMPORT",
    "reload": "TF_MENU_PROJECT_RELOAD",
    "close": "TF_MENU_FILE_CLOSE",
    "commitSource": "TF_MENU_FILE_COMMIT",
    "commitTarget": "TF_MENU_FILE_TARGET",
    "compileSingle": "TF_MENU_FILE_SINGLE_COMPILE",
    "accessProject": "TF_MENU_FILE_ACCESS_PROJECT_FILES",
    "accessRoot": "TF_MENU_FILE_ACCESS_ROOT",
    "accessDict": "TF_MENU_FILE_ACCESS_DICTIONARY",
    "accessGlossary": "TF_MENU_FILE_ACCESS_GLOSSARY",
    "accessSource": "TF_MENU_FILE_ACCESS_SOURCE",
    "accessTarget": "TF_MENU_FILE_ACCESS_TARGET",
    "accessTm": "TF_MENU_FILE_ACCESS_TM",
    "accessExportTm": "TF_MENU_FILE_ACCESS_EXPORT_TM",
    "accessCurrentSource": "TF_MENU_FILE_ACCESS_CURRENT_SOURCE_DOCUMENT",
    "accessCurrentTarget": "TF_MENU_FILE_ACCESS_CURRENT_TARGET_DOCUMENT",
    "accessWritableGlossary": "TF_MENU_FILE_ACCESS_WRITEABLE_GLOSSARY",
    "quit": "TF_MENU_FILE_QUIT",
    "overwriteTranslation": "TF_MENU_EDIT_RECYCLE",
    "insertTranslation": "TF_MENU_EDIT_INSERT",
    "overwriteMt": "TF_MENU_EDIT_OVERWRITE_MACHITE_TRANSLATION",
    "overwriteSource": "TF_MENU_EDIT_SOURCE_OVERWRITE",
    "insertSource": "TF_MENU_EDIT_SOURCE_INSERT",
    "selectSource": "TF_MENU_EDIT_SOURCE_SELECT",
    "tagNext": "TF_MENU_EDIT_TAG_NEXT_MISSED",
    "tagPainter": "TF_MENU_EDIT_TAGPAINT",
    "createGlossary": "TF_MENU_EDIT_CREATE_GLOSSARY_ENTRY",
    "replaceInProject": "TF_MENU_EDIT_REPLACE",
    "searchDict": "TF_MENU_EDIT_SEARCH_DICTIONARY",
    "switchCase": "TF_EDIT_MENU_SWITCH_CASE",
    "caseLower": "TF_EDIT_MENU_SWITCH_CASE_TO_LOWER",
    "caseUpper": "TF_EDIT_MENU_SWITCH_CASE_TO_UPPER",
    "caseTitle": "TF_EDIT_MENU_SWITCH_CASE_TO_TITLE",
    "caseSentence": "TF_EDIT_MENU_SWITCH_CASE_TO_SENTENCE",
    "caseCycle": "TF_EDIT_MENU_SWITCH_CASE_CYCLE",
    "selectMatch": "TF_MENU_EDIT_COMPARE",
    "matchPrev": "TF_MENU_EDIT_COMPARE_PREV",
    "matchNext": "TF_MENU_EDIT_COMPARE_NEXT",
    "match1": "TF_MENU_EDIT_COMPARE_1",
    "match2": "TF_MENU_EDIT_COMPARE_2",
    "match3": "TF_MENU_EDIT_COMPARE_3",
    "match4": "TF_MENU_EDIT_COMPARE_4",
    "match5": "TF_MENU_EDIT_COMPARE_5",
    "insertBidi": "TF_MENU_EDIT_INSERT_CHARS",
    "multipleDefault": "MULT_MENU_DEFAULT",
    "multipleAlt": "MULT_MENU_MULTIPLE",
    "registerUntranslated": "TF_MENU_EDIT_UNTRANSLATED_TRANSLATION",
    "registerEmpty": "TF_MENU_EDIT_EMPTY_TRANSLATION",
    "registerIdentical": "TF_MENU_EDIT_IDENTICAL_TRANSLATION",
    "gotoUntranslated": "TF_MENU_EDIT_UNTRANS",
    "gotoTranslated": "TF_MENU_EDIT_TRANS",
    "gotoNumber": "TF_MENU_EDIT_GOTO",
    "gotoNoteNext": "TF_MENU_EDIT_NEXT_NOTE",
    "gotoNotePrev": "TF_MENU_EDIT_PREV_NOTE",
    "gotoUnique": "TF_MENU_GOTO_NEXT_UNIQUE",
    "gotoMatchSource": "TF_MENU_GOTO_SELECTED_MATCH_SOURCE",
    "gotoAutoNext": "TF_MENU_GOTO_NEXT_XAUTO",
    "gotoAutoPrev": "TF_MENU_GOTO_PREV_XAUTO",
    "gotoEnforceNext": "TF_MENU_GOTO_NEXT_XENFORCED",
    "gotoEnforcePrev": "TF_MENU_GOTO_PREV_XENFORCED",
    "gotoHistoryForward": "TF_MENU_GOTO_FORWARD_IN_HISTORY",
    "gotoHistoryBack": "TF_MENU_GOTO_BACK_IN_HISTORY",
    "gotoNotes": "TF_MENU_GOTO_NOTES_PANEL",
    "gotoEditor": "TF_MENU_GOTO_EDITOR_PANEL",
    "markParagraph": "TF_MENU_DISPLAY_MARK_PARAGRAPH",
    "displaySource": "MW_VIEW_MENU_DISPLAY_SEGMENT_SOURCES",
    "markNonunique": "MW_VIEW_MENU_MARK_NON_UNIQUE_SEGMENTS",
    "markAuto": "MW_VIEW_MENU_MARK_AUTOPOPULATED",
    "markLt": "LT_OPTIONS_MENU_ENABLED",
    "markFont": "MW_VIEW_MENU_MARK_FONT_FALLBACK",
    "modInfo": "MW_VIEW_MENU_MODIFICATION_INFO",
    "modNone": "MW_VIEW_MENU_MODIFICATION_INFO_NONE",
    "modSelected": "MW_VIEW_MENU_MODIFICATION_INFO_SELECTED",
    "modAll": "MW_VIEW_MENU_MODIFICATION_INFO_ALL",
    "restoreGui": "MW_OPTIONSMENU_RESTORE_GUI",
    "issuesFile": "TF_MENU_TOOLS_CHECK_ISSUES_CURRENT_FILE",
    "lastChanges": "TF_MENU_HELP_LAST_CHANGES",
    "checkUpdates": "TF_MENU_HELP_CHECK_FOR_UPDATES",
    "accessConfig": "MW_OPTIONSMENU_ACCESS_CONFIG_DIR",
}

CREATE = {
    "en": "Create",
    "zh-CN": "创建",
    "zh-TW": "建立",
    "ja": "作成",
    "ko": "만들기",
    "de": "Erstellen",
    "fr": "Créer",
    "es": "Crear",
    "pt": "Criar",
    "pt-BR": "Criar",
    "it": "Crea",
    "nl": "Aanmaken",
    "ru": "Создать",
    "uk": "Створити",
    "pl": "Utwórz",
    "cs": "Vytvořit",
    "sk": "Vytvoriť",
    "hu": "Létrehozás",
    "fi": "Luo",
    "sv": "Skapa",
    "da": "Opret",
    "no": "Opprett",
    "el": "Δημιουργία",
    "tr": "Oluştur",
    "ar": "إنشاء",
    "ca": "Crea",
    "eu": "Sortu",
    "gl": "Crear",
    "hr": "Stvori",
    "sl": "Ustvari",
    "sq": "Krijo",
    "be": "Стварыць",
    "eo": "Krei",
    "ia": "Crear",
    "id": "Buat",
    "co": "Creà",
    "cy": "Creu",
    "mfe": "Kre",
    "sc": "Crea",
    "sh": "Kreiraj",
    "tk": "Döret",
}

# Keys with no Java Bundle equivalent — authored per locale (not English tails).
FALLBACK: dict[str, dict[str, str]] = {
    "welcomeLead": {
        "zh-CN": "面向译员的键盘优先工作站：翻译记忆、术语表与带标签文档。",
        "zh-TW": "面向譯者的鍵盤優先工作站：翻譯記憶、詞彙表與帶標籤文件。",
        "ja": "翻訳メモリ・用語集・タグ付き文書向けの、キーボード優先ワークステーション。",
        "de": "Eine tastaturgeführte Arbeitsstation für Translation Memory, Glossare und markierte Dokumente.",
        "fr": "Un poste de travail clavier d’abord pour les mémoires, glossaires et documents balisés.",
        "es": "Estación de trabajo centrada en el teclado para memorias, glosarios y documentos etiquetados.",
        "ru": "Клавиатурная рабочая станция для памяти переводов, глоссариев и размеченных документов.",
        "ar": "محطة عمل تعتمد على لوحة المفاتيح للذاكرة الترجمية والمسارد والمستندات ذات الوسوم.",
        "ko": "번역 메모리, 용어집, 태그 문서를 위한 키보드 우선 워크스테이션.",
        "pt": "Estação de trabalho centrada no teclado para memórias, glossários e documentos etiquetados.",
        "pt-BR": "Estação de trabalho centrada no teclado para memórias, glossários e documentos etiquetados.",
        "it": "Postazione a tastiera per memorie di traduzione, glossari e documenti con tag.",
        "nl": "Een toetsenbordgerichte werkplek voor vertaalgeheugens, woordenlijsten en gelabelde documenten.",
        "pl": "Stacja robocza sterowana klawiaturą: pamięć tłumaczeń, glosariusze i dokumenty z tagami.",
        "uk": "Клавіатурна робоча станція для пам’яті перекладів, глосаріїв і розмічених документів.",
        "cs": "Klávesnicová pracovní stanice pro překladovou paměť, glosáře a označené dokumenty.",
        "hu": "Billentyűzetközpontú munkaállomás fordítási memóriához, szószedetekhez és címkézett dokumentumokhoz.",
        "tr": "Çeviri belleği, sözlükler ve etiketli belgeler için klavye öncelikli iş istasyonu.",
        "ca": "Estació de treball centrada en el teclat per a memòries, glossaris i documents etiquetats.",
        "el": "Σταθμός εργασίας με προτεραιότητα το πληκτρολόγιο για μνήμες, γλωσσάρια και ετικετοποιημένα έγγραφα.",
        "fi": "Näppäimistökeskeinen työasema käännösmuistille, sanalistoille ja merkittyille asiakirjoille.",
        "sv": "Ett tangentbordsförst arbetsstation för översättningsminnen, ordlistor och märkta dokument.",
        "da": "En tastaturførst arbejdsstation til oversættelseshukommelser, glossarer og mærkede dokumenter.",
        "no": "Et tastaturførst arbeidssted for oversettelsesminner, glossarer og merkede dokumenter.",
        "id": "Stasiun kerja berprioritas papan ketik untuk memori terjemahan, glosarium, dan dokumen bertanda.",
        "hr": "Radna stanica usmjerena na tipkovnicu za prijevodnu memoriju, glosare i označene dokumente.",
        "sk": "Klávesnicová pracovná stanica pre prekladovú pamäť, glosáre a označené dokumenty.",
        "sl": "Delovna postaja s prednostjo tipkovnice za prevajalski pomnilnik, glosarje in označene dokumente.",
        "gl": "Estación de traballo centrada no teclado para memorias, glosarios e documentos etiquetados.",
        "eu": "Teklatuaren lehentasuna duen lan-estazioa: itzulpen-memoria, glosategiak eta etiketadun dokumentuak.",
        "be": "Клавіятурная рабочая станцыя для памяці перакладаў, гласарыяў і размечаных дакументаў.",
        "eo": "Klavar-unua laborejo por tradukmemoroj, vortaroj kaj etikedaj dokumentoj.",
        "ia": "Station de travalio focalisate sur le claviero pro memorias, glossarios e documentos etiquettate.",
        "co": "Stazione di travagliu à tastiera per memorie, glossarii è documenti cù etichette.",
        "cy": "Gweithfan allweddell-gyntaf ar gyfer atgofion cyfieithu, geirfaoedd a dogfennau tag.",
        "mfe": "En stasion travay klavye-premye pou memwar tradiksion, gloser ek dokiman ek tag.",
        "sc": "Istazione de traballu a tastiera pro memorias, glossàrios e documentos cun etichetas.",
        "sh": "Radna stanica usmerena na tastaturu za prevodilačku memoriju, glosare i označene dokumente.",
        "sq": "Stacion pune me përparësi tastierën për kujtesa përkthimi, glossarë dhe dokumente me etiketë.",
        "tk": "Terjime ýady, sözlükler we belligi bolan resminamalar üçin klawiatura ilkinji iş stansiýasy.",
        "ia": "Station de travalio focalisate sur le claviero pro memorias, glossarios e documentos etiquettate.",
    },
    "uiLanguage": {
        "zh-CN": "界面语言",
        "zh-TW": "介面語言",
        "ja": "表示言語",
        "ko": "UI 언어",
        "de": "UI-Sprache",
        "fr": "Langue de l’interface",
        "es": "Idioma de la interfaz",
        "ru": "Язык интерфейса",
        "ar": "لغة الواجهة",
        "pt": "Idioma da interface",
        "pt-BR": "Idioma da interface",
        "it": "Lingua dell’interfaccia",
        "nl": "Interface taal",
        "pl": "Język interfejsu",
        "uk": "Мова інтерфейсу",
        "cs": "Jazyk rozhraní",
        "hu": "Felület nyelve",
        "tr": "Arayüz dili",
        "ca": "Llengua de la interfície",
        "el": "Γλώσσα διεπαφής",
        "fi": "Käyttöliittymän kieli",
        "sv": "Gränssnittsspråk",
        "da": "Grænsefladesprog",
        "no": "Grensesnittspråk",
        "id": "Bahasa antarmuka",
        "hr": "Jezik sučelja",
        "sk": "Jazyk rozhrania",
        "sl": "Jezik vmesnika",
        "gl": "Idioma da interface",
        "eu": "Interfazearen hizkuntza",
        "be": "Мова інтэрфейсу",
        "eo": "Interfaca lingvo",
        "ia": "Lingua del interfacie",
        "co": "Lingua di l’interfaccia",
        "cy": "Iaith y rhyngwyneb",
        "mfe": "Lang lazinterface",
        "sc": "Limba de s’interfache",
        "sh": "Jezik interfejsa",
        "sq": "Gjuha e ndërfaqes",
        "tk": "Interfeýs dili",
    },
    "comingLater": {},
    "noIssues": {},
    "firstRun": {},
    "tip": {},
    "convert": {
        "zh-CN": "转换项目",
        "zh-TW": "轉換專案",
        "ja": "プロジェクトを変換",
        "de": "Projekt konvertieren",
        "fr": "Convertir le projet",
        "es": "Convertir proyecto",
        "ru": "Преобразовать проект",
        "ar": "تحويل المشروع",
        "ko": "프로젝트 변환",
        "pt": "Converter projeto",
        "pt-BR": "Converter projeto",
        "it": "Converti progetto",
        "nl": "Project converteren",
        "pl": "Konwertuj projekt",
        "uk": "Перетворити проєкт",
    },
    "searchType": {
        "zh-CN": "搜索类型",
        "zh-TW": "搜尋類型",
        "ja": "検索の種類",
        "de": "Suchtyp",
        "fr": "Type de recherche",
        "es": "Tipo de búsqueda",
        "ru": "Тип поиска",
        "ar": "نوع البحث",
        "ko": "검색 유형",
    },
    "fontUi": {
        "zh-CN": "界面字体",
        "zh-TW": "介面字型",
        "ja": "UI フォント",
        "de": "UI-Schriftart",
        "fr": "Police de l’interface",
        "es": "Fuente de la interfaz",
        "ru": "Шрифт интерфейса",
        "ar": "خط الواجهة",
        "ko": "UI 글꼴",
    },
    "fontEditor": {
        "zh-CN": "编辑器字体",
        "zh-TW": "編輯器字型",
        "ja": "エディタフォント",
        "de": "Editor-Schriftart",
        "fr": "Police de l’éditeur",
        "es": "Fuente del editor",
        "ru": "Шрифт редактора",
        "ar": "خط المحرر",
        "ko": "편집기 글꼴",
    },
    "shortcuts": {
        "zh-CN": "键盘快捷键",
        "zh-TW": "鍵盤快速鍵",
        "ja": "キーボードショートカット",
        "de": "Tastenkürzel",
        "fr": "Raccourcis clavier",
        "es": "Atajos de teclado",
        "ru": "Сочетания клавиш",
        "ar": "اختصارات لوحة المفاتيح",
        "ko": "바로 가기 키",
        "pt": "Atalhos de teclado",
        "it": "Scorciatoie da tastiera",
        "nl": "Sneltoetsen",
        "pl": "Skróty klawiszowe",
        "uk": "Клавіатурні скорочення",
    },
    "tabAdvance": {
        "zh-CN": "Tab 前进到下一段",
        "zh-TW": "Tab 前進到下一段",
        "ja": "Tab で次の分節へ",
        "de": "Tab springt zum nächsten Segment",
        "fr": "Tab avance au segment suivant",
        "es": "Tab avanza al siguiente segmento",
        "ru": "Tab переходит к следующему сегменту",
        "ar": "Tab ينتقل إلى الفقرة التالية",
        "ko": "Tab으로 다음 세그먼트로",
    },
}

NO_ISSUES = {
    "zh-CN": "当前项目没有问题。",
    "zh-TW": "目前專案沒有問題。",
    "ja": "このプロジェクトに問題はありません。",
    "de": "Keine Probleme in diesem Projekt.",
    "fr": "Aucun problème dans ce projet.",
    "es": "No hay problemas en este proyecto.",
    "ru": "В этом проекте нет проблем.",
    "ar": "لا مسائل في هذا المشروع.",
    "ko": "이 프로젝트에 문제가 없습니다.",
    "pt": "Não há problemas neste projeto.",
    "pt-BR": "Não há problemas neste projeto.",
    "it": "Nessun problema in questo progetto.",
    "nl": "Geen problemen in dit project.",
    "pl": "Brak problemów w tym projekcie.",
    "uk": "У цьому проєкті немає проблем.",
    "cs": "V tomto projektu nejsou žádné problémy.",
    "hu": "Nincs probléma ebben a projektben.",
    "tr": "Bu projede sorun yok.",
    "ca": "No hi ha problemes en aquest projecte.",
    "el": "Δεν υπάρχουν ζητήματα σε αυτό το έργο.",
    "fi": "Tässä projektissa ei ole ongelmia.",
    "sv": "Inga problem i det här projektet.",
    "da": "Ingen problemer i dette projekt.",
    "no": "Ingen problemer i dette prosjektet.",
    "id": "Tidak ada masalah dalam proyek ini.",
    "hr": "Nema problema u ovom projektu.",
    "sk": "V tomto projekte nie sú žiadne problémy.",
    "sl": "V tem projektu ni težav.",
    "gl": "Non hai problemas neste proxecto.",
    "eu": "Proiektu honetan ez dago arazorik.",
    "be": "У гэтым праекце няма праблем.",
    "eo": "Neniuj problemoj en ĉi tiu projekto.",
    "ia": "Nulle problemas in iste projecto.",
    "co": "Micca prublemi in stu prughjettu.",
    "cy": "Dim problemau yn y prosiect hwn.",
    "mfe": "Pa ena problem dan sa projet.",
    "sc": "Perunu problema in custu progetu.",
    "sh": "Nema problema u ovom projektu.",
    "sq": "Nuk ka probleme në këtë projekt.",
    "tk": "Bu taslamada mesele ýok.",
}

FIRST_RUN = {
    "zh-CN": "首次运行",
    "zh-TW": "首次執行",
    "ja": "初回起動",
    "de": "Erster Start",
    "fr": "Premier lancement",
    "es": "Primer inicio",
    "ru": "Первый запуск",
    "ar": "التشغيل الأول",
    "ko": "첫 실행",
    "pt": "Primeira execução",
    "pt-BR": "Primeira execução",
    "it": "Primo avvio",
    "nl": "Eerste start",
    "pl": "Pierwsze uruchomienie",
    "uk": "Перший запуск",
    "cs": "První spuštění",
    "hu": "Első indítás",
    "tr": "İlk çalıştırma",
    "ca": "Primera execució",
    "el": "Πρώτη εκτέλεση",
    "fi": "Ensimmäinen käynnistys",
    "sv": "Första körningen",
    "da": "Første kørsel",
    "no": "Første kjøring",
    "id": "Jalankan pertama",
    "hr": "Prvo pokretanje",
    "sk": "Prvé spustenie",
    "sl": "Prvi zagon",
    "gl": "Primeira execución",
    "eu": "Lehen exekuzioa",
    "be": "Першы запуск",
    "eo": "Unua rulo",
    "ia": "Prime execution",
    "co": "Prima esecuzione",
    "cy": "Rhedeg gyntaf",
    "mfe": "Premye lansman",
    "sc": "Prima esecutzione",
    "sh": "Prvo pokretanje",
    "sq": "Ekzekutimi i parë",
    "tk": "Ilkinji işlediliş",
}

TIP = {
    "zh-CN": "每日提示：Enter 提交并前进到下一段。Ctrl+I 插入最佳匹配。",
    "zh-TW": "每日提示：Enter 提交並前進到下一段。Ctrl+I 插入最佳符合。",
    "ja": "ヒント: Enter で分節を確定して次へ。Ctrl+I で最良一致を挿入。",
    "de": "Tipp: Enter bestätigt das Segment und geht weiter. Strg+I fügt den besten Treffer ein.",
    "fr": "Astuce : Entrée valide le segment et avance. Ctrl+I insère la meilleure correspondance.",
    "es": "Consejo: Intro confirma el segmento y avanza. Ctrl+I inserta la mejor coincidencia.",
    "ru": "Подсказка: Enter подтверждает сегмент и переходит дальше. Ctrl+I вставляет лучшее совпадение.",
    "ar": "نصيحة: Enter يعتمد الفقرة وينتقل إلى التالية. Ctrl+I يدرج أفضل تطابق.",
    "ko": "팁: Enter로 세그먼트를 확정하고 다음으로 이동합니다. Ctrl+I는 최적 일치를 삽입합니다.",
    "pt": "Dica: Enter confirma o segmento e avança. Ctrl+I insere a melhor correspondência.",
    "pt-BR": "Dica: Enter confirma o segmento e avança. Ctrl+I insere a melhor correspondência.",
    "it": "Suggerimento: Invio conferma il segmento e avanza. Ctrl+I inserisce la migliore corrispondenza.",
    "nl": "Tip: Enter bevestigt het segment en gaat verder. Ctrl+I voegt de beste overeenkomst in.",
    "pl": "Wskazówka: Enter zatwierdza segment i przechodzi dalej. Ctrl+I wstawia najlepsze dopasowanie.",
    "uk": "Підказка: Enter підтверджує сегмент і переходить далі. Ctrl+I вставляє найкращий збіг.",
}

EXTRA_EN: dict[str, str] = {
    "menuProject": "Project",
    "menuEdit": "Edit",
    "menuGoto": "Go To",
    "menuView": "View",
    "menuTools": "Tools",
    "menuOptions": "Options",
    "menuHelp": "Help",
    "importFiles": "Add Files...",
    "reload": "Reload",
    "close": "Close",
    "commitSource": "Commit Source Files",
    "commitTarget": "Commit Target Files",
    "compileSingle": "Create Current Translated File",
    "accessProject": "Access Project Contents",
    "accessRoot": "Project Folder",
    "accessDict": "Dictionaries",
    "accessGlossary": "Glossaries",
    "accessSource": "Source Files",
    "accessTarget": "Target Files",
    "accessTm": "TMs",
    "accessExportTm": "Exported TMs",
    "accessCurrentSource": "Current Source File",
    "accessCurrentTarget": "Current Target File",
    "accessWritableGlossary": "Writable Glossary",
    "quit": "Quit",
    "overwriteTranslation": "Replace with Match or Selection",
    "insertTranslation": "Insert Match or Selection",
    "overwriteMt": "Replace with Machine Translation",
    "overwriteSource": "Replace with Source",
    "insertSource": "Insert Source",
    "selectSource": "Select Source Text",
    "tagNext": "Insert Next Missing Tag",
    "tagPainter": "Insert Missing Tags",
    "createGlossary": "Create Glossary Entry...",
    "replaceInProject": "Replace...",
    "searchDict": "Search Dictionaries",
    "switchCase": "Switch Case to",
    "caseLower": "lower case",
    "caseUpper": "UPPER CASE",
    "caseTitle": "Title Case",
    "caseSentence": "Sentence case",
    "caseCycle": "Cycle",
    "selectMatch": "Select Match",
    "matchPrev": "Select Previous Match",
    "matchNext": "Select Next Match",
    "match1": "Select Match #1",
    "match2": "Select Match #2",
    "match3": "Select Match #3",
    "match4": "Select Match #4",
    "match5": "Select Match #5",
    "insertBidi": "Insert Bidi Control Character",
    "multipleDefault": "Use as Default Translation",
    "multipleAlt": "Create Alternative Translation",
    "registerUntranslated": "Remove Translation",
    "registerEmpty": "Set Empty Translation",
    "registerIdentical": "Register Identical Translation",
    "gotoUntranslated": "Next Untranslated Segment",
    "gotoTranslated": "Next Translated Segment",
    "gotoNumber": "Segment Number...",
    "gotoNoteNext": "Next Note",
    "gotoNotePrev": "Previous Note",
    "gotoUnique": "Next Unique Segment",
    "gotoMatchSource": "Source of Selected Match",
    "gotoAutoNext": "Next Segment from tm/auto/",
    "gotoAutoPrev": "Previous Segment from tm/auto/",
    "gotoEnforceNext": "Next Segment from tm/enforce/",
    "gotoEnforcePrev": "Previous Segment from tm/enforce/",
    "gotoHistoryForward": "Forward in History",
    "gotoHistoryBack": "Back in History",
    "gotoNotes": "Notepad",
    "gotoEditor": "Editor",
    "markParagraph": "Display Paragraph Delimitations",
    "displaySource": "Display Source Segments",
    "markNonunique": "Highlight Repeated Segments",
    "markAuto": "Highlight Auto-Populated Segments",
    "markLt": "Mark Language Checker Issues",
    "markFont": "Use Aggressive Font Fallback",
    "modInfo": "Display Modification Info",
    "modNone": "None",
    "modSelected": "for Current Segment",
    "modAll": "for All Segments",
    "restoreGui": "Restore OmegaT Window",
    "issuesFile": "Check Issues for Current File",
    "lastChanges": "Last Changes...",
    "checkUpdates": "Check for Updates...",
    "accessConfig": "Access Configuration Folder",
}

LOCALE_TO_BUNDLE = {
    "ar": "ar",
    "be": "be",
    "ca": "ca",
    "co": "co",
    "cs": "cs",
    "cy": "cy",
    "da": "da",
    "de": "de",
    "el": "el",
    "en": None,
    "eo": "eo",
    "es": "es",
    "eu": "eu",
    "fi": "fi",
    "fr": "fr",
    "gl": "gl",
    "hr": "hr",
    "hu": "hu",
    "ia": "ia",
    "id": "id",
    "it": "it",
    "ja": "ja",
    "ko": "ko",
    "mfe": "mfe",
    "nl": "nl",
    "no": "no",
    "pl": "pl",
    "pt": "pt",
    "pt-BR": "pt_BR",
    "ru": "ru",
    "sc": "sc",
    "sh": "sh",
    "sk": "sk",
    "sl": "sl",
    "sq": "sq",
    "sv": "sv",
    "tk": "tk",
    "tr": "tr",
    "uk": "uk",
    "zh-CN": "zh_CN",
    "zh-TW": "zh_TW",
}

ENGLISH_TAILS = {
    "Auto-completion",
    "Auto-Completion",
    "External searches",
    "User manual",
    "Insert best match",
    "Next segment",
    "Previous segment",
    "Tag validation",
    "Export TMX",
    "Mark whitespace",
    "Mark non-breaking spaces",
    "Mark bidi marks",
    "Untranslated only",
    "Keep ours",
    "Keep theirs",
    "Import MediaWiki",
    "Open MED",
    "Convert project",
    "Use stemming",
}


def unescape_java(s: str) -> str:
    def repl(m: re.Match[str]) -> str:
        return chr(int(m.group(1), 16))

    s = re.sub(r"\\u([0-9a-fA-F]{4})", repl, s)
    s = s.replace("\\n", "\n").replace("\\t", "\t")
    return s


def parse_bundle(path: Path) -> dict[str, str]:
    out: dict[str, str] = {}
    if not path.exists():
        return out
    pending_key: str | None = None
    buf: list[str] = []
    for raw in path.read_text(encoding="utf-8", errors="replace").splitlines():
        line = raw
        if pending_key is not None:
            if line.endswith("\\"):
                buf.append(line[:-1].lstrip())
                continue
            buf.append(line.lstrip())
            out[pending_key] = unescape_java("".join(buf))
            pending_key = None
            buf = []
            continue
        stripped = line.strip()
        if not stripped or stripped.startswith("#") or stripped.startswith("!"):
            continue
        if "=" not in line:
            continue
        k, v = line.split("=", 1)
        k = k.strip()
        if v.endswith("\\"):
            pending_key = k
            buf = [v[:-1]]
        else:
            out[k] = unescape_java(v)
    if pending_key:
        out[pending_key] = unescape_java("".join(buf))
    return out


def clean(s: str) -> str:
    s = re.sub(r"</?html>", "", s, flags=re.I)
    s = re.sub(r"<[^>]+>", "", s)
    s = s.replace("&", "")
    s = re.sub(r"\([A-Za-z0-9]\)", "", s)
    s = re.sub(r"\s*\{[0-9]+\}", "", s)
    s = re.sub(r"\s+", " ", s).strip()
    s = s.rstrip("：:").strip()
    return s


def lookup(bundle: dict[str, str], base: dict[str, str], key: str | None) -> str | None:
    if not key:
        return None
    if key in bundle:
        return clean(bundle[key])
    if key in base:
        return clean(base[key])
    return None


def curated(loc: str, k: str) -> str | None:
    if k == "create":
        return CREATE.get(loc, "Create")
    if k == "app":
        return "OmegaT"
    if k in ("comingLater", "noIssues"):
        return NO_ISSUES.get(loc)
    if k == "firstRun":
        return FIRST_RUN.get(loc)
    if k == "tip":
        return TIP.get(loc)
    table = FALLBACK.get(k) or {}
    return table.get(loc)


def is_english_tail(val: str, en_val: str) -> bool:
    if val == en_val:
        return True
    if val in ENGLISH_TAILS:
        return True
    if val in ("Auto-completion", "Auto-Completion"):
        return True
    return False


def main() -> None:
    en_path = I18N / "en.json"
    en = json.loads(en_path.read_text(encoding="utf-8"))
    for k, v in EXTRA_EN.items():
        en.setdefault(k, v)
    en["create"] = "Create"
    en["completer"] = "Auto-Completion"
    en["app"] = "OmegaT"
    keys = list(en.keys())
    en_path.write_text(json.dumps(en, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")

    base = parse_bundle(BUNDLE_DIR / "Bundle.properties")
    base.update(parse_bundle(TIP_DIR / "Bundle.properties"))

    for loc, suffix in LOCALE_TO_BUNDLE.items():
        dest = I18N / f"{loc}.json"
        existing = json.loads(dest.read_text(encoding="utf-8")) if dest.exists() else {}
        if loc == "en":
            dest.write_text(json.dumps(en, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
            print(f"wrote {dest.relative_to(ROOT)} ({len(en)} keys)")
            continue
        bundle = dict(base)
        if suffix:
            bundle.update(parse_bundle(BUNDLE_DIR / f"Bundle_{suffix}.properties"))
            bundle.update(parse_bundle(TIP_DIR / f"Bundle_{suffix}.properties"))
        out: dict[str, str] = {}
        for k in keys:
            val = curated(loc, k)
            if not val:
                jk = KEY_MAP.get(k)
                val = lookup(bundle, base, jk)
            if not val:
                prev = existing.get(k)
                if prev and not is_english_tail(prev, en[k]):
                    val = prev
                else:
                    val = en[k]
            if val in ("Auto-completion", "Auto-Completion") and k != "completer":
                alt = lookup(bundle, base, "PREFS_TITLE_AUTOCOMPLETER")
                if alt:
                    val = alt
            if k == "completer" and val in ("Auto-completion", "Auto-Completion"):
                alt = lookup(bundle, base, "PREFS_TITLE_AUTOCOMPLETER")
                if alt and alt not in ("Auto-completion", "Auto-Completion"):
                    val = alt
            out[k] = val
        dest.write_text(json.dumps(out, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        print(f"wrote {dest.relative_to(ROOT)} ({len(out)} keys)")


if __name__ == "__main__":
    main()
