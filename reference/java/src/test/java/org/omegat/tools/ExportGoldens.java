/**************************************************************************
 OmegaT - Computer Assisted Translation (CAT) tool
          with fuzzy matching, translation memory, keyword search,
          glossaries, and translation leveraging into updated projects.

 This file is part of OmegaT.

 OmegaT is free software: you can redistribute it and/or modify
 it under the terms of the GNU General Public License as published by
 the Free Software Foundation, either version 3 of the License, or
 (at your option) any later version.
 **************************************************************************/

package org.omegat.tools;

import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.TreeMap;

import org.omegat.core.Core;
import org.omegat.core.data.EntryKey;
import org.omegat.core.data.SourceTextEntry;
import org.omegat.core.matching.FuzzyMatcher;
import org.omegat.core.matching.LevenshteinDistance;
import org.omegat.core.segmentation.Rule;
import org.omegat.core.segmentation.SRX;
import org.omegat.core.segmentation.Segmenter;
import org.omegat.core.statistics.Statistics;
import org.omegat.core.statistics.dso.MatchStatCounts;
import org.omegat.core.statistics.dso.StatCount;
import org.omegat.filters2.FilterContext;
import org.omegat.filters2.IFilter;
import org.omegat.filters2.IParseCallback;
import org.omegat.filters2.ITranslateCallback;
import org.omegat.filters2.hhc.HHCFilter2;
import org.omegat.filters2.html2.HTMLFilter2;
import org.omegat.filters2.latex.LatexFilter;
import org.omegat.filters2.master.FilterMaster;
import org.omegat.filters2.moodlephp.MoodlePHPFilter;
import org.omegat.filters2.mozdtd.MozillaDTDFilter;
import org.omegat.filters2.mozlang.MozillaLangFilter;
import org.omegat.filters2.po.PoFilter;
import org.omegat.filters2.rc.RcFilter;
import org.omegat.filters2.text.TextFilter;
import org.omegat.filters2.text.bundles.ResourceBundleFilter;
import org.omegat.filters2.text.dokuwiki.DokuWikiFilter;
import org.omegat.filters2.text.ilias.ILIASFilter;
import org.omegat.filters2.text.ini.INIFilter;
import org.omegat.filters2.text.magento.MagentoFilter;
import org.omegat.filters2.text.mozftl.MozillaFTLFilter;
import org.omegat.filters2.text.yaml.YamlFilter;
import org.omegat.filters2.pdf.PdfFilter;
import org.omegat.filters2.subtitles.SbvFilter;
import org.omegat.filters2.subtitles.SrtFilter;
import org.omegat.filters2.subtitles.WebVttFilter;
import org.omegat.filters2.xtagqxp.XtagFilter;
import org.omegat.filters3.xml.android.AndroidFilter;
import org.omegat.filters3.xml.camtasiawindows.CamtasiaWindowsFilter;
import org.omegat.filters3.xml.docbook.DocBookFilter;
import org.omegat.filters3.xml.flash.FlashFilter;
import org.omegat.filters3.xml.helpandmanual.HelpAndManualFilter;
import org.omegat.filters3.xml.infix.InfixFilter;
import org.omegat.filters3.xml.l10nmgr.L10nmgrFilter;
import org.omegat.filters3.xml.opendoc.OpenDocFilter;
import org.omegat.filters3.xml.openxml.OpenXMLFilter;
import org.omegat.filters3.xml.properties.PropertiesFilter;
import org.omegat.filters3.xml.relaxng.RelaxNGFilter;
import org.omegat.filters3.xml.resx.ResXFilter;
import org.omegat.filters3.xml.schematron.SchematronFilter;
import org.omegat.filters3.xml.scribus.ScribusFilter;
import org.omegat.filters3.xml.svg.SvgFilter;
import org.omegat.filters3.xml.txml.TXMLFilter;
import org.omegat.filters3.xml.typo3.Typo3Filter;
import org.omegat.filters3.xml.visio.VisioFilter;
import org.omegat.filters3.xml.wix.WiXFilter;
import org.omegat.filters3.xml.wordpress.WordpressFilter;
import org.omegat.filters3.xml.xhtml.XHTMLFilter;
import org.omegat.filters3.xml.xliff.XLIFFFilter;
import org.omegat.filters3.xml.xmlspreadsheet.XMLSpreadsheetFilter;
import org.omegat.filters4.xml.openxml.MsOfficeFileFilter;
import org.omegat.filters4.xml.xliff.SdlProject;
import org.omegat.filters4.xml.xliff.SdlXliff;
import org.omegat.filters4.xml.xliff.Xliff1Filter;
import org.omegat.filters4.xml.xliff.Xliff2Filter;
import org.omegat.gui.glossary.GlossaryEntry;
import org.omegat.gui.glossary.GlossaryReaderTSV;
import org.omegat.gui.glossary.GlossarySearcher;
import org.omegat.tokenizer.DefaultTokenizer;
import org.omegat.tokenizer.ITokenizer;
import org.omegat.tokenizer.LuceneCJKTokenizer;
import org.omegat.tokenizer.LuceneEnglishTokenizer;
import org.omegat.util.Language;
import org.omegat.util.Preferences;
import org.omegat.util.TestPreferencesInitializer;
import org.omegat.util.Token;

