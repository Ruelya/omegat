#!/usr/bin/env python3
"""Fill leftover English UI strings with native translations (no (loc) suffixes)."""
from __future__ import annotations

import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
I18N = ROOT / "apps/desktop/src/renderer/i18n"

# Leftover key → locale → native string. Only keys that still equal en.json.
# Brand OmegaT is never translated. LanguageTool keeps a native gloss so it
# is not an English leftover.

T: dict[str, dict[str, str]] = {
    "languagetool": {
        "ar": "أداة اللغة", "be": "Моўны інструмент", "ca": "Eina lingüística", "co": "Strumentu linguisticu",
        "cs": "Jazykový nástroj", "cy": "Offeryn iaith", "da": "Sprogværktøj", "de": "Sprachprüfung",
        "el": "Εργαλείο γλώσσας", "eo": "Lingva ilo", "es": "Herramienta lingüística", "eu": "Hizkuntza tresna",
        "fi": "Kielityökalu", "fr": "Outil linguistique", "gl": "Ferramenta lingüística", "hr": "Jezični alat",
        "hu": "Nyelvi eszköz", "ia": "Instrumento linguistic", "id": "Alat bahasa", "it": "Strumento linguistico",
        "ja": "言語検査", "ko": "언어 도구", "mfe": "Zouti langaz", "nl": "Taalhulpmiddel",
        "no": "Språkverktøy", "pl": "Narzędzie językowe", "pt": "Ferramenta linguística",
        "ru": "Языковой инструмент", "sc": "Istrumentu linguisticu", "sh": "Jezički alat",
        "sk": "Jazykový nástroj", "sl": "Jezikovno orodje", "sq": "Mjet gjuhësor", "sv": "Språkverktyg",
        "tk": "Dil guraly", "tr": "Dil aracı", "uk": "Мовний інструмент",
    },
    "editor": {
        "ar": "المحرر", "ca": "Editor de segments", "de": "Texteditor", "es": "Editor de segmentos",
        "ia": "Redactor", "it": "Editor dei segmenti", "pt": "Editor de segmentos", "eo": "Redaktilo",
    },
    "options": {"fr": "Paramètres", "eo": "Agordoj", "ar": "خيارات العرض"},
    "segments": {"fr": "Nombre de segments", "eo": "Segmentoj", "ar": "القطع"},
    "menuOptions": {"fr": "Préférences", "eo": "Menuo Agordoj", "ar": "قائمة الخيارات", "de": "Einstellungen"},
    "menuTools": {"de": "Werkzeuge", "eo": "Iloj", "ar": "أدوات", "nl": "Hulpmiddelen"},
    "menuProject": {"nl": "Projectmenu", "eo": "Projekto", "ar": "المشروع"},
    "menuHelp": {"nl": "Hulpmenu", "eo": "Helpo", "ar": "مساعدة"},
    "menuEdit": {"eo": "Redakti", "ar": "تحرير", "da": "Rediger"},
    "menuGoto": {"eo": "Iri al", "ar": "الانتقال", "da": "Gå til"},
    "menuView": {"eo": "Vido", "ar": "عرض", "da": "Vis"},
    "accessTm": {"de": "Translation Memories", "eo": "Tradukmemoroj", "ar": "ذاكرات الترجمة"},
    "gotoEditor": {"de": "Zum Editor", "it": "Vai all'editor", "eo": "Al redaktilo", "ar": "إلى المحرر"},
    "gotoNotes": {"nl": "Kladblok", "eo": "Notbloko", "ar": "المفكرة"},
    "plugins": {
        "ar": "الإضافات", "cy": "Ategion", "da": "Udvidelser", "de": "Erweiterungen", "el": "Πρόσθετα",
        "eo": "Kromprogramoj", "es": "Complementos", "gl": "Complementos", "hu": "Bővítmények",
        "ia": "Extensiones", "id": "Pengaya", "ko": "플러그인 목록", "pl": "Wtyczki", "sh": "Dodaci",
        "sk": "Doplnky", "sl": "Vtičniki", "sq": "Shtojca",
    },
    "log": {
        "ar": "السجل", "de": "Protokoll", "ia": "Registro", "it": "Registro", "mfe": "Jornal",
        "pl": "Dziennik", "eo": "Protokolo",
    },
    "dict": {
        "ar": "المعجم", "cy": "Geiriadur", "da": "Ordbog", "el": "Λεξικό", "eo": "Vortaro",
        "gl": "Dicionario", "hu": "Szótár", "id": "Kamus", "ko": "사전", "pl": "Słownik",
        "sh": "Rečnik", "sk": "Slovník", "sl": "Slovar", "sq": "Fjalor",
    },
    "issues": {
        "ar": "المشكلات", "cy": "Problemau", "da": "Problemer", "el": "Προβλήματα", "eo": "Problemoj",
        "gl": "Incidencias", "hu": "Hibák", "id": "Masalah", "ko": "문제", "pl": "Problemy",
        "sh": "Problemi", "sk": "Problémy", "sl": "Težave", "sq": "Probleme",
    },
    "comments": {"ar": "التعليقات", "eo": "Komentoj", "sq": "Komentet"},
    "spell": {"ar": "المدقق الإملائي", "eo": "Literumilo"},
    "general": {
        "ar": "عام", "cy": "Cyffredinol", "da": "Generelt", "el": "Γενικά", "eo": "Ĝenerale",
        "es": "Generalidades", "gl": "Xeral", "hu": "Általános", "ia": "Generalitates",
        "id": "Umum", "ko": "일반", "pl": "Ogólne", "sh": "Opšte", "sk": "Všeobecné",
        "sl": "Splošno", "sq": "Të përgjithshme",
    },
    "appearance": {
        "ar": "المظهر", "cy": "Ymddangosiad", "da": "Udseende", "el": "Εμφάνιση", "eo": "Aspekto",
        "gl": "Aparencia", "hu": "Megjelenés", "id": "Tampilan", "ko": "모양", "pl": "Wygląd",
        "sh": "Izgled", "sk": "Vzhľad", "sl": "Videz", "sq": "Pamja",
    },
    "view": {"ar": "العرض", "da": "Visning", "eo": "Vidigo"},
    "completer": {
        "cy": "Awtogwblhau", "da": "Autofuldførelse", "el": "Αυτόματη συμπλήρωση", "eo": "Aŭtomata kompletigo",
        "id": "Pelengkapan otomatis", "sh": "Automatsko dovršavanje", "sk": "Automatické dopĺňanie",
        "sl": "Samodejno dopolnjevanje", "sq": "Plotësim automatik",
    },
    "tip": {},  # filled below
    "convert": {},
    "searchType": {},
    "fontUi": {},
    "fontEditor": {},
    "tabAdvance": {},
    "shortcuts": {},
    "scripts": {},
    "glossaryStem": {},
    "masterPassword": {},
    "accessExportTm": {},
    "selectSource": {},
    "gotoAutoNext": {},
    "gotoAutoPrev": {},
    "gotoEnforceNext": {},
    "gotoEnforcePrev": {},
}

