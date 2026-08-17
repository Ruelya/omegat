#!/usr/bin/env python3
"""Structural honesty gates for the OmegaT 6.2 rewrite.

P0 commits this red on the current tree. Later waves may turn only their
own items green. Do not treat a STATUS row as parity while the matching
item here is red.
"""

from __future__ import annotations

import json
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
FAILS: list[str] = []
PASSES: list[str] = []


def note(ok: bool, msg: str) -> None:
    (PASSES if ok else FAILS).append(msg)
    print(("OK  " if ok else "FAIL") + " " + msg)


def rg(pattern: str, *paths: str, glob: str | None = None) -> list[str]:
    cmd = ["rg", "-n", "--no-heading", pattern]
    if glob:
        cmd.extend(["--glob", glob])
    cmd.extend(paths)
    proc = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True)
    if proc.returncode not in (0, 1):
        return [f"rg error: {proc.stderr.strip()}"]
    return [ln for ln in proc.stdout.splitlines() if ln.strip()]


def read_text(rel: str) -> str:
    return (ROOT / rel).read_text(encoding="utf-8")


def check_identity_stems() -> None:
    hits = rg(
        r"stems::identity",
        "crates/omegat-core/src/tokenize",
        glob="lucene_*.rs",
    )
    note(not hits, "no stems::identity in lucene_*.rs" + ("" if not hits else f" ({len(hits)} hits)"))


def check_forbidden_product_tokens() -> None:
    hits = rg(
        r"fn fallback_eval|contentEditable|translate_mock|dialect_filter!|simple_filter!",
        "crates",
        "apps/desktop/src",
        glob="*.{rs,ts,tsx,js}",
    )
    # comments / tests that only *forbid* the token are still a mention; allow
    # those that are clearly negative assertions or historical notes.
    real = []
    for ln in hits:
        low = ln.lower()
        if "does not use" in low or "is gone" in low or "not.tomatch" in low or "not to match" in low:
            continue
        if "/tests/" in ln or ".test.ts" in ln or ln.endswith("_test.rs"):
            continue
        real.append(ln)
    note(not real, "no fallback_eval/contentEditable/translate_mock/macros in product" + ("" if not real else f"\n    " + "\n    ".join(real[:12])))


def check_placeholders() -> None:
    hits = rg(r'className=["\']placeholder["\']', "apps/desktop/src")
    note(not hits, "no className=placeholder in desktop" + ("" if not hits else f" ({len(hits)} hits)"))


def check_git_command() -> None:
    hits = rg(r'Command::new\("git"\)', "crates/omegat-team")
    # Product path may hide the binary behind run_git / Command::new(bin).
    hidden = rg(r'fn run_git|Command::new\(bin\)', "crates/omegat-team/src/team_utils.rs")
    product = []
    for ln in hits:
        path = ln.split(":", 1)[0]
        if path.endswith("lib.rs") or "/tests/" in path:
            continue
        product.append(ln)
    if hidden:
        product.extend(hidden)
    note(not product, "no product-path git Command in omegat-team (need git2)" + ("" if not product else f"\n    " + "\n    ".join(product[:8])))