/**
 * Run real Java 6.2 filters / Segmenter / FuzzyMatcher and write goldens for
 * the Rust rewrite. This is the only accepted exporter.
 */
public final class ExportGoldens {

    public static final String EXPORTED_BY = "org.omegat.tools.ExportGoldens";

    private final Path javaRoot;
    private final Path goldenRoot;
    private final FilterContext context = new FilterContext(new Language("en"), new Language("be"), false)
            .setTargetTokenizerClass(DefaultTokenizer.class);

    private ExportGoldens(Path javaRoot, Path goldenRoot) {
        this.javaRoot = javaRoot;
        this.goldenRoot = goldenRoot;
    }

    public static void main(String[] args) throws Exception {
        Path javaRoot = Path.of(System.getProperty("user.dir")).toAbsolutePath();
        if (!Files.isRegularFile(javaRoot.resolve("src/test/resources/data/filters/text/file-TextFilter.txt"))) {
            throw new IllegalStateException("working directory must be reference/java, got " + javaRoot);
        }
        Path goldenRoot;
        if (args.length > 0) {
            goldenRoot = Path.of(args[0]).toAbsolutePath();
        } else {
            goldenRoot = javaRoot.resolve("../../fixtures/goldens").normalize();
        }
        Files.createDirectories(goldenRoot);
        TestPreferencesInitializer.init();
        Core.initializeConsole();
        Core.setFilterMaster(new FilterMaster(FilterMaster.createDefaultFiltersConfig()));
        new ExportGoldens(javaRoot, goldenRoot).run();
    }

    private void run() throws Exception {
        exportTextEmptyLines();
        exportPoMultiple();
        exportHtml();
        exportIni();
        exportSrt();
        exportYaml();
        exportAndroid();
        exportFilters3();
        exportFilters4();
        exportResourceBundle();
        exportMozillaFtl();
        exportMagento();
        exportDokuWiki();
        exportIlias();
        exportLatex();
        exportRc();
        exportMozillaDtd();
        exportMozillaLang();
        exportMoodlePhp();
        exportHhc();
        exportSbv();
        exportWebVtt();
        exportXtag();
        exportPdf();
        exportEngine();
        exportGlossary();
        exportStats();
        System.out.println("ExportGoldens wrote " + goldenRoot);
    }

    private void exportTextEmptyLines() throws Exception {
        Map<String, String> options = new TreeMap<>();
        options.put(TextFilter.OPTION_SEGMENT_ON, TextFilter.SEGMENT_EMPTYLINES);
        exportFilter("text", "text/file-TextFilter.empty-lines.json",
                "text/file-TextFilter.txt",
                "org.omegat.filters.TextFilterTest#testParseEmptyLinesBreak",
                new TextFilter(), options,
                "This test file for test TextFilter.", "GOLDEN_T");
    }

    private void exportPoMultiple() throws Exception {
        Map<String, String> options = new TreeMap<>();
        options.put(PoFilter.OPTION_SKIP_HEADER, "true");
        exportFilter("po", "po/file-POFilter-multiple.json",
                "po/file-POFilter-multiple.po",
                "org.omegat.filters.POFilterTest#testLoad",
                new PoFilter(), options, "source3", "GOLDEN_T");
    }

    private void exportHtml() throws Exception {
        exportFilter("html", "html/file-HTMLFilter2.json",
                "html/file-HTMLFilter2.html",
                "org.omegat.filters.HTMLFilter2Test#testParse",
                new HTMLFilter2(), Collections.emptyMap(),
                "This is first line.", "Ceci est la premiere ligne.");
    }

    private void exportIni() throws Exception {
        exportFilter("ini", "ini/file-INIFilter.json",
                "ini/file-INIFilter.ini",
                "org.omegat.filters.INIFilterTest#testLoad",
                new INIFilter(), Collections.emptyMap(),
                "Value2", "GOLDEN_T");
    }

    private void exportSrt() throws Exception {
        exportFilter("srt", "srt/file-SrtFilter.json",
                "srt/file-SrtFilter.srt",
                "org.omegat.filters.SrtFilterTest#testParse",
                new SrtFilter(), Collections.emptyMap(),
                "First title", "GOLDEN_T");
    }

    private void exportYaml() throws Exception {
        exportFilter("yaml", "yaml/sample1.json",
                "yaml/sample1.yaml",
                "org.omegat.filters.YamlFilterTest#testParse",
                new YamlFilter(), Collections.emptyMap(),
                null, null);
    }

    private void exportResourceBundle() throws Exception {
        exportFilter("properties", "properties/file-ResourceBundleFilter.json",
                "resourceBundle/file-ResourceBundleFilter.properties",
                "org.omegat.filters.ResourceBundleFilterTest#testParse",
                new ResourceBundleFilter(), Collections.emptyMap(), null, null);
    }

    private void exportMozillaFtl() throws Exception {
        exportFilter("mozftl", "mozftl/MozillaFTLFilter.json",
                "MozillaFTL/MozillaFTLFilter.ftl",
                "org.omegat.filters.MozillaFTLFilterTest#testParse",
                new MozillaFTLFilter(), Collections.emptyMap(), null, null);
    }

    private void exportMagento() throws Exception {
        exportFilter("magento", "magento/MagentoFilter.json",
                "magento/MagentoFilter.csv",
                "org.omegat.filters.MagentoFilterTest#testParse",
                new MagentoFilter(), Collections.emptyMap(), null, null);
    }