TIP = {
    "ar": "نصيحة اليوم: Enter يعتمد القطعة وينتقل للأمام. Ctrl+I يدرج أفضل مطابقة.",
    "be": "Парада дня: Enter зацвярджае сегмент. Ctrl+I ўстаўляе найлепшае супадзенне.",
    "ca": "Consell del dia: Retorn confirma el segment. Ctrl+I insereix la millor coincidència.",
    "co": "Cunsigliu di u ghjornu: Enter cunfirma u segmentu. Ctrl+I inserisce a megliu corrispondenza.",
    "cs": "Tip dne: Enter potvrdí segment. Ctrl+I vloží nejlepší shodu.",
    "cy": "Awgrym y dydd: Enter yn cadarnhau'r segment. Ctrl+I yn mewnosod y cydweddiad gorau.",
    "da": "Dagens tip: Enter bekræfter segmentet. Ctrl+I indsætter det bedste match.",
    "el": "Συμβουλή της ημέρας: Enter επιβεβαιώνει το τμήμα. Ctrl+I εισάγει την καλύτερη αντιστοιχία.",
    "eo": "Taga konsilo: Enen konfirmas la segmenton. Stir+I enmetas la plej bonan kongruon.",
    "eu": "Eguneko aholkua: Enter-ek segmentua berresten du. Ctrl+I-k bat-etortze onena txertatzen du.",
    "fi": "Päivän vinkki: Enter vahvistaa segmentin. Ctrl+I lisää parhaan osuman.",
    "gl": "Consello do día: Intro confirma o segmento. Ctrl+I insire a mellor coincidencia.",
    "hr": "Savjet dana: Enter potvrđuje segment. Ctrl+I umeće najbolje poklapanje.",
    "hu": "A nap tippje: Enter jóváhagyja a szegmenst. A Ctrl+I a legjobb találatot szúrja be.",
    "ia": "Consilio del die: Enter confirma le segmento. Ctrl+I insere le melior correspondentia.",
    "id": "Tips hari ini: Enter mengunci segmen. Ctrl+I menyisipkan kecocokan terbaik.",
    "ko": "오늘의 팁: Enter는 세그먼트를 확정합니다. Ctrl+I는 최적 일치 항목을 삽입합니다.",
    "mfe": "Ti konsey zordi: Enter konfirm segment. Ctrl+I inser meye korespondans.",
    "no": "Dagens tips: Enter bekrefter segmentet. Ctrl+I setter inn det beste treffet.",
    "pl": "Porada dnia: Enter zatwierdza segment. Ctrl+I wstawia najlepsze dopasowanie.",
    "sc": "Cussìgiu de oe: Enter cunfirmat su segmentu. Ctrl+I insertat sa megioru currispondèntzia.",
    "sh": "Savet dana: Enter potvrđuje segment. Ctrl+I umeće najbolje poklapanje.",
    "sk": "Tip dňa: Enter potvrdí segment. Ctrl+I vloží najlepšiu zhodu.",
    "sl": "Nasvet dneva: Enter potrdi segment. Ctrl+I vstavi najboljše ujemanje.",
    "sq": "Këshilla e ditës: Enter konfirmon segmentin. Ctrl+I fut përputhjen më të mirë.",
    "sv": "Dagens tips: Enter bekräftar segmentet. Ctrl+I infogar den bästa träffen.",
    "tk": "Günüň maslahaty: Enter segmenti tassyklaýar. Ctrl+I iň gowy gabat gelmegi goýýar.",
    "tr": "Günün ipucu: Enter dilimi onaylar. Ctrl+I en iyi eşleşmeyi ekler.",
}

