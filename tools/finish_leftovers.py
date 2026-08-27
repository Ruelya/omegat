#!/usr/bin/env python3
"""Decode literal \\uXXXX in locale JSON and fill remaining English leftovers."""
from __future__ import annotations

import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
I18N = ROOT / "apps/desktop/src/renderer/i18n"

U_ESC = re.compile(r"\\u([0-9A-Fa-f]{4})")


def decode_u(s: str) -> str:
    if "\\u" not in s:
        return s
    return U_ESC.sub(lambda m: chr(int(m.group(1), 16)), s)


# Remaining leftover keys after fill_locale_leftovers.py (not OmegaT).
REST: dict[str, dict[str, str]] = {
    "ar": {
        "completer": "الإكمال التلقائي",
    },
    "ca": {
        "general": "Generalitats",
        "colors": "Colors de la interfície",
        "segments": "Nombre de segments",
        "gotoEditor": "A l'editor",
    },
    "cs": {
        "accessExportTm": "Exportované TM",
        "selectSource": "Vybrat zdrojový text",
        "gotoAutoNext": "Další segment z tm/auto/",
        "gotoAutoPrev": "Předchozí segment z tm/auto/",
        "gotoEnforceNext": "Další segment z tm/enforce/",
        "gotoEnforcePrev": "Předchozí segment z tm/enforce/",
        "restart": "Restartovat",
    },
    "cy": {
        "menuProject": "Prosiect",
    },
    "es": {
        "gotoEditor": "Al editor",
        "restart": "Reiniciar",
    },
    "eu": {
        "accessExportTm": "Esportatutako TM-ak",
        "selectSource": "Hautatu jatorrizko testua",
        "gotoAutoNext": "Hurrengo segmentua tm/auto/-tik",
        "gotoAutoPrev": "Aurreko segmentua tm/auto/-tik",
        "gotoEnforceNext": "Hurrengo segmentua tm/enforce/-tik",
        "gotoEnforcePrev": "Aurreko segmentua tm/enforce/-tik",
        "restart": "Berrabiarazi",
    },
    "fi": {
        "accessExportTm": "Viedyt TM:t",
        "selectSource": "Valitse lähdeteksti",
        "gotoAutoNext": "Seuraava segmentti kohteesta tm/auto/",
        "gotoAutoPrev": "Edellinen segmentti kohteesta tm/auto/",
        "gotoEnforceNext": "Seuraava segmentti kohteesta tm/enforce/",
        "gotoEnforcePrev": "Edellinen segmentti kohteesta tm/enforce/",
        "restart": "Käynnistä uudelleen",
    },
    "gl": {
        "log": "Rexistro",
        "masterPassword": "Contrasinal mestre",
        "commitSource": "Enviar ficheiros de orixe",
        "commitTarget": "Enviar ficheiros de destino",
        "accessProject": "Abrir o contido do proxecto",
        "accessRoot": "Cartafol do proxecto",
        "accessTarget": "Ficheiros de destino",
        "accessTm": "Memorias de tradución",
        "accessExportTm": "TM exportadas",
        "accessCurrentSource": "Ficheiro de orixe actual",
        "accessCurrentTarget": "Ficheiro de destino actual",
        "accessWritableGlossary": "Glosario escribible",
        "selectSource": "Seleccionar o texto de orixe",
        "searchDict": "Buscar nos dicionarios",
        "caseSentence": "Maiúscula de frase",
        "insertBidi": "Inserir carácter de control bidi",
        "gotoUnique": "Seguinte segmento único",
        "gotoMatchSource": "Orixe da coincidencia seleccionada",
        "gotoAutoNext": "Seguinte segmento de tm/auto/",
        "gotoAutoPrev": "Segmento anterior de tm/auto/",
        "gotoEnforceNext": "Seguinte segmento de tm/enforce/",
        "gotoEnforcePrev": "Segmento anterior de tm/enforce/",
        "markParagraph": "Amosar delimitacións de parágrafo",
        "markFont": "Usar reserva de fonte agresiva",
        "issuesFile": "Comprobar incidencias do ficheiro actual",
        "checkUpdates": "Buscar actualizacións...",
        "accessConfig": "Abrir o cartafol de configuración",
        "clearRecent": "Borrar proxectos recentes",
    },
    "hr": {
        "selectSource": "Odaberi izvorni tekst",
        "gotoAutoNext": "Sljedeći segment iz tm/auto/",
        "gotoAutoPrev": "Prethodni segment iz tm/auto/",
        "gotoEnforceNext": "Sljedeći segment iz tm/enforce/",
        "gotoEnforcePrev": "Prethodni segment iz tm/enforce/",
        "restart": "Ponovno pokreni",
    },
    "hu": {
        "masterPassword": "Mesterjelszó",
        "commitSource": "Forrásfájlok feltöltése",
        "commitTarget": "Célfájlok feltöltése",
        "accessExportTm": "Exportált TM-ek",
        "selectSource": "Forrásszöveg kijelölése",
        "searchDict": "Keresés a szótárakban",
        "insertBidi": "Bidi vezérlőkarakter beszúrása",
        "gotoAutoNext": "Következő szegmens a tm/auto/ mappából",
        "gotoAutoPrev": "Előző szegmens a tm/auto/ mappából",
        "gotoEnforceNext": "Következő szegmens a tm/enforce/ mappából",
        "gotoEnforcePrev": "Előző szegmens a tm/enforce/ mappából",
        "markParagraph": "Bekezdéshatárok megjelenítése",
        "issuesFile": "Aktuális fájl problémáinak ellenőrzése",
        "checkUpdates": "Frissítések keresése...",
        "clearRecent": "Legutóbbi projektek törlése",
        "restart": "Újraindítás",
    },
    "ia": {
        "accessTm": "Memorias de traduction",
        "accessExportTm": "TMs exportate",
        "selectSource": "Seliger le texto fonte",
        "searchDict": "Cercar in dictionarios",
        "gotoAutoNext": "Proxime segmento de tm/auto/",
        "gotoAutoPrev": "Previe segmento de tm/auto/",
        "gotoEnforceNext": "Proxime segmento de tm/enforce/",
        "gotoEnforcePrev": "Previe segmento de tm/enforce/",
        "gotoEditor": "Al redactor",
        "markParagraph": "Monstrar delimitationes de paragrapho",
        "restart": "Reinitiar",
    },
    "id": {
        "menuEdit": "Sunting",
    },
    "ko": {
        "commitSource": "원문 파일 커밋",
        "commitTarget": "번역문 파일 커밋",
        "accessProject": "프로젝트 내용 열기",
        "accessRoot": "프로젝트 폴더",
        "accessTarget": "번역문 파일",
        "accessTm": "번역 메모리",
        "accessExportTm": "내보낸 TM",
        "accessCurrentSource": "현재 원문 파일",
        "accessCurrentTarget": "현재 번역문 파일",
        "accessWritableGlossary": "쓰기 가능한 용어집",
        "selectSource": "원문 선택",
        "replaceInProject": "바꾸기...",
        "searchDict": "사전에서 검색",
        "caseSentence": "문장 첫 글자 대문자",
        "insertBidi": "양방향 제어 문자 삽입",
        "registerIdentical": "동일한 번역 등록",
        "gotoUnique": "다음 고유 세그먼트",
        "gotoMatchSource": "선택한 일치의 원문",
        "gotoAutoNext": "tm/auto/의 다음 세그먼트",
        "gotoAutoPrev": "tm/auto/의 이전 세그먼트",
        "gotoEnforceNext": "tm/enforce/의 다음 세그먼트",
        "gotoEnforcePrev": "tm/enforce/의 이전 세그먼트",
        "markParagraph": "단락 경계 표시",
        "markAuto": "자동 채워진 세그먼트 강조",
        "markFont": "적극적 글꼴 대체 사용",
        "issuesFile": "현재 파일의 문제 확인",
        "checkUpdates": "업데이트 확인...",
        "accessConfig": "설정 폴더 열기",
        "clearRecent": "최근 프로젝트 지우기",
    },
    "mfe": {
        "menuEdit": "Modifie",
        "accessExportTm": "TM ki finn eksporte",
        "selectSource": "Seleksionn text sour",
        "gotoAutoNext": "Segment swivan depi tm/auto/",
        "gotoAutoPrev": "Segment avan depi tm/auto/",
        "gotoEnforceNext": "Segment swivan depi tm/enforce/",
        "gotoEnforcePrev": "Segment avan depi tm/enforce/",
        "restart": "Redemare",
    },
    "no": {
        "commitSource": "Send inn kildefiler",
        "commitTarget": "Send inn målfiler",
        "accessExportTm": "Eksporterte TM-er",
        "selectSource": "Merk kildetekst",
        "searchDict": "Søk i ordbøker",
        "gotoAutoNext": "Neste segment fra tm/auto/",
        "gotoAutoPrev": "Forrige segment fra tm/auto/",
        "gotoEnforceNext": "Neste segment fra tm/enforce/",
        "gotoEnforcePrev": "Forrige segment fra tm/enforce/",
        "markParagraph": "Vis avsnittsgrenser",
        "checkUpdates": "Se etter oppdateringer...",
        "clearRecent": "Tøm nylige prosjekter",
        "restart": "Start på nytt",
    },
    "pl": {
        "masterPassword": "Hasło główne",
        "commitSource": "Zatwierdź pliki źródłowe",
        "commitTarget": "Zatwierdź pliki docelowe",
        "accessExportTm": "Wyeksportowane TM-y",
        "selectSource": "Zaznacz tekst źródłowy",
        "searchDict": "Szukaj w słownikach",
        "caseCycle": "Przełącz wielkość liter",
        "gotoAutoNext": "Następny segment z tm/auto/",
        "gotoAutoPrev": "Poprzedni segment z tm/auto/",
        "gotoEnforceNext": "Następny segment z tm/enforce/",
        "gotoEnforcePrev": "Poprzedni segment z tm/enforce/",
        "markParagraph": "Pokaż granice akapitów",
        "issuesFile": "Sprawdź problemy bieżącego pliku",
        "checkUpdates": "Sprawdź aktualizacje...",
        "accessConfig": "Otwórz folder konfiguracji",
        "clearRecent": "Wyczyść ostatnie projekty",
        "restart": "Uruchom ponownie",
    },
    "pt": {
        "gotoEditor": "Para o editor",
    },
    "sc": {
        "accessExportTm": "TM esportadas",
        "selectSource": "Seletziona su testu de orìgine",
        "searchDict": "Chirca in sos ditzionàrios",
        "gotoAutoNext": "Segmentu imbeniente de tm/auto/",
        "gotoAutoPrev": "Segmentu pretzedente de tm/auto/",
        "gotoEnforceNext": "Segmentu imbeniente de tm/enforce/",
        "gotoEnforcePrev": "Segmentu pretzedente de tm/enforce/",
        "markParagraph": "Mustra delimitzatziones de paràgrafu",
        "restart": "Torra a aviare",
    },
    "sq": {
        "masterPassword": "Fjalëkalimi kryesor",
        "selectSource": "Zgjidh tekstin burimor",
        "gotoAutoNext": "Segmenti tjetër nga tm/auto/",
        "gotoAutoPrev": "Segmenti i mëparshëm nga tm/auto/",
        "gotoEnforceNext": "Segmenti tjetër nga tm/enforce/",
        "gotoEnforcePrev": "Segmenti i mëparshëm nga tm/enforce/",
    },
    "sv": {
        "accessExportTm": "Exporterade TM:er",
        "selectSource": "Markera källtext",
        "searchDict": "Sök i ordböcker",
        "gotoAutoNext": "Nästa segment från tm/auto/",
        "gotoAutoPrev": "Föregående segment från tm/auto/",
        "gotoEnforceNext": "Nästa segment från tm/enforce/",
        "gotoEnforcePrev": "Föregående segment från tm/enforce/",
        "markParagraph": "Visa styckegränser",
        "restart": "Starta om",
    },
    "tk": {
        "accessExportTm": "Eksport edilen TM-ler",
        "selectSource": "Çeşme teksti saýla",
        "gotoAutoNext": "tm/auto/ içinden indiki segment",
        "gotoAutoPrev": "tm/auto/ içinden öňki segment",
        "gotoEnforceNext": "tm/enforce/ içinden indiki segment",
        "gotoEnforcePrev": "tm/enforce/ içinden öňki segment",
        "restart": "Täzeden başlat",
    },
    "tr": {
        "accessExportTm": "Dışa aktarılan TM'ler",
        "selectSource": "Kaynak metni seç",
        "gotoAutoNext": "tm/auto/ klasöründen sonraki dilim",
        "gotoAutoPrev": "tm/auto/ klasöründen önceki dilim",
        "gotoEnforceNext": "tm/enforce/ klasöründen sonraki dilim",
        "gotoEnforcePrev": "tm/enforce/ klasöründen önceki dilim",
        "restart": "Yeniden başlat",
    },
    "zh-CN": {
        "languagetool": "语言检查工具",
        "multiple": "高亮有备选译文的片段",
    },
    "zh-TW": {
        "languagetool": "語言檢查工具",
    },
    "pt-BR": {
        "languagetool": "Ferramenta linguística",
    },
    "ja": {
        "languagetool": "言語検査",
    },
}


def main() -> None:
    en = json.loads((I18N / "en.json").read_text(encoding="utf-8"))
    leftover = 0
    for p in sorted(I18N.glob("*.json")):
        data = json.loads(p.read_text(encoding="utf-8"))
        loc = p.stem
        for k, v in list(data.items()):
            if isinstance(v, str):
                data[k] = decode_u(v)
        for k, val in REST.get(loc, {}).items():
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
            data["completer"] = "الإكمال التلقائي"
        # Drop leftover (loc) suffix hacks
        for k, v in list(data.items()):
            if isinstance(v, str) and (v.endswith(f" ({loc})") or v.endswith(f"({loc})")):
                stem = v.rsplit(" (", 1)[0]
                if stem == en.get(k) or stem == "LanguageTool":
                    native = REST.get(loc, {}).get(k)
                    if native:
                        data[k] = native
        p.write_text(json.dumps(data, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        if p.name == "en.json":
            continue
        same = [k for k in en if data.get(k) == en[k] and en[k] != "OmegaT"]
        leftover += len(same)
        print(f"{loc}: leftover_eq_en={len(same)} {same[:6]}")
    print(f"total leftover={leftover}")


if __name__ == "__main__":
    main()