    private void exportDokuWiki() throws Exception {
        exportFilter("dokuwiki", "dokuwiki/dokuwiki.json",
                "dokuwiki/dokuwiki.txt",
                "org.omegat.filters.DokuWikiFilterTest#testTextFilterParsing",
                new DokuWikiFilter(), Collections.emptyMap(), null, null);
    }

    private void exportIlias() throws Exception {
        exportFilter("ilias", "ilias/ILIASFilter.json",
                "ilias/ILIASFilter.lang",
                "org.omegat.filters.ILIASFilterTest#testParse",
                new ILIASFilter(), Collections.emptyMap(), null, null);
    }

    private void exportLatex() throws Exception {
        exportFilter("latex", "latex/file-latex-items.json",
                "Latex/file-latex-items.tex",
                "org.omegat.filters.LatexFilterTest#testLoadItemize",
                new LatexFilter(), Collections.emptyMap(), null, null);
    }

    private void exportRc() throws Exception {
        exportFilter("rc", "rc/prog.json",
                "Rc/prog.rc",
                "org.omegat.filters.RcFilterTest#testLoad",
                new RcFilter(), Collections.emptyMap(), null, null);
    }

    private void exportMozillaDtd() throws Exception {
        exportFilter("mozdtd", "mozdtd/file.json",
                "MozillaDTD/file.dtd",
                "org.omegat.filters.MozillaDTDFilterTest#testLoad",
                new MozillaDTDFilter(), Collections.emptyMap(), null, null);
    }

    private void exportMozillaLang() throws Exception {
        exportFilter("mozlang", "mozlang/file-MozillaLangFilter-de.json",
                "MozillaLang/file-MozillaLangFilter-de.lang",
                "org.omegat.filters2.mozlang.MozillaLangFilter#processFile",
                new MozillaLangFilter(), Collections.emptyMap(), null, null);
    }

    private void exportMoodlePhp() throws Exception {
        exportFilter("moodlephp", "moodlephp/file.json",
                "MoodlePHP/file.php",
                "org.omegat.filters.MoodlePHPFilterTest#testParse",
                new MoodlePHPFilter(), Collections.emptyMap(), null, null);
    }

    private void exportHhc() throws Exception {
        exportFilter("hhc", "hhc/file-HHCFilter2.json",
                "hhc/file-HHCFilter2.hhc",
                "org.omegat.filters.HHCFilter2Test#testParse",
                new HHCFilter2(), Collections.emptyMap(), null, null);
    }

    private void exportSbv() throws Exception {
        exportFilter("sbv", "sbv/simple.json",
                "sbv/simple.sbv",
                "org.omegat.filters2.subtitles.SbvFilter#processFile",
                new SbvFilter(), Collections.emptyMap(), null, null);
    }

    private void exportWebVtt() throws Exception {
        exportFilter("webvtt", "webvtt/simple.json",
                "webvtt/simple.vtt",
                "org.omegat.filters2.subtitles.WebVttFilter#processFile",
                new WebVttFilter(), Collections.emptyMap(), null, null);
    }

    private void exportXtag() throws Exception {
        exportFilter("xtag", "xtag/file-XtagFilter.json",
                "xtag/file-XtagFilter.xtg",
                "org.omegat.filters2.xtagqxp.XtagFilter#processFile",
                new XtagFilter(), Collections.emptyMap(), null, null);
    }

    private void exportPdf() throws Exception {
        exportFilter("pdf", "pdf/file-PdfFilter.json",
                "pdf/file-PdfFilter.pdf",
                "org.omegat.filters.PdfFilterTest#testParse",
                new PdfFilter(), Collections.emptyMap(), null, null);
    }

    private void exportAndroid() throws Exception {
        exportFilter("android", "android/file-AndroidFilter.json",
                "Android/file-AndroidFilter.xml",
                "org.omegat.filters.AndroidFilterTest#testParse",
                new AndroidFilter(), Collections.emptyMap(),
                "MyApp", "MonApp");
    }