def check_html_visitor() -> None:
    html = ROOT / "crates/omegat-filters/src/html.rs"
    visitor = ROOT / "crates/omegat-filters/src/html/filter_visitor.rs"
    visitor_alt = ROOT / "crates/omegat-filters/src/html/filter_visitor.rs"
    has_visitor = visitor.is_file() or visitor_alt.is_file() or (
        ROOT / "crates/omegat-filters/src/html/mod.rs"
    ).is_file() and (ROOT / "crates/omegat-filters/src/html/filter_visitor.rs").is_file()
    regex_main = False
    if html.is_file():
        text = html.read_text(encoding="utf-8")
        if re.search(r"h\[1-6\]|block.*=.*Regex", text) and "fn parse" in text:
            regex_main = True
    html_tests = list((ROOT / "fixtures/goldens/filters/html").glob("*.json")) if (ROOT / "fixtures/goldens/filters/html").is_dir() else []
    required = {
        "testParse",
        "testIgnoreCommentParse",
        "testParseAllBlockElements",
        "testParseRegression",
        "testTranslate",
        "testLoad",
        "testTagsOptimization",
        "testHtmlEntityDecode",
        "testLayout",
        "testLayoutTrimWhitespace",
        "testLayoutPreserveWhitespace",
        "testAddCharsetHeaderWhenNoHeader",
        "testAddCharsetHeaderWhenExistingHeader",
        "testAddCharsetHeaderWhenExistingMeta",
        "testAddCharsetHeaderHtml5WhenExistingMeta",
    }
    found = set()
    for p in html_tests:
        try:
            data = json.loads(p.read_text(encoding="utf-8"))
        except json.JSONDecodeError:
            continue
        jt = str(data.get("java_test") or "")
        if "#" in jt:
            found.add(jt.split("#", 1)[1])
    missing = sorted(required - found)
    ok = has_visitor and not regex_main and not missing
    detail = []
    if not has_visitor:
        detail.append("missing crates/omegat-filters/src/html/filter_visitor.rs")
    if regex_main:
        detail.append("html.rs still uses a block-tag regex as parse")
    if missing:
        detail.append("HTMLFilter2Test goldens missing: " + ", ".join(missing))
    note(ok, "HTML FilterVisitor path + HTMLFilter2Test goldens" + ("" if ok else " (" + "; ".join(detail) + ")"))


def check_dialect_tags() -> None:
    path = ROOT / "fixtures/goldens/engine/dialect_tags.json"
    if not path.is_file():
        note(False, "dialect_tags.json missing (Java export format not present)")
        return
    data = json.loads(path.read_text(encoding="utf-8"))
    dialects = data.get("dialects") or data.get("cases") or []
    if isinstance(dialects, dict):
        dialects = [{"id": k, **v} for k, v in dialects.items()]
    if not dialects:
        note(False, "dialect_tags.json has no dialects")
        return
    rust_dir = ROOT / "crates/omegat-filters/src/filters3"
    rust_name = {
        "propxml": "properties_dialect.rs",
        "xmlss": "xmlspreadsheet_dialect.rs",
        "xliff": "xliff_dialect.rs",
    }
    missing_eq = []
    for d in dialects:
        did = d.get("id") or d.get("name")
        intact = set(d.get("intact") or d.get("intact_tags") or [])
        if did == "camtasia" and "AudioClickSensitivity" not in intact:
            missing_eq.append("camtasia intact lacks AudioClickSensitivity")
        rust = rust_dir / rust_name.get(did, f"{did}_dialect.rs")
        if not rust.is_file():
            missing_eq.append(f"no {rust.name}")
            continue
        src = rust.read_text(encoding="utf-8")
        for tag in sorted(intact)[:8]:
            if tag and tag not in src:
                missing_eq.append(f"{did}: rust dialect missing intact {tag}")
                break
    note(not missing_eq, "dialect_tags.json assert_eq vs *_dialect.rs" + ("" if not missing_eq else " (" + "; ".join(missing_eq[:8]) + ")"))


def check_tokens() -> None:
    path = ROOT / "fixtures/goldens/engine/tokens.json"
    if not path.is_file():
        note(False, "tokens.json missing")
        return
    data = json.loads(path.read_text(encoding="utf-8"))
    cases = data.get("cases") or []
    by_tok: dict[str, dict[str, list[dict]]] = {}
    hello_only = []
    for c in cases:
        tok = c.get("tokenizer") or ""
        if "Lucene" not in tok:
            continue
        mode = c.get("stemming") or ""
        by_tok.setdefault(tok, {}).setdefault(mode, []).append(c)
        lang = c.get("lang") or ""
        inp = c.get("input") or ""
        englishish = lang in {"en", "en-us", "en-gb"}
        if (not englishish) and inp == "Hello worlds running":
            hello_only.append(f"{tok} {lang} {mode}")
    need = {"NONE", "GLOSSARY", "MATCHING"}
    missing_modes = []
    for tok, modes in sorted(by_tok.items()):
        have = set(modes)
        if not need.issubset(have):
            missing_modes.append(f"{tok}: have {sorted(have)}")
    ok = not hello_only and not missing_modes
    detail = []
    if hello_only:
        detail.append("Latin Hello-worlds cases: " + ", ".join(hello_only[:6]))
    if missing_modes:
        detail.append("missing modes: " + "; ".join(missing_modes[:6]))
    note(ok, "Lucene token goldens NONE+GLOSSARY+MATCHING on language text" + ("" if ok else " (" + " | ".join(detail) + ")"))