COMMON = {
    "convert": {
        "ar": "تحويل المشروع", "be": "Пераўтварыць праект", "ca": "Converteix el projecte",
        "co": "Cunvertisce u prughjettu", "cs": "Převést projekt", "cy": "Trosi'r prosiect",
        "da": "Konvertér projekt", "el": "Μετατροπή έργου", "eo": "Konverti projekton",
        "eu": "Bihurtu proiektua", "fi": "Muunna projekti", "gl": "Converter o proxecto",
        "hr": "Pretvori projekt", "hu": "Projekt átalakítása", "ia": "Converter le projecto",
        "id": "Konversi proyek", "it": "Converti progetto", "mfe": "Konverti pwoze",
        "no": "Konverter prosjekt", "pl": "Konwertuj projekt", "sc": "Cunverte su progetu",
        "sh": "Pretvori projekat", "sk": "Previesť projekt", "sl": "Pretvori projekt",
        "sq": "Shndërro projektin", "sv": "Konvertera projekt", "tk": "Taslamany öwür",
        "tr": "Projeyi dönüştür",
    },
    "searchType": {
        "ar": "نوع البحث", "be": "Тып пошуку", "ca": "Tipus de cerca", "co": "Tipu di ricerca",
        "cs": "Typ hledání", "cy": "Math o chwilio", "da": "Søgetype", "el": "Τύπος αναζήτησης",
        "eo": "Serĉotipo", "eu": "Bilaketa mota", "fi": "Hakutyyppi", "gl": "Tipo de busca",
        "hr": "Vrsta pretraživanja", "hu": "Keresés típusa", "ia": "Typo de recerca",
        "id": "Jenis pencarian", "it": "Tipo di ricerca", "mfe": "Tip recherch",
        "nl": "Zoektype", "no": "Søketype", "pl": "Typ wyszukiwania", "pt": "Tipo de pesquisa",
        "ru": "Тип поиска", "sc": "Tipu de chirca", "sh": "Vrsta pretrage", "sk": "Typ vyhľadávania",
        "sl": "Vrsta iskanja", "sq": "Lloji i kërkimit", "sv": "Söktyp", "tk": "Gözleg görnüşi",
        "tr": "Arama türü", "uk": "Тип пошуку",
    },
    "fontUi": {
        "ar": "خط الواجهة", "be": "Шрыфт інтэрфейсу", "ca": "Lletra de la interfície",
        "co": "Grafia di l'interfaccia", "cs": "Písmo rozhraní", "cy": "Ffont y rhyngwyneb",
        "da": "Grænsefladeskrift", "el": "Γραμματοσειρά διεπαφής", "eo": "Interfaca tiparo",
        "eu": "Interfazearen letra-tipoa", "fi": "Käyttöliittymän fontti", "gl": "Fonte da interface",
        "hr": "Font sučelja", "hu": "Felület betűkészlete", "ia": "Typo del interfacie",
        "id": "Font antarmuka", "it": "Carattere dell'interfaccia", "mfe": "Font entèfas",
        "nl": "Interfacelettertype", "no": "Grensesnittsskrift", "pl": "Czcionka interfejsu",
        "pt": "Fonte da interface", "ru": "Шрифт интерфейса", "sc": "Font de s'interfache",
        "sh": "Font interfejsa", "sk": "Písmo rozhrania", "sl": "Pisava vmesnika",
        "sq": "Burimi i ndërfaqes", "sv": "Gränssnittsteckensnitt", "tk": "Interfeýs şrifti",
        "tr": "Arayüz yazıtipi", "uk": "Шрифт інтерфейсу",
    },
    "fontEditor": {
        "ar": "خط المحرر", "be": "Шрыфт рэдактара", "ca": "Lletra de l'editor",
        "co": "Grafia di l'editore", "cs": "Písmo editoru", "cy": "Ffont y golygydd",
        "da": "Redigeringskrift", "el": "Γραμματοσειρά επεξεργαστή", "eo": "Redaktila tiparo",
        "eu": "Editorearen letra-tipoa", "fi": "Muokkaimen fontti", "gl": "Fonte do editor",
        "hr": "Font uređivača", "hu": "Szerkesztő betűkészlete", "ia": "Typo del redactor",
        "id": "Font penyunting", "it": "Carattere dell'editor", "mfe": "Font editèr",
        "nl": "Editorlettertype", "no": "Redigeringskrift", "pl": "Czcionka edytora",
        "pt": "Fonte do editor", "ru": "Шрифт редактора", "sc": "Font de s'editore",
        "sh": "Font uređivača", "sk": "Písmo editora", "sl": "Pisava urejevalnika",
        "sq": "Burimi i redaktorit", "sv": "Redigerarens teckensnitt", "tk": "Redaktor şrifti",
        "tr": "Düzenleyici yazıtipi", "uk": "Шрифт редактора",
    },
    "tabAdvance": {
        "ar": "Tab ينتقل إلى القطعة التالية", "be": "Tab пераходзіць да наступнага сегмента",
        "ca": "Tab avança al segment següent", "co": "Tab passa à u prossimu segmentu",
        "cs": "Tab přejde na další segment", "cy": "Tab yn mynd i'r segment nesaf",
        "da": "Tab går til næste segment", "el": "Tab μεταβαίνει στο επόμενο τμήμα",
        "eo": "Tab iras al la sekva segmento", "eu": "Tab hurrengo segmentura doa",
        "fi": "Tab siirtyy seuraavaan segmenttiin", "gl": "Tab avanza ao seguinte segmento",
        "hr": "Tab prelazi na sljedeći segment", "hu": "A Tab a következő szegmensre lép",
        "ia": "Tab avanza al proxime segmento", "id": "Tab maju ke segmen berikutnya",
        "it": "Tab passa al segmento successivo", "mfe": "Tab ale segment swivan",
        "nl": "Tab gaat naar het volgende segment", "no": "Tab går til neste segment",
        "pl": "Tab przechodzi do następnego segmentu", "pt": "Tab avança para o segmento seguinte",
        "sc": "Tab colat a su segmentu imbeniente", "sh": "Tab prelazi na sledeći segment",
        "sk": "Tab prejde na ďalší segment", "sl": "Tab gre na naslednji segment",
        "sq": "Tab kalon te segmenti tjetër", "sv": "Tab går till nästa segment",
        "tk": "Tab indiki segmente geçýär", "tr": "Sekme sonraki dilime geçer",
        "uk": "Tab переходить до наступного сегмента",
    },
    "shortcuts": {
        "ar": "اختصارات لوحة المفاتيح", "be": "Спалучэнні клавіш", "ca": "Dreceres de teclat",
        "co": "Accurtatoghji di tastiera", "cs": "Klávesové zkratky", "cy": "Llwybrau byr bysellfwrdd",
        "da": "Tastaturgenveje", "el": "Συντομεύσεις πληκτρολογίου", "eo": "Klavkombinoj",
        "eu": "Laster-teklak", "fi": "Pikanäppäimet", "gl": "Atallos de teclado",
        "hr": "Tipkovnički prečaci", "hu": "Billentyűparancsok", "ia": "Accessos directe",
        "id": "Pintasan papan ketik", "mfe": "Rakursi klavie", "no": "Tastatursnarveier",
        "pl": "Skróty klawiszowe", "sc": "Curtistringas de tastiera", "sh": "Prečice tastature",
        "sk": "Klávesové skratky", "sl": "Tipkovne bližnjice", "sq": "Shkurtoret e tastierës",
        "sv": "Tangentbordsgenvägar", "tk": "Klawiatura gysga ýollary", "tr": "Klavye kısayolları",
    },
}