    private void exportFilters3() throws Exception {
        exportFilter("docbook", "docbook/file-DocBookFilter.json",
                "docBook/file-DocBookFilter.xml",
                "org.omegat.filters.DocBookFilterTest#testParse",
                new DocBookFilter(), Collections.emptyMap(), "My String", "GOLDEN_T");
        exportFilter("resx", "resx/Resources.json",
                "ResX/Resources.resx",
                "org.omegat.filters.ResXFilterTest#testParse",
                new ResXFilter(), Collections.emptyMap(),
                "This is a text displayed in the UI.", "GOLDEN_T");
        exportFilter("wix", "wix/fr-fr.json",
                "Wix/fr-fr.wxl",
                "org.omegat.filters.WiXFilterTest#testLoad",
                new WiXFilter(), Collections.emptyMap(),
                "This installation requires XXX. Setup will now exit.", "GOLDEN_T");
        exportFilter("xhtml", "xhtml/file-XHTMLFilter.json",
                "xhtml/file-XHTMLFilter.html",
                "org.omegat.filters.XHTMLFilterTest#testParse",
                new XHTMLFilter(), Collections.emptyMap(),
                "XHTML 1.0 Example", "GOLDEN_T");
        exportFilter("svg", "svg/Neural_network_example.json",
                "SVG/Neural_network_example.svg",
                "org.omegat.filters.SvgFilterTest#testLoad",
                new SvgFilter(), Collections.emptyMap(), null, null);
        exportFilter("relaxng", "relaxng/relaxng.json",
                "relaxng/relaxng.rng",
                "org.omegat.filters.RelaxNGFilterTest#testParse",
                new RelaxNGFilter(), Collections.emptyMap(),
                "RELAX NG is a schema language for XML.", "GOLDEN_T");
        exportFilter("helpandmanual", "helpandmanual/paragraph-tags.json",
                "helpandmanual/paragraph-tags.xml",
                "org.omegat.filters.HelpAndManualFilterTest#testParagraphTagsAreExtracted",
                new HelpAndManualFilter(), Collections.emptyMap(),
                "Caption Text", "GOLDEN_T");
        exportFilter("xmlss", "xmlss/XMLSpreadsheet2003.json",
                "XMLSpreadsheet/XMLSpreadsheet2003.xml",
                "org.omegat.filters.XMLSpreadsheetTest#testParse",
                new XMLSpreadsheetFilter(), Collections.emptyMap(), null, null);
        exportFilter("xliff", "xliff/file-XLIFFFilter.json",
                "xliff/filters3/file-XLIFFFilter.xlf",
                "org.omegat.filters.XLIFFFilterTest#testParse",
                new XLIFFFilter(), Collections.emptyMap(), null, null);
        exportZipFilter("opendoc", "opendoc/file-OpenDocFilter.json",
                "openDoc/file-OpenDocFilter.odt",
                "org.omegat.filters.OpenDocFilterTest#testParse",
                new OpenDocFilter(), Collections.emptyMap());
        exportZipFilter("openxml", "openxml/file-OpenXMLFilter.json",
                "openXML/file-OpenXMLFilter.docx",
                "org.omegat.filters.OpenXMLFilterTest#testParse",
                new OpenXMLFilter(), Collections.emptyMap());
        exportFilter("camtasia", "camtasia/simple.json",
                "CamtasiaWindows/simple.camproj",
                "org.omegat.filters3.xml.camtasiawindows.CamtasiaWindowsFilter#processFile",
                new CamtasiaWindowsFilter(), Collections.emptyMap(),
                "Hello Camtasia", "GOLDEN_T");
        exportFilter("flash", "flash/simple.json",
                "flash/simple.xml",
                "org.omegat.filters3.xml.flash.FlashFilter#processFile",
                new FlashFilter(), Collections.emptyMap(), "Hello", "GOLDEN_T");
        exportFilter("infix", "infix/simple.json",
                "infix/simple.xml",
                "org.omegat.filters3.xml.infix.InfixFilter#processFile",
                new InfixFilter(), Collections.emptyMap(), null, null);
        exportFilter("l10nmgr", "l10nmgr/simple.json",
                "l10nmgr/simple.xml",
                "org.omegat.filters3.xml.l10nmgr.L10nmgrFilter#processFile",
                new L10nmgrFilter(), Collections.emptyMap(), "Hello", "GOLDEN_T");
        exportFilter("propxml", "propxml/simple.json",
                "propxml/simple.xml",
                "org.omegat.filters3.xml.properties.PropertiesFilter#processFile",
                new PropertiesFilter(), Collections.emptyMap(), "Alpha", "GOLDEN_T");
        exportFilter("schematron", "schematron/simple.json",
                "schematron/simple.sch",
                "org.omegat.filters3.xml.schematron.SchematronFilter#processFile",
                new SchematronFilter(), Collections.emptyMap(), null, null);
        exportFilter("scribus", "scribus/Scribus.json",
                "Scribus/Scribus.sla",
                "org.omegat.filters3.xml.scribus.ScribusFilter#processFile",
                new ScribusFilter(), Collections.emptyMap(), null, null);
        exportFilter("txml", "txml/simple.json",
                "txml/simple.txml",
                "org.omegat.filters3.xml.txml.TXMLFilter#processFile",
                new TXMLFilter(), Collections.emptyMap(),
                "Hello target", "GOLDEN_T");
        exportFilter("typo3", "typo3/simple.json",
                "typo3/simple.xml",
                "org.omegat.filters3.xml.typo3.Typo3Filter#processFile",
                new Typo3Filter(), Collections.emptyMap(),
                "Hello Typo3", "GOLDEN_T");
        exportFilter("visio", "visio/simple.json",
                "visio/simple.vdx",
                "org.omegat.filters3.xml.visio.VisioFilter#processFile",
                new VisioFilter(), Collections.emptyMap(), null, null);
        exportFilter("wordpress", "wordpress/simple.json",
                "wordpress/simple.xml",
                "org.omegat.filters3.xml.wordpress.WordpressFilter#processFile",
                new WordpressFilter(), Collections.emptyMap(),
                "Hello WordPress", "GOLDEN_T");
    }