def check_filter_tests() -> None:
    inv = ROOT / "fixtures/goldens/engine/filter_tests.json"
    if not inv.is_file():
        note(False, "filter_tests.json missing (Java *FilterTest inventory not exported)")
        return
    data = json.loads(inv.read_text(encoding="utf-8"))
    tests = data.get("tests") or data.get("cases") or []
    missing = []
    for t in tests:
        golden = t.get("golden") or t.get("path")
        if not golden:
            missing.append(t.get("java_test") or "?")
            continue
        p = ROOT / "fixtures/goldens" / golden if not str(golden).startswith("fixtures/") else ROOT / golden
        if not p.is_file():
            missing.append(str(t.get("java_test") or golden))
    note(not missing, "every *FilterTest#test* has a golden" + ("" if not missing else f" ({len(missing)} missing, e.g. {missing[:6]})"))


def check_plugin_dirs() -> None:
    props = ROOT / "reference/java/Plugins.properties"
    if not props.is_file():
        note(False, "Plugins.properties missing")
        return
    # 49 filter plugin ids: lines 1-49 of the plugin= continuation, filters2/3/4 only
    ids = [
        "xliff", "android", "xhtml", "helpandmanual", "propxml", "schematron",
        "relaxng", "camtasia", "docbook", "opendoc", "openxml", "resx", "wix",
        "typo3", "l10nmgr", "svg", "infix", "flash", "txml", "visio", "xmlss",
        "wordpress", "scribus", "text", "latex", "po", "rc", "moodlephp",
        "mozdtd", "mozlang", "properties", "mozftl", "html", "hhc", "ini",
        "dokuwiki", "magento", "ilias", "yaml", "pdf", "srt", "sbv", "webvtt",
        "xtag", "msoffice", "xliff1", "xliff2", "sdlxliff", "sdlproject",
    ]
    gold = ROOT / "fixtures/goldens/filters"
    missing = [i for i in ids if not (gold / i).is_dir()]
    note(not missing and len(ids) == 49, f"49 Java plugin id golden directories exist ({len(ids) - len(missing)}/{len(ids)})" + ("" if not missing else f" missing {missing}"))


def check_ieditor() -> None:
    path = ROOT / "fixtures/goldens/engine/ieditor_methods.json"
    impl = ROOT / "tools/honesty/ieditor_impl.txt"
    if not path.is_file():
        note(False, "ieditor_methods.json missing (IEditor export format not present)")
        return
    data = json.loads(path.read_text(encoding="utf-8"))
    methods = data.get("methods") or []
    implemented: set[str] = set()
    if impl.is_file():
        implemented = {ln.strip() for ln in impl.read_text(encoding="utf-8").splitlines() if ln.strip() and not ln.startswith("#")}
    missing = [m for m in methods if m not in implemented]
    note(not missing, "IEditor method set equals implementation table" + ("" if not missing else f" (gap {len(missing)}: {missing[:8]})"))


def check_menus() -> None:
    path = ROOT / "fixtures/goldens/engine/menu_actions.json"
    if not path.is_file():
        note(False, "menu_actions.json missing (MainWindowMenuHandler export format not present)")
        return
    data = json.loads(path.read_text(encoding="utf-8"))
    java = data.get("actions") or data.get("methods") or []
    actions_ts = ROOT / "apps/desktop/src/renderer/menus/actions.ts"
    text = actions_ts.read_text(encoding="utf-8") if actions_ts.is_file() else ""
    listed = re.findall(r'"([a-z0-9.-]+)"', text.split("SCRIPT_SLOT_ACTIONS")[0] if "SCRIPT_SLOT_ACTIONS" in text else text)
    # observable-behavior tests: must not be only "switch has this case"
    test = ROOT / "apps/desktop/src/renderer/menus/actions.test.ts"
    test_src = test.read_text(encoding="utf-8") if test.is_file() else ""
    only_presence = "toHaveLength(120)" in test_src and "observable" not in test_src.lower()
    gap = []
    if len(java) < 120:
        gap.append(f"Java export has {len(java)} ActionPerformed (need 120)")
    if only_presence:
        gap.append("menu tests assert case presence, not observable behavior")
    note(not gap, "120 menu actions wired to observable behavior" + ("" if not gap else " (" + "; ".join(gap) + ")"))
    _ = listed  # reserved for later exact-id mapping


