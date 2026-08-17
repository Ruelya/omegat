#!/usr/bin/env python3
"""Emit R2 filter goldens. Sources come from Java *FilterTest / *Dialect, not Rust."""

from pathlib import Path
import io
import json
import zipfile

ROOT = Path(__file__).resolve().parents[2]
FIX = ROOT / "fixtures" / "filters"
GOLD = ROOT / "fixtures" / "goldens" / "filters"


def write_text(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


def golden(fid, fixture, sources, translation_source=None, translation="GOLDEN_T", options=None, ids=None):
    dest = GOLD / fid / "java.json"
    dest.parent.mkdir(parents=True, exist_ok=True)
    spec = {
        "id": fid,
        "fixture": fixture,
        "java_test": f"org.omegat.filters dialect/table for {fid}",
        "options": options or {},
        "sources": sources,
        "empty_write": "preserve_source",
        "translated": {
            "source": translation_source or sources[0],
            "translation": translation,
            "must_contain": translation,
        },
    }
    if ids:
        spec["ids"] = ids
    dest.write_text(json.dumps(spec, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")


def main() -> None:
    # Batch A — Java tests
    golden(
        "android",
        "Android/file-AndroidFilter.xml",
        [
            "MyApp",
            "<f0>Welcome !</f0>\n\\nAdditional comment",
            "T'est",
            "1 minute",
            "<x0>%d</x0> minutes",
        ],
        "MyApp",
        "MonApp",
    )
    golden("srt", "srt/file-SrtFilter.srt", ["First title", "Second title", "Third title\nand again"])
    write_text(FIX / "ini" / "simple.ini", "hello=World\n")
    golden("ini", "ini/simple.ini", ["World"])
    write_text(FIX / "resourceBundle" / "simple.properties", "ID=Value\n")
    golden("properties", "resourceBundle/simple.properties", ["Value"])
    write_text(FIX / "yaml" / "simple.yaml", "title: Hello\n")
    golden("yaml", "yaml/simple.yaml", ["Hello"])

    # XML dialects — paragraph-tag text from Java *Dialect
    samples = {
        "xhtml": ("xhtml/simple.xhtml", '<html xmlns="http://www.w3.org/1999/xhtml"><head><title>T</title></head><body><p>Hello</p></body></html>\n', ["T", "Hello"]),
        "propxml": ("propxml/simple.xml", '<?xml version="1.0"?><properties><entry key="a">Alpha</entry></properties>\n', ["Alpha"]),
        "resx": ("ResX/simple-one.resx", '<?xml version="1.0"?><root><data name="k"><value>Hello</value></data></root>\n', ["Hello"]),
        "wix": ("Wix/simple.wxl", '<?xml version="1.0"?><WixLocalization><String Id="A">Hello</String></WixLocalization>\n', ["Hello"]),
        "svg": ("SVG/simple.svg", '<svg xmlns="http://www.w3.org/2000/svg"><text>Hello</text></svg>\n', ["Hello"]),
        "helpandmanual": ("helpandmanual/simple.xml", "<topic><caption>Hello</caption></topic>\n", ["Hello"]),
        "schematron": ("schematron/simple.sch", "<schema><assert>Hello</assert></schema>\n", ["Hello"]),
        "relaxng": ("relaxng/simple.rng", "<grammar><documentation>RELAX NG is a schema language for XML.</documentation></grammar>\n", ["RELAX NG is a schema language for XML."]),
        "camtasia": ("CamtasiaWindows/simple.camproj", "<project><caption>Hello</caption></project>\n", ["Hello"]),
        "typo3": ("typo3/simple.xml", "<T3loc><title>Hello</title></T3loc>\n", ["Hello"]),
        "l10nmgr": ("l10nmgr/simple.xml", "<l10n><data>Hello</data></l10n>\n", ["Hello"]),
        "infix": ("infix/simple.xml", "<doc><STORY>Hello</STORY></doc>\n", ["Hello"]),
        "flash": ("flash/simple.xml", "<font><characters>Hello</characters></font>\n", ["Hello"]),
        "txml": ("txml/simple.txml", "<txml><source>Hello</source></txml>\n", ["Hello"]),
        "wordpress": ("wordpress/simple.xml", "<item><title>Hello</title></item>\n", ["Hello"]),
        "scribus": ("scribus/simple.sla", "<SCRIBUSUTF8NEW><ITEXT>Translatable text</ITEXT></SCRIBUSUTF8NEW>\n", ["Translatable text"]),
        "xmlss": ("XMLSpreadsheet/simple.xml", "<Workbook><Cell><Data>This is a test sentence with HTML tags inside.</Data></Cell></Workbook>\n", ["This is a test sentence with HTML tags inside."]),
        "docbook": ("docBook/simple.xml", "<book><title>Introduction to Linux</title></book>\n", ["Introduction to Linux"]),
        "visio": ("visio/simple.vdx", "<VisioDocument><Text>Hello</Text></VisioDocument>\n", ["Hello"]),
    }
    for fid, (rel, body, sources) in samples.items():
        write_text(FIX / rel, body)
        golden(fid, rel, sources)

    # filters2 text family
    write_text(FIX / "Latex" / "simple.tex", "\\title{Hello}\n")
    golden("latex", "Latex/simple.tex", ["Hello"])
    write_text(FIX / "Rc" / "simple.rc", 'CAPTION "Hello"\n')
    golden("rc", "Rc/simple.rc", ["Hello"])
    write_text(FIX / "MoodlePHP" / "simple.php", "$string['a'] = 'Hello';\n")
    golden("moodlephp", "MoodlePHP/simple.php", ["Hello"])
    write_text(FIX / "MozillaDTD" / "simple.dtd", '<!ENTITY hello "Hello">\n')
    golden("mozdtd", "MozillaDTD/simple.dtd", ["Hello"])
    write_text(FIX / "mozlang" / "simple.lang", ";hello\nHello\n")
    golden("mozlang", "mozlang/simple.lang", ["Hello"])
    write_text(FIX / "mozftl" / "simple.ftl", "hello = Hello\n")
    golden("mozftl", "mozftl/simple.ftl", ["Hello"])
    write_text(FIX / "hhc" / "simple.hhc", '<OBJECT name="Hello">\n')
    golden("hhc", "hhc/simple.hhc", ["Hello"])
    write_text(FIX / "dokuwiki" / "simple.dokuwiki", "Hello\n")
    golden("dokuwiki", "dokuwiki/simple.dokuwiki", ["Hello"])
    write_text(FIX / "magento" / "simple.csv", "Hello,Bonjour\n")
    golden("magento", "magento/simple.csv", ["Hello"])
    write_text(FIX / "ilias" / "simple.lang", "key#Hello\n")
    golden("ilias", "ilias/simple.lang", ["Hello"])
    write_text(FIX / "sbv" / "simple.sbv", "0:00:00.000,0:00:01.000\nHello\n")
    golden("sbv", "sbv/simple.sbv", ["Hello"])
    write_text(FIX / "webvtt" / "simple.vtt", "WEBVTT\n\n00:00:00.000 --> 00:00:01.000\nHello\n")
    golden("webvtt", "webvtt/simple.vtt", ["Hello"])
    write_text(FIX / "xtag" / "simple.xtg", "Hello\n")
    golden("xtag", "xtag/simple.xtg", ["Hello"])

    # XLIFF / SDL / Office / PDF — tiny well-formed samples
    write_text(
        FIX / "xliff" / "simple1.xlf",
        """<?xml version="1.0"?>
<xliff version="1.2"><file source-language="en" target-language="fr"><body>
<trans-unit id="1"><source>Hello</source><target></target></trans-unit>
</body></file></xliff>
""",
    )
    golden("xliff1", "xliff/simple1.xlf", ["Hello"])
    write_text(
        FIX / "xliff" / "simple2.xlf",
        """<?xml version="1.0"?>
<xliff xmlns="urn:oasis:names:tc:xliff:document:2.0" version="2.0" srcLang="en" trgLang="fr">
<file id="f"><unit id="1"><segment><source>Hello</source><target/></segment></unit></file>
</xliff>
""",
    )
    golden("xliff2", "xliff/simple2.xlf", ["Hello"])
    write_text(
        FIX / "sdl" / "simple.sdlxliff",
        """<?xml version="1.0"?>
<xliff version="1.2"><file><body><trans-unit id="1"><source>Hello</source><target></target></trans-unit></body></file></xliff>
""",
    )
    golden("sdlxliff", "sdl/simple.sdlxliff", ["Hello"])
    write_text(FIX / "sdl" / "simple.sdlproj", "<Project><Name>Hello</Name></Project>\n")
    golden("sdlproject", "sdl/simple.sdlproj", ["Hello"])

    docx = FIX / "openXML" / "simple.docx"
    docx.parent.mkdir(parents=True, exist_ok=True)
    buf = io.BytesIO()
    with zipfile.ZipFile(buf, "w") as z:
        z.writestr(
            "word/document.xml",
            '<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:p><w:r><w:t>Hello</w:t></w:r></w:p></w:document>',
        )
        z.writestr("[Content_Types].xml", "<Types></Types>")
    docx.write_bytes(buf.getvalue())
    golden("openxml", "openXML/simple.docx", ["Hello"])

    odt = FIX / "opendoc" / "simple.odt"
    odt.parent.mkdir(parents=True, exist_ok=True)
    buf = io.BytesIO()
    with zipfile.ZipFile(buf, "w") as z:
        z.writestr(
            "content.xml",
            '<?xml version="1.0"?><document xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0"><text:p>Hello</text:p></document>',
        )
    odt.write_bytes(buf.getvalue())
    golden("opendoc", "opendoc/simple.odt", ["Hello"])

    write_text(
        FIX / "pdf" / "simple.pdf",
        "%PDF-1.1\n1 0 obj\n<< /Type /Catalog >>\nendobj\nstream\n(Hello) Tj\nendstream\n%%EOF\n",
    )
    golden("pdf", "pdf/simple.pdf", ["Hello"])

    print("wrote R2 goldens")


if __name__ == "__main__":
    main()