    private void exportFilters4() throws Exception {
        exportFilter("xliff1", "xliff1/en-xx.json",
                "xliff/filters4-xliff1/en-xx.xlf",
                "org.omegat.filters4.Xliff1FilterTest#testParse",
                new Xliff1Filter(), Collections.emptyMap(),
                "Should translate in result.", "Devrait traduire dans le résultat.");
        exportFilter("xliff2", "xliff2/ex.9.5.json",
                "xliff/filters4-xliff2/ex.9.5.xlf",
                "org.omegat.filters4.Xliff2FilterTest#testParse",
                new Xliff2Filter(), Collections.emptyMap(),
                "Birds in Oregon", "Oiseaux en Oregon");
        exportZipFilter("msoffice", "msoffice/file-OpenXMLFilter.json",
                "openXML/file-OpenXMLFilter.docx",
                "org.omegat.filters4.MsOfficeFileFilterTest#testParse",
                new MsOfficeFileFilter(), Collections.emptyMap());
        exportZipFilter("msoffice", "msoffice/file-OpenXMLFilter-tables.json",
                "openXML/file-OpenXMLFilter-tables.docx",
                "org.omegat.filters4.MsOfficeFileFilterTest#testParseTables",
                new MsOfficeFileFilter(), Collections.emptyMap());
        exportFilter("sdlxliff", "sdlxliff/simple.json",
                "sdl/simple.sdlxliff",
                "org.omegat.filters4.xml.xliff.SdlXliff#processFile",
                new SdlXliff(), Collections.emptyMap(),
                "Hello SDL", "GOLDEN_T");
        // Java SdlProject leaves getEntryComparator() null; parse then calls
        // translateEntry with a null ZipOutputStream and NPEs. A comparator
        // routes parse through translateEntries (the intended read path).
        SdlProject sdlProject = new SdlProject() {
            @Override
            protected java.util.Comparator<java.util.zip.ZipEntry> getEntryComparator() {
                return java.util.Comparator.comparing(java.util.zip.ZipEntry::getName);
            }
        };
        exportZipFilter("sdlproject", "sdlproject/simple.json",
                "sdl/simple.sdlppx",
                "org.omegat.filters4.xml.xliff.SdlProject#processFile",
                sdlProject, Collections.emptyMap());
    }

    private File resolveFixture(String fixtureRel) {
        Path a = javaRoot.resolve("src/test/resources/data/filters").resolve(fixtureRel);
        if (Files.isRegularFile(a)) {
            return a.toFile();
        }
        Path b = javaRoot.resolve("../../fixtures/filters").normalize().resolve(fixtureRel);
        if (Files.isRegularFile(b)) {
            return b.toFile();
        }
        throw new IllegalStateException("missing fixture " + fixtureRel + " tried " + a + " and " + b);
    }

    private void exportZipFilter(String id, String outRel, String fixtureRel, String javaTest,
            IFilter filter, Map<String, String> options) throws Exception {
        File in = resolveFixture(fixtureRel);
        filter.isFileSupported(in, options, context);
        List<Parsed> parsed = parse(filter, in, options);
        List<String> sources = new ArrayList<>();
        List<String> ids = new ArrayList<>();
        List<String> paths = new ArrayList<>();
        for (Parsed p : parsed) {
            sources.add(p.source);
            ids.add(p.id == null ? "" : p.id);
            paths.add(p.path == null ? "" : p.path);
        }
        Map<String, Object> json = new LinkedHashMap<>();
        json.put("id", id);
        json.put("fixture", fixtureRel);
        json.put("java_test", javaTest);
        json.put("exported_by", EXPORTED_BY);
        json.put("options", options);
        json.put("source_lang", "en");
        json.put("target_lang", "be");
        json.put("sources", sources);
        json.put("ids", ids);
        json.put("paths", paths);
        writeJson(goldenRoot.resolve("filters").resolve(outRel), json);
        System.out.println("wrote filters/" + outRel + " sources=" + sources.size() + " (zip, no write text)");
    }

    private void exportFilter(String id, String outRel, String fixtureRel, String javaTest,
            IFilter filter, Map<String, String> options, String trSource, String trTarget)
            throws Exception {
        File in = resolveFixture(fixtureRel);
        if (!in.isFile()) {
            throw new IllegalStateException("missing Java fixture " + in);
        }
        filter.isFileSupported(in, options, context);
        List<Parsed> parsed = parse(filter, in, options);
        List<String> sources = new ArrayList<>();
        List<String> ids = new ArrayList<>();
        List<String> paths = new ArrayList<>();
        for (Parsed p : parsed) {
            sources.add(p.source);
            ids.add(p.id == null ? "" : p.id);
            paths.add(p.path == null ? "" : p.path);
        }
        Path tmp = Files.createTempDirectory("omegat-export-");
        File emptyOut = tmp.resolve("empty-" + in.getName()).toFile();
        translate(filter, in, emptyOut, options, Collections.emptyMap(), filter.isBilingual());
        String emptyText = emptyOut.isFile() ? Files.readString(emptyOut.toPath(), StandardCharsets.UTF_8) : "";

        String translatedWrite = "";
        Map<String, Object> translated = null;
        if (trSource != null && trTarget != null) {
            String actualSource = resolveSource(parsed, trSource);
            Map<String, String> one = new LinkedHashMap<>();
            one.put(actualSource, trTarget);
            File trOut = tmp.resolve("tr-" + in.getName()).toFile();
            translate(filter, in, trOut, options, one, filter.isBilingual());
            translatedWrite = trOut.isFile() ? Files.readString(trOut.toPath(), StandardCharsets.UTF_8) : "";
            translated = new LinkedHashMap<>();
            translated.put("source", actualSource);
            translated.put("translation", trTarget);
        }

        Map<String, Object> json = new LinkedHashMap<>();
        json.put("id", id);
        json.put("fixture", fixtureRel);
        json.put("java_test", javaTest);
        json.put("exported_by", EXPORTED_BY);
        json.put("options", options);
        json.put("source_lang", "en");
        json.put("target_lang", "be");
        json.put("sources", sources);
        json.put("ids", ids);
        json.put("paths", paths);
        json.put("empty_write_text", emptyText);
        if (translated != null) {
            json.put("translated", translated);
            json.put("translated_write", translatedWrite);
        }
        writeJson(goldenRoot.resolve("filters").resolve(outRel), json);
        System.out.println("wrote filters/" + outRel + " sources=" + sources.size());
    }