def check_locales() -> None:
    i18n = ROOT / "apps/desktop/src/renderer/i18n"
    en_path = i18n / "en.json"
    if not en_path.is_file():
        note(False, "en.json missing")
        return
    en = json.loads(en_path.read_text(encoding="utf-8"))
    en_keys = set(en)
    key_mismatches = []
    english_tails = []
    bundle_dir = ROOT / "reference/java/src/main/resources/org/omegat"
    for p in sorted(i18n.glob("*.json")):
        if p.name == "en.json":
            continue
        data = json.loads(p.read_text(encoding="utf-8"))
        keys = set(data)
        if keys != en_keys:
            key_mismatches.append(f"{p.name} Δkeys +{len(keys - en_keys)} -{len(en_keys - keys)}")
        loc = p.stem
        bundle = None
        for cand in (bundle_dir / f"Bundle_{loc.replace('-', '_')}.properties",):
            if cand.is_file():
                bundle = cand
                break
        if not bundle:
            continue
        same = sum(1 for k, v in data.items() if isinstance(v, str) and v == en.get(k))
        if same:
            english_tails.append(f"{loc} {same}/{len(en)}")
    ok = not key_mismatches and not english_tails
    detail = []
    if key_mismatches:
        detail.append("keyset " + ", ".join(key_mismatches[:4]))
    if english_tails:
        detail.append("english tails " + ", ".join(english_tails[:6]))
    note(ok, "locale keysets match en.json and Bundle translations are not English leftovers" + ("" if ok else " (" + " | ".join(detail) + ")"))


def check_status() -> None:
    path = ROOT / "docs/rewrite/STATUS.md"
    text = path.read_text(encoding="utf-8")
    rows = []
    for ln in text.splitlines():
        if not ln.startswith("|"):
            continue
        cells = [c.strip() for c in ln.strip("|").split("|")]
        if len(cells) < 3:
            continue
        if cells[0] in {"Area", "---"} or set(cells[2]) <= {"-"}:
            continue
        if cells[2] in {"parity", "scaffold", "parity_gap"}:
            rows.append((cells[0], cells[1], cells[2]))
    if not rows:
        note(False, "STATUS.md has no status table rows")
        return
    all_parity = all(s == "parity" for _, _, s in rows)
    # P12 may allow multi-row parity only when no scaffold remains and gates are green.
    # Until then a full-table parity is a fail. This script is itself a gate, so
    # all-parity always fails here; P12 will delete this clause when flipping.
    p12_done = (ROOT / "tools/honesty/P12_GATES_GREEN").is_file()
    ok = (not all_parity) or p12_done
    scaffold = sum(1 for *_, s in rows if s == "scaffold")
    note(ok, f"STATUS not full-table parity ({len(rows)} rows, {scaffold} scaffold)" + ("" if ok else " (all rows are parity)"))


def check_provenance() -> None:
    proc = subprocess.run(
        [sys.executable, str(ROOT / "tools/export_java_goldens/check_provenance.py")],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    note(proc.returncode == 0, "golden provenance" + ("" if proc.returncode == 0 else f"\n{proc.stderr or proc.stdout}"))


def main() -> int:
    check_identity_stems()
    check_forbidden_product_tokens()
    check_placeholders()
    check_git_command()
    check_html_visitor()
    check_dialect_tags()
    check_tokens()
    check_filter_tests()
    check_plugin_dirs()
    check_ieditor()
    check_menus()
    check_locales()
    check_status()
    check_provenance()
    print()
    print(f"{len(PASSES)} passed, {len(FAILS)} failed")
    if FAILS:
        print("honesty gates RED (expected until the matching wave is done)")
        return 1
    print("honesty gates GREEN")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