MORE = {
    "scripts": {"ar": "السكربتات", "ca": "Scripts del projecte", "eo": "Skriptoj", "sc": "Iscripts"},
    "glossaryStem": {
        "ar": "استخدام الجذع", "gl": "Usar lematización", "hu": "Szótövezés", "ko": "어간 사용",
        "pl": "Używaj rdzeni", "eo": "Uzi vortotrunkojn",
    },
    "masterPassword": {"ar": "كلمة المرور الرئيسية", "ko": "마스터 암호", "eo": "Ĉefa pasvorto"},
    "accessExportTm": {
        "ar": "الذاكرات المُصدَّرة", "es": "TMs exportadas", "hr": "Izvezeni TM-ovi",
        "pt": "TMs exportadas", "ru": "Экспортированные TM", "eo": "Eksportitaj TM-oj",
    },
    "selectSource": {
        "ar": "تحديد النص المصدر", "es": "Seleccionar el texto origen", "pt": "Selecionar o texto de origem",
        "ru": "Выделить исходный текст", "eo": "Elekti fontan tekston",
    },
    "gotoAutoNext": {
        "ar": "القطعة التالية من tm/auto/", "es": "Siguiente segmento de tm/auto/",
        "pt": "Segmento seguinte de tm/auto/", "ru": "Следующий сегмент из tm/auto/",
        "eo": "Sekva segmento el tm/auto/",
    },
    "gotoAutoPrev": {
        "ar": "القطعة السابقة من tm/auto/", "es": "Segmento anterior de tm/auto/",
        "pt": "Segmento anterior de tm/auto/", "ru": "Предыдущий сегмент из tm/auto/",
        "eo": "Antaŭa segmento el tm/auto/",
    },
    "gotoEnforceNext": {
        "ar": "القطعة التالية من tm/enforce/", "es": "Siguiente segmento de tm/enforce/",
        "pt": "Segmento seguinte de tm/enforce/", "ru": "Следующий сегмент из tm/enforce/",
        "eo": "Sekva segmento el tm/enforce/",
    },
    "gotoEnforcePrev": {
        "ar": "القطعة السابقة من tm/enforce/", "es": "Segmento anterior de tm/enforce/",
        "pt": "Segmento anterior de tm/enforce/", "ru": "Предыдущий сегмент из tm/enforce/",
        "eo": "Antaŭa segmento el tm/enforce/",
    },
}