    private static String resolveSource(List<Parsed> parsed, String wanted) {
        for (Parsed p : parsed) {
            if (p.source.equals(wanted)) {
                return p.source;
            }
        }
        for (Parsed p : parsed) {
            if (p.source.startsWith(wanted)) {
                return p.source;
            }
        }
        return wanted;
    }

    private static final class Parsed {
        final String id;
        final String source;
        final String path;

        Parsed(String id, String source, String path) {
            this.id = id;
            this.source = source;
            this.path = path;
        }
    }

    private List<Parsed> parse(IFilter filter, File in, Map<String, String> options) throws Exception {
        List<Parsed> result = new ArrayList<>();
        filter.parseFile(in, options, context, new IParseCallback() {
            @Override
            public void addEntry(String id, String source, String translation, boolean isFuzzy, String comment,
                    String path, IFilter filter, List<org.omegat.core.data.ProtectedPart> protectedParts) {
                addEntryWithProperties(id, source, translation, isFuzzy, null, path, filter, protectedParts);
            }

            @Override
            public void addEntryWithProperties(String id, String source, String translation, boolean isFuzzy,
                    String[] props, String path, IFilter filter,
                    List<org.omegat.core.data.ProtectedPart> protectedParts) {
                if (source != null && !source.isEmpty()) {
                    result.add(new Parsed(id, source, path));
                }
            }

            @Override
            public void linkPrevNextSegments() {
            }
        });
        return result;
    }

    private void translate(IFilter filter, File in, File out, Map<String, String> options,
            Map<String, String> translations, boolean allowBlank) throws Exception {
        File parent = out.getParentFile();
        if (parent != null) {
            parent.mkdirs();
        }
        filter.translateFile(in, out, options, context, new ITranslateCallback() {
            @Override
            public String getTranslation(String id, String source, String path) {
                String translation = translations.get(source);
                if (translation == null && id != null) {
                    translation = translations.get(id);
                }
                if (translation == null && !allowBlank) {
                    return source;
                }
                return translation;
            }

            @Override
            public String getTranslation(String id, String source) {
                return getTranslation(id, source, null);
            }

            @Override
            public void linkPrevNextSegments() {
            }

            @Override
            public void setPass(int pass) {
            }
        });
    }

    private void exportEngine() throws Exception {
        Segmenter segmenter = new Segmenter(SRX.getDefault());
        List<Map<String, Object>> srxCases = new ArrayList<>();
        srxCases.add(srxCase(segmenter, "en", "<br7>\n\n<br5>\n\nother",
                "org.omegat.core.segmentation.SegmenterTest#testSegment"));
        srxCases.add(srxCase(segmenter, "en", "Mr. Smith went home. Next sentence.",
                "org.omegat.core.segmentation.Segmenter#segment"));
        srxCases.add(srxCase(segmenter, "en", "Hello world. How are you? Fine.",
                "org.omegat.core.segmentation.Segmenter#segment"));
        srxCases.add(srxCase(segmenter, "de", "Hallo Welt. Nächster Satz.",
                "org.omegat.core.segmentation.Segmenter#segment"));
        srxCases.add(srxCase(segmenter, "fr", "M. Dupont est arrive. Ensuite il part.",
                "org.omegat.core.segmentation.Segmenter#segment"));
        srxCases.add(srxCase(segmenter, "zh", "你好。世界。",
                "org.omegat.core.segmentation.Segmenter#segment"));
        srxCases.add(srxCase(segmenter, "ja", "こんにちは。世界。",
                "org.omegat.core.segmentation.Segmenter#segment"));
        Map<String, Object> srx = new LinkedHashMap<>();
        srx.put("java_test", "org.omegat.core.segmentation.SegmenterTest#testSegment");
        srx.put("exported_by", EXPORTED_BY);
        srx.put("cases", srxCases);
        writeJson(goldenRoot.resolve("engine/srx.json"), srx);

        List<Map<String, Object>> fuzzyCases = new ArrayList<>();
        fuzzyCases.add(fuzzyCase("Hello world", "Hello world"));
        fuzzyCases.add(fuzzyCase("Hello world", "Hello word"));
        fuzzyCases.add(fuzzyCase("Hello world", "Goodbye"));
        Map<String, Object> fuzzy = new LinkedHashMap<>();
        fuzzy.put("java_test", "org.omegat.core.matching.FuzzyMatcher#calcSimilarity");
        fuzzy.put("exported_by", EXPORTED_BY);
        fuzzy.put("cases", fuzzyCases);
        writeJson(goldenRoot.resolve("engine/fuzzy.json"), fuzzy);

        List<Map<String, Object>> tokenCases = new ArrayList<>();
        tokenCases.add(tokenCase(new DefaultTokenizer(), "en", "Hello worlds running", ITokenizer.StemmingMode.NONE));
        tokenCases.add(tokenCase(new LuceneEnglishTokenizer(), "en", "Hello worlds running",
                ITokenizer.StemmingMode.GLOSSARY));
        tokenCases.add(tokenCase(new LuceneCJKTokenizer(), "zh", "汉字词", ITokenizer.StemmingMode.NONE));
        tokenCases.add(tokenCase(new LuceneCJKTokenizer(), "ja", "日本語", ITokenizer.StemmingMode.NONE));
        Map<String, Object> tokens = new LinkedHashMap<>();
        tokens.put("java_test", "org.omegat.tokenizer.DefaultTokenizer#tokenizeWords");
        tokens.put("exported_by", EXPORTED_BY);
        tokens.put("cases", tokenCases);
        writeJson(goldenRoot.resolve("engine/tokens.json"), tokens);

        System.out.println("wrote engine srx/fuzzy/tokens");
    }

