#!/usr/bin/env python3
"""Write a short user manual per UI locale from the catalog strings."""
from __future__ import annotations

import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
I18N = ROOT / "apps/desktop/src/renderer/i18n"
OUT = ROOT / "docs/manual"

# Keep the two long-form manuals; generate the rest from catalogs.
SKIP = {"en", "zh-CN"}


def main() -> None:
    en = json.loads((I18N / "en.json").read_text(encoding="utf-8"))
    OUT.mkdir(parents=True, exist_ok=True)
    for p in sorted(I18N.glob("*.json")):
        loc = p.stem
        if loc in SKIP:
            continue
        c = json.loads(p.read_text(encoding="utf-8"))

        def t(key: str) -> str:
            return c.get(key) or en.get(key) or key

        text = f"""# {t("app")} — {t("manual")}

{t("welcomeLead")}

## {t("install") if "install" in c else "Install"}

- Linux: deb / rpm / tar.gz ({t("app")})
- Windows: NSIS
- macOS: DMG

{t("app")} sidecar: `omegat-sidecar`. UI locale: `{loc}`.

## {t("newProject")} / {t("openProject")}

1. {t("openProject")} — `omegat.project`
2. {t("newProject")} — {t("sourceLang")}, {t("targetLang")}, {t("sentenceSeg")}

`source/` · `target/` · `omegat/project_save.tmx` · `tm/` · `glossary/` · `dictionary/`

## {t("editor")}

- {t("save")} (`Ctrl+S`)
- {t("compile")} (`Ctrl+D`)
- {t("search")} / {t("replace")}
- {t("nextSeg")} / {t("prevSeg")}
- {t("completer")}: {t("glossary")}, {t("autotext")}, {t("charset")}, {t("historyCompletion")}, {t("historyPrediction")}

{t("files")}, {t("matches")}, {t("glossary")}, {t("dict")}, {t("mt")}, {t("notes")}, {t("comments")}, {t("multiple")}, {t("properties")}

## {t("prefs")}

{t("general")}, {t("appearance")}, {t("fonts")}, {t("colors")}, {t("editing")}, {t("view")}, {t("filters")}, {t("segmentation")}, {t("shortcuts")}, {t("spell")}, {t("languagetool")}, {t("finder")}, {t("team")}

## {t("aligner")}

HEAPWISE / PARSEWISE / ID · Viterbi ≠ Forward-Backward · {t("create")} TMX

## {t("team")}

git / svn / http / file · {t("keepOurs")} / {t("keepTheirs")}

## {t("wiki")} / {t("med")} / {t("scripts")}

{t("wiki")} · {t("med")} · {t("scripts")} · `{t("run")}`

Java HTML: `docs/manual/java-html.md`.
"""
        (OUT / f"{loc}.md").write_text(text, encoding="utf-8")
    print("wrote locale manuals", OUT)


if __name__ == "__main__":
    main()