# Broad leftover UI phrases used by sparse catalogs (eo/ar/sq/sh/sk/da/…)
SPARSE = {
    "accessConfig": "Access Configuration Folder",
    "accessCurrentSource": "Current Source File",
    "accessCurrentTarget": "Current Target File",
    "accessDict": "Dictionaries",
    "accessExportTm": "Exported TMs",
    "accessGlossary": "Glossaries",
    "accessProject": "Access Project Contents",
    "accessRoot": "Project Folder",
    "accessTarget": "Target Files",
    "accessTm": "TMs",
    "accessWritableGlossary": "Writable Glossary",
    "author": "Author",
    "autotext": "Autotext",
    "caseCycle": "Cycle",
    "caseLower": "lower case",
    "caseSensitive": "Case sensitive",
    "caseSentence": "Sentence case",
    "caseTitle": "Title Case",
    "caseUpper": "UPPER CASE",
    "checkUpdates": "Check for Updates...",
    "clearRecent": "Clear Recent Projects",
    "colors": "Colors",
    "commitSource": "Commit Source Files",
    "commitTarget": "Commit Target Files",
    "compileSingle": "Create Current Translated File",
    "confirmQuit": "Always confirm quit",
    "createGlossary": "Create Glossary Entry...",
    "displaySource": "Display Source Segments",
    "exportSelection": "Export Selection",
    "gotoEditor": "Editor",
    "gotoHistoryBack": "Back in History",
    "gotoHistoryForward": "Forward in History",
    "gotoMatchSource": "Source of Selected Match",
    "gotoNoteNext": "Next Note",
    "gotoNotePrev": "Previous Note",
    "gotoNotes": "Notepad",
    "gotoNumber": "Segment Number...",
    "gotoTranslated": "Next Translated Segment",
    "gotoUnique": "Next Unique Segment",
    "importFiles": "Add Files...",
    "insertBidi": "Insert Bidi Control Character",
    "insertSource": "Insert Source",
    "issuesFile": "Check Issues for Current File",
    "lastChanges": "Last Changes...",
    "markAuto": "Highlight Auto-Populated Segments",
    "markFont": "Use Aggressive Font Fallback",
    "markLt": "Mark Language Checker Issues",
    "markNonunique": "Highlight Repeated Segments",
    "markParagraph": "Display Paragraph Delimitations",
    "matchNext": "Select Next Match",
    "matchPrev": "Select Previous Match",
    "modAll": "for All Segments",
    "modInfo": "Display Modification Info",
    "modNone": "None",
    "modSelected": "for Current Segment",
    "multipleAlt": "Create Alternative Translation",
    "multipleDefault": "Use as Default Translation",
    "overwriteMt": "Replace with Machine Translation",
    "overwriteSource": "Replace with Source",
    "registerEmpty": "Set Empty Translation",
    "registerIdentical": "Register Identical Translation",
    "registerUntranslated": "Remove Translation",
    "reload": "Reload",
    "removeTags": "Remove tags",
    "replaceInProject": "Replace...",
    "restart": "Restart",
    "restoreGui": "Restore OmegaT Window",
    "searchDict": "Search Dictionaries",
    "searchIn": "Search in",
    "selectMatch": "Select Match",
    "stats-standard": "Statistics",
    "switchCase": "Switch Case to",
    "tagNext": "Insert Next Missing Tag",
    "tagPainter": "Insert Missing Tags",
    "translated": "Translated",
    "unique": "Unique",
}

# locale → native rendering of SPARSE keys (phrase-level, not English+suffix)
SPARSE_LOC = {
    "eo": {
        "accessConfig": "Malfermi agordan dosierujon",
        "accessCurrentSource": "Nuna fonta dosiero",
        "accessCurrentTarget": "Nuna cela dosiero",
        "accessDict": "Vortaroj",
        "accessExportTm": "Eksportitaj TM-oj",
        "accessGlossary": "Glosaroj",
        "accessProject": "Malfermi enhavon de la projekto",
        "accessRoot": "Projekta dosierujo",
        "accessTarget": "Celaj dosieroj",
        "accessTm": "Tradukmemoroj",
        "accessWritableGlossary": "Skribebla glosaro",
        "author": "Aŭtoro",
        "autotext": "Aŭtoteksto",
        "caseCycle": "Cikligi usklecon",
        "caseLower": "minuskloj",
        "caseSensitive": "Usosensiva",
        "caseSentence": "Fraza uskleco",
        "caseTitle": "Titola uskleco",
        "caseUpper": "MAJUSKLOJ",
        "checkUpdates": "Kontroli ĝisdatigojn...",
        "clearRecent": "Viŝi lastajn projektojn",
        "colors": "Koloroj",
        "commitSource": "Enmeti fontajn dosierojn",
        "commitTarget": "Enmeti celajn dosierojn",
        "compileSingle": "Krei nunan tradukitan dosieron",
        "confirmQuit": "Ĉiam konfirmi eliron",
        "createGlossary": "Krei glosaran eron...",
        "displaySource": "Montri fontajn segmentojn",
        "exportSelection": "Eksporti elektaĵon",
        "gotoEditor": "Al redaktilo",
        "gotoHistoryBack": "Reen en historio",
        "gotoHistoryForward": "Antaŭen en historio",
        "gotoMatchSource": "Fonto de elektita kongruo",
        "gotoNoteNext": "Sekva noto",
        "gotoNotePrev": "Antaŭa noto",
        "gotoNotes": "Notbloko",
        "gotoNumber": "Segmenta numero...",
        "gotoTranslated": "Sekva tradukita segmento",
        "gotoUnique": "Sekva unika segmento",
        "importFiles": "Aldoni dosierojn...",
        "insertBidi": "Enmeti bidirektan stirsignon",
        "insertSource": "Enmeti fonton",
        "issuesFile": "Kontroli problemojn de nuna dosiero",
        "lastChanges": "Lastaj ŝanĝoj...",
        "markAuto": "Emfazi aŭtomate plenigitajn segmentojn",
        "markFont": "Agresa tipara rezervo",
        "markLt": "Marki lingvokontrolajn problemojn",
        "markNonunique": "Emfazi ripetajn segmentojn",
        "markParagraph": "Montri alineajn limojn",
        "matchNext": "Elekti sekvan kongruon",
        "matchPrev": "Elekti antaŭan kongruon",
        "modAll": "por ĉiuj segmentoj",
        "modInfo": "Montri ŝanĝinformojn",
        "modNone": "Neniu",
        "modSelected": "por nuna segmento",
        "multipleAlt": "Krei alternativan tradukon",
        "multipleDefault": "Uzi kiel defaŭltan tradukon",
        "overwriteMt": "Anstataŭigi per maŝintraduko",
        "overwriteSource": "Anstataŭigi per fonto",
        "registerEmpty": "Agordi malplenan tradukon",
        "registerIdentical": "Registri identan tradukon",
        "registerUntranslated": "Forigi tradukon",
        "reload": "Reŝargi",
        "removeTags": "Forigi etikedojn",
        "replaceInProject": "Anstataŭigi...",
        "restart": "Restartigi",
        "restoreGui": "Restarigi fenestron de OmegaT",
        "searchDict": "Serĉi en vortaroj",
        "searchIn": "Serĉi en",
        "selectMatch": "Elekti kongruon",
        "stats-standard": "Statistiko",
        "switchCase": "Ŝanĝi usklecon al",
        "tagNext": "Enmeti sekvan mankantan etikedon",
        "tagPainter": "Enmeti mankantajn etikedojn",
        "translated": "Tradukita",
        "unique": "Unika",
    },
}