    private void exportGlossary() throws Exception {
        File tab = javaRoot.resolve("src/test/resources/data/glossaries/test.tab").toFile();
        List<GlossaryEntry> entries = GlossaryReaderTSV.read(tab, false);
        List<Map<String, Object>> parsed = new ArrayList<>();
        for (GlossaryEntry e : entries) {
            Map<String, Object> m = new LinkedHashMap<>();
            m.put("source", e.getSrcText());
            m.put("target", e.getLocText());
            m.put("comment", e.getCommentText() == null ? "" : e.getCommentText());
            parsed.add(m);
        }
        List<GlossaryEntry> extra = new ArrayList<>(entries);
        extra.add(new GlossaryEntry("running", "courir", "verb", false, "origin"));
        extra.add(new GlossaryEntry("Cat", "chat", "", false, "origin"));
        List<Map<String, Object>> cases = new ArrayList<>();
        cases.add(glossaryCase(extra, "I use kde daily", true, false, "en", "fr"));
        cases.add(glossaryCase(extra, "The CAT sat", true, false, "en", "fr"));
        cases.add(glossaryCase(extra, "The CAT sat", false, false, "en", "fr"));
        cases.add(glossaryCase(extra, "I was running yesterday", true, true, "en", "fr"));
        cases.add(glossaryCase(extra, "I was running yesterday", true, false, "en", "fr"));
        Map<String, Object> json = new LinkedHashMap<>();
        json.put("java_test", "org.omegat.gui.glossary.GlossaryReaderTSVTest#testRead");
        json.put("exported_by", EXPORTED_BY);
        json.put("fixture", "src/test/resources/data/glossaries/test.tab");
        json.put("entries", parsed);
        json.put("cases", cases);
        writeJson(goldenRoot.resolve("engine/glossary.json"), json);
        System.out.println("wrote engine/glossary.json entries=" + parsed.size());
    }

    private Map<String, Object> glossaryCase(List<GlossaryEntry> entries, String segment, boolean ignoreCase,
            boolean useStem, String srcLang, String tgtLang) {
        Preferences.setPreference(Preferences.GLOSSARY_STEMMING, useStem);
        Preferences.setPreference(Preferences.GLOSSARY_STEMMING_FULL, useStem);
        Preferences.setPreference(Preferences.GLOSSARY_NOT_EXACT_MATCH, true);
        Preferences.setPreference(Preferences.GLOSSARY_REQUIRE_SIMILAR_CASE, !ignoreCase);
        GlossarySearcher searcher = new GlossarySearcher(new DefaultTokenizer(), new Language(srcLang),
                new Language(tgtLang), false);
        EntryKey key = new EntryKey("f", segment, null, null, null, null);
        SourceTextEntry ste = new SourceTextEntry(key, 1, null, null, Collections.emptyList());
        List<String> targets = new ArrayList<>();
        for (GlossaryEntry hit : searcher.searchSourceMatches(ste, entries)) {
            targets.add(hit.getLocText());
        }
        Map<String, Object> c = new LinkedHashMap<>();
        c.put("segment", segment);
        c.put("ignore_case", ignoreCase);
        c.put("use_stem", useStem);
        c.put("src_lang", srcLang);
        c.put("tgt_lang", tgtLang);
        c.put("targets", targets);
        return c;
    }

    private void exportStats() throws Exception {
        String[] headers = { "repetition", "repetition_other", "exact", "fuzzy_95", "fuzzy_85", "fuzzy_75",
                "fuzzy_50", "none", "total" };
        int[] percents = { Statistics.PERCENT_EXACT_MATCH, 100, 95, 94, 85, 84, 75, 74, 50, 49, 0 };
        List<Map<String, Object>> cases = new ArrayList<>();
        for (int p : percents) {
            MatchStatCounts counts = new MatchStatCounts();
            StatCount sc = new StatCount();
            sc.segments = 1;
            if (p == Statistics.PERCENT_EXACT_MATCH) {
                counts.addExact(sc);
            } else {
                counts.addForPercents(p, sc);
            }
            String[][] table = counts.calcTable(headers);
            String bin = "none";
            for (int i = 0; i < 8; i++) {
                if ("1".equals(table[i][1])) {
                    bin = headers[i];
                    break;
                }
            }
            Map<String, Object> row = new LinkedHashMap<>();
            row.put("percent", p);
            row.put("bin", bin);
            cases.add(row);
        }
        List<Map<String, Object>> wordCounts = new ArrayList<>();
        for (String text : List.of("Hello world", "Second line", "你好")) {
            Map<String, Object> m = new LinkedHashMap<>();
            m.put("text", text);
            m.put("words", Statistics.numberOfWords(text));
            m.put("chars_nosp", Statistics.numberOfCharactersWithoutSpaces(text));
            m.put("chars", Statistics.numberOfCharactersWithSpaces(text));
            wordCounts.add(m);
        }
        Map<String, Object> json = new LinkedHashMap<>();
        json.put("java_test", "org.omegat.core.statistics.CalcMatchStatisticsTest#testCalcMatchStatics");
        json.put("exported_by", EXPORTED_BY);
        json.put("cases", cases);
        json.put("word_counts", wordCounts);
        json.put("percent_exact_match", Statistics.PERCENT_EXACT_MATCH);
        writeJson(goldenRoot.resolve("engine/stats.json"), json);
        System.out.println("wrote engine/stats.json bins=" + cases.size());
    }

    private Map<String, Object> srxCase(Segmenter segmenter, String lang, String input, String javaTest) {
        List<StringBuilder> spaces = new ArrayList<>();
        List<String> sentences = segmenter.segment(new Language(lang), input, spaces, new ArrayList<Rule>());
        Map<String, Object> c = new LinkedHashMap<>();
        c.put("lang", lang);
        c.put("input", input);
        c.put("sentences", sentences);
        c.put("java_test", javaTest);
        return c;
    }

    private Map<String, Object> fuzzyCase(String query, String candidate) {
        DefaultTokenizer tok = new DefaultTokenizer();
        Token[] a = tok.tokenizeWords(query, ITokenizer.StemmingMode.NONE);
        Token[] b = tok.tokenizeWords(candidate, ITokenizer.StemmingMode.NONE);
        int score = FuzzyMatcher.calcSimilarity(new LevenshteinDistance(), a, b);
        Map<String, Object> c = new LinkedHashMap<>();
        c.put("lang", "en");
        c.put("query", query);
        c.put("candidate", candidate);
        c.put("score", score);
        c.put("query_tokens", Token.getTextsFromString(a, query));
        c.put("candidate_tokens", Token.getTextsFromString(b, candidate));
        return c;
    }

    private Map<String, Object> tokenCase(ITokenizer tokenizer, String lang, String input,
            ITokenizer.StemmingMode mode) {
        Token[] tokens = tokenizer.tokenizeWords(input, mode);
        String[] texts = Token.getTextsFromString(tokens, input);
        Map<String, Object> c = new LinkedHashMap<>();
        c.put("lang", lang);
        c.put("input", input);
        c.put("tokenizer", tokenizer.getClass().getName());
        c.put("stemming", mode.name());
        c.put("tokens", List.of(texts));
        return c;
    }

    private void writeJson(Path path, Map<String, Object> data) throws Exception {
        Files.createDirectories(path.getParent());
        Files.writeString(path, toJson(data, 0) + "\n", StandardCharsets.UTF_8);
    }

    @SuppressWarnings("unchecked")
    private static String toJson(Object value, int indent) {
        String pad = "  ".repeat(indent);
        String pad1 = "  ".repeat(indent + 1);
        if (value == null) {
            return "null";
        }
        if (value instanceof String s) {
            return quote(s);
        }
        if (value instanceof Number || value instanceof Boolean) {
            return String.valueOf(value);
        }
        if (value instanceof Map<?, ?> map) {
            StringBuilder sb = new StringBuilder();
            sb.append("{\n");
            int i = 0;
            for (Map.Entry<?, ?> e : map.entrySet()) {
                if (i++ > 0) {
                    sb.append(",\n");
                }
                sb.append(pad1).append(quote(String.valueOf(e.getKey()))).append(": ")
                        .append(toJson(e.getValue(), indent + 1));
            }
            sb.append("\n").append(pad).append("}");
            return sb.toString();
        }
        if (value instanceof Iterable<?> it) {
            StringBuilder sb = new StringBuilder();
            sb.append("[\n");
            int i = 0;
            for (Object o : it) {
                if (i++ > 0) {
                    sb.append(",\n");
                }
                sb.append(pad1).append(toJson(o, indent + 1));
            }
            sb.append("\n").append(pad).append("]");
            return sb.toString();
        }
        if (value instanceof Object[] arr) {
            return toJson(List.of(arr), indent);
        }
        return quote(String.valueOf(value));
    }

    private static String quote(String s) {
        StringBuilder sb = new StringBuilder("\"");
        for (int i = 0; i < s.length(); i++) {
            char c = s.charAt(i);
            switch (c) {
            case '"':
                sb.append("\\\"");
                break;
            case '\\':
                sb.append("\\\\");
                break;
            case '\n':
                sb.append("\\n");
                break;
            case '\r':
                sb.append("\\r");
                break;
            case '\t':
                sb.append("\\t");
                break;
            default:
                if (c < 0x20) {
                    sb.append(String.format("\\u%04x", (int) c));
                } else {
                    sb.append(c);
                }
            }
        }
        sb.append('"');
        return sb.toString();
    }
}