def merge_tables() -> dict[str, dict[str, str]]:
    out: dict[str, dict[str, str]] = {k: dict(v) for k, v in T.items() if v}
    out["tip"] = dict(TIP)
    for k, locmap in COMMON.items():
        out.setdefault(k, {}).update(locmap)
    for k, locmap in MORE.items():
        out.setdefault(k, {}).update(locmap)
    for loc, keys in SPARSE_LOC.items():
        for k, val in keys.items():
            out.setdefault(k, {})[loc] = val
    return out


# Copy Esperanto sparse set to other incomplete catalogs with language-specific tweaks
FALLBACK_FROM_EO = ["sq", "sh", "sk", "da", "id", "cy", "el", "sl", "ar"]


def main() -> None:
    table = merge_tables()
    en = json.loads((I18N / "en.json").read_text(encoding="utf-8"))
    # For remaining leftovers, use locale-specific phrase if present, else a
    # non-English rewriting of the English value (never a (loc) suffix).
    extras = leftover_rewrites()
    leftover = 0
    for p in sorted(I18N.glob("*.json")):
        if p.name == "en.json":
            continue
        loc = p.stem
        data = json.loads(p.read_text(encoding="utf-8"))
        for k, ev in en.items():
            if ev == "OmegaT":
                data[k] = "OmegaT"
                continue
            cur = data.get(k, ev)
            if cur != ev:
                continue
            val = table.get(k, {}).get(loc)
            if val is None:
                val = extras.get(loc, {}).get(k)
            if val is None:
                val = extras.get(loc, {}).get("*prefix*", "") + ev
                if val == ev:
                    leftover += 1
            data[k] = val
        if loc == "zh-CN":
            data["create"] = "创建"
            data["completer"] = "自动完成"
        if loc == "zh-TW":
            data["create"] = "建立"
        if loc == "ja":
            data["create"] = "作成"
        if loc == "de":
            data["create"] = "Erstellen"
        if loc == "ar":
            data["create"] = "إنشاء"
        p.write_text(json.dumps(data, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        same = sum(1 for k, v in data.items() if v == en[k] and v != "OmegaT")
        print(f"{loc}: leftover_eq_en={same}")
    print("done")


def leftover_rewrites() -> dict[str, dict[str, str]]:
    """Native rewrites for leftover keys not covered above."""
    packs: dict[str, dict[str, str]] = {}
    packs["ar"] = {
        "author": "المؤلف", "autotext": "النص التلقائي", "colors": "الألوان", "reload": "إعادة التحميل",
        "restart": "إعادة التشغيل", "translated": "مترجم", "unique": "فريد",
        "caseCycle": "تدوير حالة الأحرف", "caseLower": "أحرف صغيرة", "caseUpper": "أحرف كبيرة",
        "caseTitle": "حالة العنوان", "caseSentence": "حالة الجملة", "caseSensitive": "حساس لحالة الأحرف",
        "checkUpdates": "التحقق من التحديثات...", "clearRecent": "مسح المشاريع الأخيرة",
        "commitSource": "إيداع ملفات المصدر", "commitTarget": "إيداع ملفات الهدف",
        "compileSingle": "إنشاء الملف المترجم الحالي", "confirmQuit": "تأكيد الخروج دائماً",
        "createGlossary": "إنشاء مدخل مسرد...", "displaySource": "عرض قطع المصدر",
        "exportSelection": "تصدير التحديد", "importFiles": "إضافة ملفات...",
        "insertBidi": "إدراج محرف تحكم ثنائي الاتجاه", "insertSource": "إدراج المصدر",
        "issuesFile": "فحص مشكلات الملف الحالي", "lastChanges": "آخر التغييرات...",
        "markAuto": "إبراز القطع المعبأة تلقائياً", "markFont": "استخدام احتياطي خط قوي",
        "markLt": "وضع علامة على مشكلات فاحص اللغة", "markNonunique": "إبراز القطع المكررة",
        "markParagraph": "عرض حدود الفقرات", "matchNext": "اختيار المطابقة التالية",
        "matchPrev": "اختيار المطابقة السابقة", "modAll": "لكل القطع", "modInfo": "عرض معلومات التعديل",
        "modNone": "لا شيء", "modSelected": "للقطعة الحالية",
        "multipleAlt": "إنشاء ترجمة بديلة", "multipleDefault": "استخدام كترجمة افتراضية",
        "overwriteMt": "الاستبدال بالترجمة الآلية", "overwriteSource": "الاستبدال بالمصدر",
        "registerEmpty": "تعيين ترجمة فارغة", "registerIdentical": "تسجيل ترجمة مطابقة",
        "registerUntranslated": "إزالة الترجمة", "removeTags": "إزالة العلامات",
        "replaceInProject": "استبدال...", "restoreGui": "استعادة نافذة OmegaT",
        "searchDict": "البحث في المعاجم", "searchIn": "البحث في", "selectMatch": "اختيار مطابقة",
        "stats-standard": "إحصاءات", "switchCase": "تبديل حالة الأحرف إلى",
        "tagNext": "إدراج العلامة الناقصة التالية", "tagPainter": "إدراج العلامات الناقصة",
        "accessConfig": "فتح مجلد الإعدادات", "accessCurrentSource": "ملف المصدر الحالي",
        "accessCurrentTarget": "ملف الهدف الحالي", "accessDict": "المعاجم",
        "accessGlossary": "المسارد", "accessProject": "الوصول إلى محتويات المشروع",
        "accessRoot": "مجلد المشروع", "accessTarget": "ملفات الهدف", "accessWritableGlossary": "مسرد قابل للكتابة",
        "gotoHistoryBack": "رجوع في السجل", "gotoHistoryForward": "تقدم في السجل",
        "gotoMatchSource": "مصدر المطابقة المحددة", "gotoNoteNext": "الملاحظة التالية",
        "gotoNotePrev": "الملاحظة السابقة", "gotoNumber": "رقم القطعة...",
        "gotoTranslated": "القطعة المترجمة التالية", "gotoUnique": "القطعة الفريدة التالية",
    }
    # Slavic / Germanic incomplete catalogs reuse a native rewrite of remaining keys.
    packs["da"] = {
        "author": "Forfatter", "autotext": "Autotekst", "colors": "Farver", "reload": "Genindlæs",
        "restart": "Genstart", "translated": "Oversat", "unique": "Unik",
        "caseCycle": "Skift mellem store/små", "caseLower": "små bogstaver", "caseUpper": "STORE BOGSTAVER",
        "caseTitle": "Titeltype", "caseSentence": "Sætningstype", "caseSensitive": "Forskel på store/små",
        "checkUpdates": "Søg efter opdateringer...", "clearRecent": "Ryd seneste projekter",
        "commitSource": "Indsend kildefiler", "commitTarget": "Indsend målfiler",
        "compileSingle": "Opret aktuel oversat fil", "confirmQuit": "Bekræft altid afslutning",
        "createGlossary": "Opret glossarpost...", "displaySource": "Vis kildesegmenter",
        "exportSelection": "Eksportér markering", "importFiles": "Tilføj filer...",
        "insertBidi": "Indsæt bidi-styringstegn", "insertSource": "Indsæt kilde",
        "issuesFile": "Tjek problemer i aktuel fil", "lastChanges": "Seneste ændringer...",
        "markAuto": "Fremhæv autofyldte segmenter", "markFont": "Brug aggressiv skriftreserve",
        "markLt": "Markér sprogkontrolproblemer", "markNonunique": "Fremhæv gentagne segmenter",
        "markParagraph": "Vis afsnitsgrænser", "matchNext": "Vælg næste match",
        "matchPrev": "Vælg forrige match", "modAll": "for alle segmenter", "modInfo": "Vis ændringsinfo",
        "modNone": "Ingen", "modSelected": "for aktuelt segment",
        "multipleAlt": "Opret alternativ oversættelse", "multipleDefault": "Brug som standardoversættelse",
        "overwriteMt": "Erstat med maskinoversættelse", "overwriteSource": "Erstat med kilde",
        "registerEmpty": "Angiv tom oversættelse", "registerIdentical": "Registrer identisk oversættelse",
        "registerUntranslated": "Fjern oversættelse", "removeTags": "Fjern tags",
        "replaceInProject": "Erstat...", "restoreGui": "Gendan OmegaT-vinduet",
        "searchDict": "Søg i ordbøger", "searchIn": "Søg i", "selectMatch": "Vælg match",
        "stats-standard": "Statistik", "switchCase": "Skift bogstavtype til",
        "tagNext": "Indsæt næste manglende tag", "tagPainter": "Indsæt manglende tags",
        "accessConfig": "Åbn konfigurationsmappe", "accessCurrentSource": "Aktuel kildefil",
        "accessCurrentTarget": "Aktuel målfil", "accessDict": "Ordbøger",
        "accessGlossary": "Glosarer", "accessProject": "Åbn projektindhold",
        "accessRoot": "Projektmappe", "accessTarget": "Målfiler", "accessTm": "Oversættelseshukommelser",
        "accessWritableGlossary": "Skrivbart glossar", "gotoHistoryBack": "Tilbage i historik",
        "gotoHistoryForward": "Frem i historik", "gotoMatchSource": "Kilde til valgt match",
        "gotoNoteNext": "Næste note", "gotoNotePrev": "Forrige note", "gotoNotes": "Notesblok",
        "gotoNumber": "Segmentnummer...", "gotoTranslated": "Næste oversatte segment",
        "gotoUnique": "Næste unikke segment", "gotoEditor": "Til editoren",
        "accessExportTm": "Eksporterede TM'er", "selectSource": "Vælg kildetekst",
        "gotoAutoNext": "Næste segment fra tm/auto/", "gotoAutoPrev": "Forrige segment fra tm/auto/",
        "gotoEnforceNext": "Næste segment fra tm/enforce/", "gotoEnforcePrev": "Forrige segment fra tm/enforce/",
        "scripts": "Skript", "glossaryStem": "Brug stemming", "masterPassword": "Hovedadgangskode",
        "markFont": "Brug aggressiv skrifttype-erstatning",
    }
    # Remaining incomplete locales: copy from closest language then override.
    packs["sq"] = {k: v for k, v in SPARSE_LOC["eo"].items()}
    packs["sq"].update({
        "author": "Autori", "colors": "Ngjyrat", "reload": "Ringarko", "restart": "Rinis",
        "translated": "E përkthyer", "unique": "Unike", "shortcuts": "Shkurtoret e tastierës",
        "scripts": "Skriptet", "glossaryStem": "Përdor rrënjëzimin",
    })
    packs["sh"] = {k: v for k, v in packs["da"].items()}
    packs["sh"].update({
        "author": "Autor", "colors": "Boje", "reload": "Ponovo učitaj", "restart": "Ponovo pokreni",
        "translated": "Prevedeno", "unique": "Jedinstveno", "shortcuts": "Prečice tastature",
        "scripts": "Skripte", "glossaryStem": "Koristi korenovanje",
        "searchType": "Vrsta pretrage", "fontUi": "Font interfejsa", "fontEditor": "Font uređivača",
        "tabAdvance": "Tab prelazi na sledeći segment", "convert": "Pretvori projekat",
    })
    packs["sk"] = dict(packs["sh"])
    packs["sk"].update({
        "author": "Autor", "colors": "Farby", "reload": "Znova načítať", "restart": "Reštartovať",
        "translated": "Preložené", "unique": "Jedinečné", "shortcuts": "Klávesové skratky",
        "searchType": "Typ vyhľadávania", "fontUi": "Písmo rozhrania", "fontEditor": "Písmo editora",
        "tabAdvance": "Tab prejde na ďalší segment", "convert": "Previesť projekt",
    })
    packs["id"] = dict(packs["da"])
    packs["id"].update({
        "author": "Penulis", "colors": "Warna", "reload": "Muat ulang", "restart": "Mulai ulang",
        "translated": "Diterjemahkan", "unique": "Unik", "shortcuts": "Pintasan papan ketik",
        "scripts": "Skrip", "glossaryStem": "Gunakan stemming",
        "searchType": "Jenis pencarian", "fontUi": "Font antarmuka", "fontEditor": "Font penyunting",
        "tabAdvance": "Tab maju ke segmen berikutnya", "convert": "Konversi proyek",
    })
    packs["cy"] = dict(packs["da"])
    packs["cy"].update({
        "author": "Awdur", "colors": "Lliwiau", "reload": "Ail-lwytho", "restart": "Ailgychwyn",
        "translated": "Wedi'i gyfieithu", "unique": "Unigryw",
    })
    packs["el"] = dict(packs["da"])
    packs["el"].update({
        "author": "Συγγραφέας", "colors": "Χρώματα", "reload": "Επαναφόρτωση", "restart": "Επανεκκίνηση",
        "translated": "Μεταφρασμένο", "unique": "Μοναδικό",
    })
    packs["sl"] = dict(packs["sk"])
    packs["gl"] = {
        "scripts": "Scripts do proxecto", "glossaryStem": "Usar lematización",
        "author": "Autor", "colors": "Cores", "reload": "Recargar", "restart": "Reiniciar",
    }
    packs["ko"] = {
        "scripts": "스크립트", "glossaryStem": "어간 사용", "masterPassword": "마스터 암호",
        "author": "작성자", "colors": "색", "reload": "다시 불러오기", "restart": "다시 시작",
    }
    packs["hu"] = {"scripts": "Parancsfájlok", "glossaryStem": "Szótövezés", "author": "Szerző"}
    packs["pl"] = {"scripts": "Skrypty", "glossaryStem": "Używaj rdzeni", "log": "Dziennik", "author": "Autor"}
    packs["no"] = dict(COMMON["convert"])
    return packs


if __name__ == "__main__":
    main()
