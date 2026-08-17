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
import java.lang.reflect.Constructor;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Set;
import java.util.TreeMap;
import java.util.TreeSet;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

import org.omegat.core.Core;
import org.omegat.core.data.EntryKey;
import org.omegat.core.data.NotLoadedProject;
import org.omegat.core.data.ProjectProperties;
import org.omegat.core.data.ProtectedPart;
import org.omegat.core.data.RealProjectTest;
import org.omegat.core.data.SourceTextEntry;
import org.omegat.core.events.IStopped;
import org.omegat.core.matching.FuzzyMatcher;
import org.omegat.core.matching.LevenshteinDistance;
import org.omegat.core.matching.NearString;
import org.omegat.core.segmentation.Rule;
import org.omegat.core.segmentation.SRX;
import org.omegat.core.segmentation.Segmenter;
import org.omegat.core.statistics.CalcMatchStatistics;
import org.omegat.core.statistics.CalcPerFileMatchStatistics;
import org.omegat.core.statistics.CalcStandardStatistics;
import org.omegat.core.statistics.FindMatches;
import org.omegat.core.statistics.FindMatchesTest;
import org.omegat.core.statistics.ICalcStatistics;
import org.omegat.core.statistics.Statistics;
import org.omegat.core.statistics.TestingProject;
import org.omegat.core.statistics.TestingStatsConsumer;
import org.omegat.core.statistics.dso.MatchStatCounts;
import org.omegat.core.statistics.dso.StatCount;
import org.omegat.core.tagvalidation.ErrorReport;
import org.omegat.core.tagvalidation.TagRepair;
import org.omegat.core.tagvalidation.TagValidation;
import org.omegat.core.threads.CancellationToken;
import org.omegat.core.threads.Completion;
import org.omegat.tokenizer.ITokenizer;
import org.omegat.util.OConsts;
import org.omegat.util.StringUtil;
import org.omegat.util.Token;
import org.omegat.util.TMXReader2;
import org.omegat.util.TMXWriter2;
import org.omegat.util.TagUtil.Tag;
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
import org.omegat.filters2.html2.HTMLOptions;
import org.omegat.filters3.xml.DefaultXMLDialect;
import org.omegat.filters3.xml.XMLDialect;
import org.omegat.filters3.xml.android.AndroidDialect;
import org.omegat.filters3.xml.android.AndroidFilter;
import org.omegat.filters3.xml.camtasiawindows.CamtasiaWindowsDialect;
import org.omegat.filters3.xml.camtasiawindows.CamtasiaWindowsFilter;
import org.omegat.filters3.xml.docbook.DocBookDialect;
import org.omegat.filters3.xml.docbook.DocBookFilter;
import org.omegat.filters3.xml.flash.FlashDialect;
import org.omegat.filters3.xml.flash.FlashFilter;
import org.omegat.filters3.xml.helpandmanual.HelpAndManualDialect;
import org.omegat.filters3.xml.helpandmanual.HelpAndManualFilter;
import org.omegat.filters3.xml.infix.InfixDialect;
import org.omegat.filters3.xml.infix.InfixFilter;
import org.omegat.filters3.xml.l10nmgr.L10nmgrDialect;
import org.omegat.filters3.xml.l10nmgr.L10nmgrFilter;
import org.omegat.filters3.xml.opendoc.OpenDocDialect;
import org.omegat.filters3.xml.opendoc.OpenDocFilter;
import org.omegat.filters3.xml.opendoc.OpenDocOptions;
import org.omegat.filters3.xml.openxml.OpenXMLDialect;
import org.omegat.filters3.xml.openxml.OpenXMLFilter;
import org.omegat.filters3.xml.openxml.OpenXMLOptions;
import org.omegat.filters3.xml.properties.PropertiesDialect;
import org.omegat.filters3.xml.properties.PropertiesFilter;
import org.omegat.filters3.xml.relaxng.RelaxNGDialect;
import org.omegat.filters3.xml.relaxng.RelaxNGFilter;
import org.omegat.filters3.xml.resx.ResXDialect;
import org.omegat.filters3.xml.resx.ResXFilter;
import org.omegat.filters3.xml.schematron.SchematronDialect;
import org.omegat.filters3.xml.schematron.SchematronFilter;
import org.omegat.filters3.xml.scribus.ScribusDialect;
import org.omegat.filters3.xml.scribus.ScribusFilter;
import org.omegat.filters3.xml.svg.SvgDialect;
import org.omegat.filters3.xml.svg.SvgFilter;
import org.omegat.filters3.xml.txml.TXMLDialect;
import org.omegat.filters3.xml.txml.TXMLFilter;
import org.omegat.filters3.xml.typo3.Typo3Dialect;
import org.omegat.filters3.xml.typo3.Typo3Filter;
import org.omegat.filters3.xml.visio.VisioDialect;
import org.omegat.filters3.xml.visio.VisioFilter;
import org.omegat.filters3.xml.wix.WiXDialect;
import org.omegat.filters3.xml.wix.WiXFilter;
import org.omegat.filters3.xml.wordpress.WordpressDialect;
import org.omegat.filters3.xml.wordpress.WordpressFilter;
import org.omegat.filters3.xml.xhtml.XHTMLDialect;
import org.omegat.filters3.xml.xhtml.XHTMLFilter;
import org.omegat.filters3.xml.xhtml.XHTMLOptions;
import org.omegat.filters3.xml.xliff.XLIFFDialect;
import org.omegat.filters3.xml.xliff.XLIFFFilter;
import org.omegat.filters3.xml.xliff.XLIFFOptions;
import org.omegat.filters3.xml.xmlspreadsheet.XMLSpreadsheetDialect;
import org.omegat.filters3.xml.xmlspreadsheet.XMLSpreadsheetFilter;
import org.omegat.filters4.xml.openxml.MsOfficeFileFilter;
import org.omegat.filters4.xml.xliff.SdlProject;
import org.omegat.filters4.xml.xliff.SdlXliff;
import org.omegat.filters4.xml.xliff.Xliff1Filter;
import org.omegat.filters4.xml.xliff.Xliff2Filter;
import org.omegat.gui.editor.IEditor;
import org.omegat.gui.glossary.GlossaryEntry;
import org.omegat.gui.glossary.GlossaryReaderTSV;
import org.omegat.gui.glossary.GlossarySearcher;
import org.omegat.gui.main.MainWindowMenuHandler;
import org.omegat.tokenizer.DefaultTokenizer;
import org.omegat.tokenizer.ITokenizer;
import org.omegat.tokenizer.LuceneArabicTokenizer;
import org.omegat.tokenizer.LuceneArmenianTokenizer;
import org.omegat.tokenizer.LuceneBasqueTokenizer;
import org.omegat.tokenizer.LuceneBrazilianTokenizer;
import org.omegat.tokenizer.LuceneBulgarianTokenizer;
import org.omegat.tokenizer.LuceneCJKTokenizer;
import org.omegat.tokenizer.LuceneCatalanTokenizer;
import org.omegat.tokenizer.LuceneCzechTokenizer;
import org.omegat.tokenizer.LuceneDanishTokenizer;
import org.omegat.tokenizer.LuceneDutchTokenizer;
import org.omegat.tokenizer.LuceneEnglishTokenizer;
import org.omegat.tokenizer.LuceneFinnishTokenizer;
import org.omegat.tokenizer.LuceneFrenchTokenizer;
import org.omegat.tokenizer.LuceneGalicianTokenizer;
import org.omegat.tokenizer.LuceneGermanTokenizer;
import org.omegat.tokenizer.LuceneGreekTokenizer;
import org.omegat.tokenizer.LuceneHindiTokenizer;
import org.omegat.tokenizer.LuceneHungarianTokenizer;
import org.omegat.tokenizer.LuceneIndonesianTokenizer;
import org.omegat.tokenizer.LuceneIrishTokenizer;
import org.omegat.tokenizer.LuceneItalianTokenizer;
import org.omegat.tokenizer.LuceneJapaneseTokenizer;
import org.omegat.tokenizer.LuceneLatvianTokenizer;
import org.omegat.tokenizer.LuceneNorwegianTokenizer;
import org.omegat.tokenizer.LucenePersianTokenizer;
import org.omegat.tokenizer.LucenePolishTokenizer;
import org.omegat.tokenizer.LucenePortugueseTokenizer;
import org.omegat.tokenizer.LuceneRomanianTokenizer;
import org.omegat.tokenizer.LuceneRussianTokenizer;
import org.omegat.tokenizer.LuceneSmartChineseTokenizer;
import org.omegat.tokenizer.LuceneSpanishTokenizer;
import org.omegat.tokenizer.LuceneSwedishTokenizer;
import org.omegat.tokenizer.LuceneThaiTokenizer;
import org.omegat.tokenizer.LuceneTurkishTokenizer;
import org.omegat.util.HTMLUtils;
import org.omegat.util.Language;
import org.omegat.util.MultiMap;
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
        ExportGoldens exporter = new ExportGoldens(javaRoot, goldenRoot);
        if (args.length > 1 && "honesty".equals(args[1])) {
            exporter.exportHonesty();
            System.out.println("ExportGoldens wrote honesty surfaces to " + goldenRoot);
        } else if (args.length > 1 && "engine".equals(args[1])) {
            exporter.exportEngine();
            exporter.exportGlossary();
            exporter.exportStats();
            exporter.exportP1Core();
            exporter.exportHonesty();
            System.out.println("ExportGoldens wrote engine goldens to " + goldenRoot);
        } else {
            exporter.run();
            exporter.exportHonesty();
        }
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
        exportP1Core();
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

        String enOrig = "The quick, brown <x0/> jumped over 1 \"lazy\" dog.";
        tokenCases.add(tokenWordsCase(new LuceneEnglishTokenizer(), "en", enOrig, ITokenizer.StemmingMode.NONE,
                "org.omegat.tokenizer.TokenizerTest#testEnglish"));
        tokenCases.add(tokenWordsCase(new LuceneEnglishTokenizer(), "en", enOrig, ITokenizer.StemmingMode.GLOSSARY,
                "org.omegat.tokenizer.TokenizerTest#testEnglish"));
        tokenCases.add(tokenWordsCase(new LuceneEnglishTokenizer(), "en", enOrig, ITokenizer.StemmingMode.GLOSSARY_FULL,
                "org.omegat.tokenizer.TokenizerTest#testEnglish"));
        tokenCases.add(tokenWordsCase(new LuceneEnglishTokenizer(), "en", enOrig, ITokenizer.StemmingMode.MATCHING,
                "org.omegat.tokenizer.TokenizerTest#testEnglish"));
        tokenCases.add(tokenWordsCase(new LuceneEnglishTokenizer(), "en", "organisation organization",
                ITokenizer.StemmingMode.GLOSSARY, "org.omegat.tokenizer.TokenizerTest#testEnglish"));
        tokenCases.add(tokenWordsCase(new LuceneEnglishTokenizer(), "en", "organisation organization",
                ITokenizer.StemmingMode.GLOSSARY_FULL, "org.omegat.tokenizer.TokenizerTest#testEnglish"));

        String defOrig = "The quick, brown <x0/> jumped over 1 \"lazy\" \u0130stanbul. "
                + "\u65E5\u672C\u8A9E\u3042\u3044\u3046\u3048\u304A\u3002";
        tokenCases.add(tokenWordsCase(new DefaultTokenizer(), "en", defOrig, ITokenizer.StemmingMode.NONE,
                "org.omegat.tokenizer.TokenizerTest#testDefault"));
        tokenCases.add(tokenWordsCase(new DefaultTokenizer(), "en", defOrig, ITokenizer.StemmingMode.GLOSSARY,
                "org.omegat.tokenizer.TokenizerTest#testDefault"));
        tokenCases.add(tokenWordsCase(new DefaultTokenizer(), "en", defOrig, ITokenizer.StemmingMode.MATCHING,
                "org.omegat.tokenizer.TokenizerTest#testDefault"));

        tokenCases.add(tokenWordsCase(new LuceneGermanTokenizer(), "de", "pr\u00e4sentierte",
                ITokenizer.StemmingMode.GLOSSARY, "org.omegat.tokenizer.TokenizerTest#testGerman"));
        tokenCases.add(tokenWordsCase(new LuceneGermanTokenizer(), "de", "pr\u00e4sentieren",
                ITokenizer.StemmingMode.GLOSSARY, "org.omegat.tokenizer.TokenizerTest#testGerman"));
        tokenCases.add(tokenWordsCase(new LuceneItalianTokenizer(), "it", "paesi", ITokenizer.StemmingMode.GLOSSARY,
                "org.omegat.tokenizer.TokenizerTest#testItalian"));
        tokenCases.add(tokenWordsCase(new LuceneItalianTokenizer(), "it", "paesi", ITokenizer.StemmingMode.GLOSSARY_FULL,
                "org.omegat.tokenizer.TokenizerTest#testItalian"));

        String trOrig = "\u201C\u0130stanbul a\u011Fz\u0131\u201D, T\u00FCrkiye T\u00FCrk\u00E7esi"
                + "yaz\u0131 dilinin kayna\u011F\u0131 olarak kabul edilir; yaz\u0131 dili bu"
                + "a\u011F\u0131z temelinde olu\u015Fmu\u015Ftur.";
        tokenCases.add(tokenWordsCase(new LuceneTurkishTokenizer(), "tr", trOrig, ITokenizer.StemmingMode.NONE,
                "org.omegat.tokenizer.TokenizerTest#testTurkish"));
        tokenCases.add(tokenWordsCase(new LuceneTurkishTokenizer(), "tr", trOrig, ITokenizer.StemmingMode.GLOSSARY,
                "org.omegat.tokenizer.TokenizerTest#testTurkish"));
        tokenCases.add(tokenWordsCase(new LuceneTurkishTokenizer(), "tr", trOrig, ITokenizer.StemmingMode.MATCHING,
                "org.omegat.tokenizer.TokenizerTest#testTurkish"));

        String jaTags = "<x0/>\u3042</x0>\u300C<x1/>\u300D<x2/>\u3002<foo bar 123";
        tokenCases.add(tokenWordsCase(new LuceneJapaneseTokenizer(), "ja", jaTags, ITokenizer.StemmingMode.NONE,
                "org.omegat.tokenizer.TokenizerTest#testJapanese"));
        tokenCases.add(tokenWordsCase(new LuceneJapaneseTokenizer(), "ja", jaTags, ITokenizer.StemmingMode.MATCHING,
                "org.omegat.tokenizer.TokenizerTest#testJapanese"));

        String zhOrig = "\u6F22\u8A9E\u7684\u6587\u5B57\u7CFB\u7D71\u2014\u2014\u6F22\u5B57\u662F"
                + "\u4E00\u7A2E\u610F\u97F3\u8A9E\u8A00\uFF0C\u8868\u610F\u7684\u540C\u6642\u4E5F"
                + "\u5177\u4E00\u5B9A\u7684\u8868\u97F3\u529F\u80FD\u3002";
        tokenCases.add(tokenWordsCase(new LuceneSmartChineseTokenizer(), "zh", zhOrig, ITokenizer.StemmingMode.NONE,
                "org.omegat.tokenizer.TokenizerTest#testChinese"));
        tokenCases.add(tokenWordsCase(new LuceneSmartChineseTokenizer(), "zh", zhOrig, ITokenizer.StemmingMode.GLOSSARY,
                "org.omegat.tokenizer.TokenizerTest#testChinese"));
        tokenCases.add(tokenWordsCase(new LuceneSmartChineseTokenizer(), "zh", zhOrig, ITokenizer.StemmingMode.MATCHING,
                "org.omegat.tokenizer.TokenizerTest#testChinese"));

        // Language-body fixtures (not Latin "Hello worlds running") × NONE/GLOSSARY/MATCHING.
        // English-family may still use TokenizerTest English sentences above.
        Object[][] luceneLang = languageTokenizerFixtures();
        ITokenizer.StemmingMode[] modes = { ITokenizer.StemmingMode.NONE, ITokenizer.StemmingMode.GLOSSARY,
                ITokenizer.StemmingMode.MATCHING };
        for (Object[] row : luceneLang) {
            ITokenizer tok = (ITokenizer) row[0];
            String lang = (String) row[1];
            String input = (String) row[2];
            String javaTest = (String) row[3];
            bindTokenizer(tok, lang);
            for (ITokenizer.StemmingMode mode : modes) {
                tokenCases.add(tokenWordsCase(tok, lang, input, mode, javaTest));
            }
        }

        Map<String, Object> tokens = new LinkedHashMap<>();
        tokens.put("java_test", "org.omegat.tokenizer.TokenizerTest#testEnglish");
        tokens.put("exported_by", EXPORTED_BY);
        tokens.put("cases", tokenCases);
        writeJson(goldenRoot.resolve("engine/tokens.json"), tokens);

        System.out.println("wrote engine srx/fuzzy/tokens cases=" + tokenCases.size());
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

    /**
     * P1: every public test* on Segmenter / Levenshtein / TagValidation /
     * TagRepair / TMXWriter / FindMatches / CalcMatchStatistics.
     */
    private void exportP1Core() throws Exception {
        exportSegmenterTests();
        exportLevenshteinTests();
        exportTagValidationTests();
        exportTagRepairTests();
        exportTmxWriterTests();
        exportFindMatchesTests();
        exportCalcMatchStatisticsTests();
    }

    private void exportSegmenterTests() throws Exception {
        Segmenter segmenter = new Segmenter(SRX.getDefault());
        List<Map<String, Object>> cases = new ArrayList<>();

        List<StringBuilder> spaces = new ArrayList<>();
        List<Rule> brules = new ArrayList<>();
        String input = "<br7>\n\n<br5>\n\nother";
        List<String> segs = segmenter.segment(new Language("en"), input, spaces, brules);
        Map<String, Object> c = new LinkedHashMap<>();
        c.put("java_test", "org.omegat.core.segmentation.SegmenterTest#testSegment");
        c.put("name", "testSegment");
        c.put("lang", "en");
        c.put("input", input);
        c.put("sentences", segs);
        c.put("spaces", spaces.stream().map(StringBuilder::toString).toList());
        cases.add(c);

        spaces = new ArrayList<>();
        brules = new ArrayList<>();
        String oldString = "<br7>\n\n<br5>\n\nother";
        segs = segmenter.segment(new Language("en"), oldString, spaces, brules);
        String glued = segmenter.glue(new Language("en"), new Language("fr"), segs, spaces, brules);
        c = new LinkedHashMap<>();
        c.put("java_test", "org.omegat.core.segmentation.SegmenterTest#testGlue");
        c.put("name", "testGlue");
        c.put("source_lang", "en");
        c.put("target_lang", "fr");
        c.put("input", oldString);
        c.put("sentences", segs);
        c.put("spaces", spaces.stream().map(StringBuilder::toString).toList());
        c.put("glued", glued);
        cases.add(c);

        String[] glueInputs = {
                "Foo. Bar.\nHere.\n\nThere.\r\nThis.\tThat.\n\tOther.",
                "Foo. \n Bar.",
                "Foo. \t Bar."
        };
        for (String src : glueInputs) {
            spaces = new ArrayList<>();
            brules = new ArrayList<>();
            segs = new ArrayList<>(segmenter.segment(new Language("en"), src, spaces, brules));
            for (int i = 0; i < segs.size(); i++) {
                segs.set(i, segs.get(i).replace(".", "\\u3002"));
            }
            glued = segmenter.glue(new Language("en"), new Language("ja"), segs, spaces, brules);
            c = new LinkedHashMap<>();
            c.put("java_test", "org.omegat.core.segmentation.SegmenterTest#testGlueCJK");
            c.put("name", "testGlueCJK");
            c.put("source_lang", "en");
            c.put("target_lang", "ja");
            c.put("input", src);
            c.put("sentences", segs);
            c.put("spaces", spaces.stream().map(StringBuilder::toString).toList());
            c.put("glued", glued);
            cases.add(c);
        }

        Map<String, Object> json = new LinkedHashMap<>();
        json.put("java_test", "org.omegat.core.segmentation.SegmenterTest#testSegment");
        json.put("exported_by", EXPORTED_BY);
        json.put("cases", cases);
        writeJson(goldenRoot.resolve("engine/segmenter_tests.json"), json);
        System.out.println("wrote engine/segmenter_tests.json cases=" + cases.size());
    }

    private void exportLevenshteinTests() throws Exception {
        LevenshteinDistance calc = new LevenshteinDistance();
        List<Map<String, Object>> cases = new ArrayList<>();
        cases.add(levCase("testIdenticalTokens",
                new String[] { "test", "example" }, new String[] { "test", "example" },
                calc.compute(tokens("test", "example"), tokens("test", "example")), false));
        cases.add(levCase("testSourceNonEmptyTargetEmpty",
                new String[] { "alpha", "beta" }, new String[] {},
                calc.compute(tokens("alpha", "beta"), new Token[0]), false));
        cases.add(levCase("testSourceEmptyTargetNonEmpty",
                new String[] {}, new String[] { "gamma", "delta", "epsilon" },
                calc.compute(new Token[0], tokens("gamma", "delta", "epsilon")), false));
        cases.add(levCase("testCompletelyDifferentTokens",
                new String[] { "A", "B", "C" }, new String[] { "X", "Y", "Z" },
                calc.compute(tokens("A", "B", "C"), tokens("X", "Y", "Z")), false));
        cases.add(levCase("testPartiallySimilarTokens",
                new String[] { "cat", "dog", "fish" }, new String[] { "cat", "wolf", "fish" },
                calc.compute(tokens("cat", "dog", "fish"), tokens("cat", "wolf", "fish")), false));
        Map<String, Object> nullCase = levCase("testNullInputs", new String[] { "null" }, new String[] {},
                -1, true);
        cases.add(nullCase);

        Map<String, Object> json = new LinkedHashMap<>();
        json.put("java_test", "org.omegat.core.matching.LevenshteinDistanceTest#testIdenticalTokens");
        json.put("exported_by", EXPORTED_BY);
        json.put("cases", cases);
        writeJson(goldenRoot.resolve("engine/levenshtein.json"), json);
        System.out.println("wrote engine/levenshtein.json cases=" + cases.size());
    }

    private static Token[] tokens(String... words) {
        Token[] t = new Token[words.length];
        int pos = 0;
        for (int i = 0; i < words.length; i++) {
            t[i] = new Token(words[i], pos);
            pos += words[i].length() + 1;
        }
        return t;
    }

    private Map<String, Object> levCase(String method, String[] source, String[] target, int distance,
            boolean nullInputs) {
        Map<String, Object> c = new LinkedHashMap<>();
        c.put("java_test", "org.omegat.core.matching.LevenshteinDistanceTest#" + method);
        c.put("name", method);
        c.put("source", List.of(source));
        c.put("target", List.of(target));
        c.put("distance", distance);
        c.put("null_inputs", nullInputs);
        return c;
    }

    @SuppressWarnings("unchecked")
    private void exportTagValidationTests() throws Exception {
        Method ordered = TagValidation.class.getDeclaredMethod("inspectOrderedTags", List.class, List.class,
                boolean.class, ErrorReport.class);
        ordered.setAccessible(true);
        Method unordered = TagValidation.class.getDeclaredMethod("inspectUnorderedTags", List.class,
                List.class, ErrorReport.class);
        unordered.setAccessible(true);
        Constructor<ErrorReport> empty = ErrorReport.class.getDeclaredConstructor();
        empty.setAccessible(true);
        Constructor<ErrorReport> pair = ErrorReport.class.getDeclaredConstructor(String.class, String.class);
        pair.setAccessible(true);

        List<Map<String, Object>> cases = new ArrayList<>();
        Object[][] orderedCases = {
                { "no_errors", new String[] { "<g0>", "<g1>", "</g1>", "</g0>" },
                        new String[] { "<g0>", "<g1>", "</g1>", "</g0>" }, false },
                { "html_input_single", new String[] { "<s0>", "<i1>", "</s0>" },
                        new String[] { "<s0>", "<i1>", "</s0>" }, false },
                { "missing_end", new String[] { "<g0>", "<g1>", "</g1>", "</g0>" },
                        new String[] { "<g0>", "<g1>", "</g1>" }, false },
                { "duplicate_end", new String[] { "<g0>", "<g1>", "</g1>", "</g0>" },
                        new String[] { "<g0>", "<g1>", "</g1>", "</g0>", "</g0>" }, false },
                { "extraneous", new String[] { "<g0>", "<g1>", "</g1>", "</g0>" },
                        new String[] { "<g0>", "<g1>", "<x2/>", "</g1>", "</g0>" }, false },
                { "malformed", new String[] { "<g0>", "</g0>", "<g1>", "</g1>" },
                        new String[] { "<g0>", "</g0>", "</g1>", "<g1>" }, false },
                { "order", new String[] { "<g0>", "</g0>", "<g1>", "</g1>" },
                        new String[] { "<g1>", "</g1>", "<g0>", "</g0>" }, false },
                { "order_loose", new String[] { "<g0>", "</g0>", "<g1>", "</g1>" },
                        new String[] { "<g1>", "</g1>", "<g0>", "</g0>" }, true },
        };
        for (Object[] row : orderedCases) {
            ErrorReport report = empty.newInstance();
            ordered.invoke(null, tagList((String[]) row[1]), tagList((String[]) row[2]), row[3], report);
            cases.add(tagReportCase("testOrderedTagValidation", (String) row[0], (String[]) row[1],
                    (String[]) row[2], (Boolean) row[3], "ordered", report));
        }

        Object[][] unorderedCases = {
                { "no_errors", new String[] { "a", "b", "c", "d" }, new String[] { "a", "b", "c", "d" } },
                { "missing", new String[] { "a", "b", "c", "d" }, new String[] { "a", "b", "c" } },
                { "count_mismatch_ok", new String[] { "a", "b", "c", "d" },
                        new String[] { "a", "b", "c", "d", "d" } },
        };
        for (Object[] row : unorderedCases) {
            ErrorReport report = empty.newInstance();
            unordered.invoke(null, tagList((String[]) row[1]), tagList((String[]) row[2]), report);
            cases.add(tagReportCase("testUnorderedTagValidation", (String) row[0], (String[]) row[1],
                    (String[]) row[2], false, "unordered", report));
        }
        ErrorReport extra = empty.newInstance();
        ordered.invoke(null, tagList(new String[] { "a", "b", "c", "d" }),
                tagList(new String[] { "a", "b", "e", "c", "d" }), false, extra);
        cases.add(tagReportCase("testUnorderedTagValidation", "extraneous_via_ordered",
                new String[] { "a", "b", "c", "d" }, new String[] { "a", "b", "e", "c", "d" }, false,
                "ordered", extra));

        Object[][] printfCases = {
                { "ok", "Foo %s bar %d", "Foo %s bar %d" },
                { "missing", "Foo %s bar %d", "Foo %s bar" },
                { "extraneous", "Foo %s bar %d", "Foo %s bar %d baz %d" },
        };
        for (Object[] row : printfCases) {
            ErrorReport report = pair.newInstance(row[1], row[2]);
            TagValidation.inspectPrintfVariables(true, report);
            cases.add(tagReportCase("testPrintfTagValidation", (String) row[0], new String[] { (String) row[1] },
                    new String[] { (String) row[2] }, false, "printf", report));
        }

        Preferences.setPreference(Preferences.CHECK_REMOVE_PATTERN, "foo");
        ErrorReport ok = pair.newInstance("foo bar baz", "bar baz");
        TagValidation.inspectRemovePattern(ok);
        cases.add(tagReportCase("testRemovePattern", "ok", new String[] { "foo bar baz" },
                new String[] { "bar baz" }, false, "remove", ok));
        ErrorReport bad = pair.newInstance("foo bar baz", "foo bar baz");
        TagValidation.inspectRemovePattern(bad);
        cases.add(tagReportCase("testRemovePattern", "extraneous", new String[] { "foo bar baz" },
                new String[] { "foo bar baz" }, false, "remove", bad));

        Map<String, Object> json = new LinkedHashMap<>();
        json.put("java_test", "org.omegat.core.tagvalidation.TagValidationTest#testOrderedTagValidation");
        json.put("exported_by", EXPORTED_BY);
        json.put("cases", cases);
        writeJson(goldenRoot.resolve("engine/tag_validation.json"), json);
        System.out.println("wrote engine/tag_validation.json cases=" + cases.size());
    }

    private Map<String, Object> tagReportCase(String method, String name, String[] src, String[] loc,
            boolean loose, String kind, ErrorReport report) {
        Map<String, Object> c = new LinkedHashMap<>();
        c.put("java_test", "org.omegat.core.tagvalidation.TagValidationTest#" + method);
        c.put("name", name);
        c.put("kind", kind);
        c.put("loose", loose);
        c.put("src_tags", List.of(src));
        c.put("loc_tags", List.of(loc));
        c.put("src_errors", errorMap(report.srcErrors));
        c.put("trans_errors", errorMap(report.transErrors));
        return c;
    }

    private List<Map<String, Object>> errorMap(Map<Tag, ErrorReport.TagError> errors) {
        List<Map<String, Object>> out = new ArrayList<>();
        for (Map.Entry<Tag, ErrorReport.TagError> e : errors.entrySet()) {
            Map<String, Object> row = new LinkedHashMap<>();
            row.put("tag", e.getKey().tag);
            row.put("pos", e.getKey().pos);
            row.put("error", e.getValue().name());
            out.add(row);
        }
        out.sort((a, b) -> {
            int t = String.valueOf(a.get("tag")).compareTo(String.valueOf(b.get("tag")));
            if (t != 0) {
                return t;
            }
            return String.valueOf(a.get("error")).compareTo(String.valueOf(b.get("error")));
        });
        return out;
    }

    private static List<Tag> tagList(String[] array) {
        List<Tag> list = new ArrayList<>();
        for (String item : array) {
            list.add(new Tag(-1, item));
        }
        return list;
    }

    private void exportTagRepairTests() throws Exception {
        Method fixExtraneous = TagRepair.class.getDeclaredMethod("fixExtraneous", StringBuilder.class, Tag.class);
        Method fixMissing = TagRepair.class.getDeclaredMethod("fixMissing", List.class, StringBuilder.class,
                Tag.class);
        Method fixMalformed = TagRepair.class.getDeclaredMethod("fixMalformed", List.class, StringBuilder.class,
                Tag.class);
        Method fixWhitespace = TagRepair.class.getDeclaredMethod("fixWhitespace", StringBuilder.class,
                String.class);
        fixExtraneous.setAccessible(true);
        fixMissing.setAccessible(true);
        fixMalformed.setAccessible(true);
        fixWhitespace.setAccessible(true);

        List<Map<String, Object>> cases = new ArrayList<>();

        StringBuilder text = new StringBuilder("Foo bar baz bar bonkers");
        fixExtraneous.invoke(null, text, new Tag(-1, "bar"));
        fixExtraneous.invoke(null, text, new Tag(-1, "bar"));
        cases.add(repairCase("extraneous", "Foo bar baz bar bonkers", text.toString(),
                List.of("bar", "bar"), null));

        text = new StringBuilder("Foo bar {tag2}baz");
        fixMissing.invoke(null, tagList(new String[] { "{tag1}", "{tag2}" }), text, new Tag(-1, "{tag1}"));
        cases.add(repairCase("missing_before", "Foo bar {tag2}baz", text.toString(), List.of("{tag1}"),
                List.of("{tag1}", "{tag2}")));

        text = new StringBuilder("Foo bar {tag2}baz");
        fixMissing.invoke(null, tagList(new String[] { "{tag2}", "{tag1}" }), text, new Tag(-1, "{tag1}"));
        cases.add(repairCase("missing_after", "Foo bar {tag2}baz", text.toString(), List.of("{tag1}"),
                List.of("{tag2}", "{tag1}")));

        text = new StringBuilder("Foo bar baz");
        fixMissing.invoke(null, tagList(new String[] { "{tag1}" }), text, new Tag(-1, "{tag1}"));
        cases.add(repairCase("missing_no_anchor", "Foo bar baz", text.toString(), List.of("{tag1}"),
                List.of("{tag1}")));

        text = new StringBuilder("Foo bar {tag2}baz{tag1}");
        fixMalformed.invoke(null, tagList(new String[] { "{tag1}", "{tag2}" }), text, new Tag(-1, "{tag1}"));
        cases.add(repairCase("malformed", "Foo bar {tag2}baz{tag1}", text.toString(), List.of("{tag1}"),
                List.of("{tag1}", "{tag2}")));

        text = new StringBuilder("\nFoo\n");
        fixWhitespace.invoke(null, text, "Foo");
        cases.add(repairCase("whitespace_strip", "\nFoo\n", text.toString(), List.of(), null));

        text = new StringBuilder("Foo");
        fixWhitespace.invoke(null, text, "\nFoo\n");
        cases.add(repairCase("whitespace_add", "Foo", text.toString(), List.of(), null));

        Map<String, Object> json = new LinkedHashMap<>();
        json.put("java_test", "org.omegat.core.tagvalidation.TagRepairTest#testRepairTags");
        json.put("exported_by", EXPORTED_BY);
        json.put("cases", cases);
        writeJson(goldenRoot.resolve("engine/tag_repair.json"), json);
        System.out.println("wrote engine/tag_repair.json cases=" + cases.size());
    }

    private Map<String, Object> repairCase(String name, String input, String output, List<String> tags,
            List<String> sourceOrder) {
        Map<String, Object> c = new LinkedHashMap<>();
        c.put("java_test", "org.omegat.core.tagvalidation.TagRepairTest#testRepairTags");
        c.put("name", name);
        c.put("input", input);
        c.put("output", output);
        c.put("tags", tags);
        if (sourceOrder != null) {
            c.put("source_order", sourceOrder);
        }
        return c;
    }

    private void exportTmxWriterTests() throws Exception {
        Path tmp = Files.createTempDirectory("omegat-tmx-export");
        File outFile = tmp.resolve("out.tmx").toFile();
        List<Map<String, Object>> cases = new ArrayList<>();

        String invalid = "" + (char) 0x00 + (char) 0x01 + (char) 0x02 + (char) 0x18 + (char) 0x19
                + (char) 0xD8FF + (char) 0xFFFE + (char) 0x12FFFF;
        try (TMXWriter2 wr = new TMXWriter2(outFile, new Language("en-US"), new Language("be-BY"), false,
                true, false)) {
            wr.writeEntry(invalid, "test", RealProjectTest.createEmptyTMXEntry(), null);
        }
        List<String> sources = loadTmxSources(outFile, true, false);
        Map<String, Object> inv = new LinkedHashMap<>();
        inv.put("java_test", "org.omegat.util.TMXWriterTest#testWriteInvalidChars");
        inv.put("name", "testWriteInvalidChars");
        inv.put("sanitized_source", StringUtil.removeXMLInvalidChars(invalid));
        inv.put("read_sources", sources);
        cases.add(inv);

        try (TMXWriter2 wr = new TMXWriter2(outFile, new Language("en-US"), new Language("be-BY"), false,
                true, false)) {
            wr.writeEntry("source", "target", RealProjectTest.createEmptyTMXEntry(), null);
            wr.writeEntry("1<a1/>2", "zz", RealProjectTest.createEmptyTMXEntry(), null);
            wr.writeEntry("3<a1>4</a1>5", "zz", RealProjectTest.createEmptyTMXEntry(), null);
            wr.writeEntry("6<a1>7", "zz", RealProjectTest.createEmptyTMXEntry(), null);
        }
        String written = Files.readString(outFile.toPath());
        Map<String, Object> level2 = new LinkedHashMap<>();
        level2.put("java_test", "org.omegat.util.TMXWriterTest#testLevel2write");
        level2.put("name", "testLevel2write");
        level2.put("sources", List.of("source", "1<a1/>2", "3<a1>4</a1>5", "6<a1>7"));
        level2.put("targets", List.of("target", "zz", "zz", "zz"));
        level2.put("xml", written);
        level2.put("level2_fragments", List.of(
                writeLevelTwoFragment("source"),
                writeLevelTwoFragment("1<a1/>2"),
                writeLevelTwoFragment("3<a1>4</a1>5"),
                writeLevelTwoFragment("6<a1>7")));
        cases.add(level2);

        File fixture = javaRoot.resolve("src/test/resources/data/tmx/test-save-tmx14.tmx").toFile();
        Object[][] reads = {
                { "omegat", true, false },
                { "ext_l1", false, false },
                { "ext_l2", true, false },
                { "ext_l2_slash", true, true },
        };
        for (Object[] row : reads) {
            File patched = tmp.resolve(row[0] + ".tmx").toFile();
            String tool = "omegat".equals(row[0]) ? "OmegaT" : "ext";
            patchCreationTool(fixture, tool, patched);
            List<String> read = loadTmxSources(patched, (Boolean) row[1], (Boolean) row[2]);
            Map<String, Object> rc = new LinkedHashMap<>();
            rc.put("java_test", "org.omegat.util.TMXWriterTest#testLevel2reads");
            rc.put("name", "testLevel2reads");
            rc.put("mode", row[0]);
            rc.put("ext_level2", row[1]);
            rc.put("use_slash", row[2]);
            rc.put("sources", read);
            cases.add(rc);
        }

        try (TMXWriter2 wr = new TMXWriter2(outFile, new Language("en-US"), new Language("be-BY"), false,
                true, false)) {
            wr.writeEntry("source", "tar\nget", RealProjectTest.createEmptyTMXEntry(), null);
        }
        String eolXml = Files.readString(outFile.toPath());
        List<String> trs = loadTmxTranslations(outFile, true, false);
        Map<String, Object> eol = new LinkedHashMap<>();
        eol.put("java_test", "org.omegat.util.TMXWriterTest#testEOLwrite");
        eol.put("name", "testEOLwrite");
        eol.put("contains_platform_eol", eolXml.contains("tar" + System.lineSeparator() + "get"));
        eol.put("read_translation", trs.isEmpty() ? "" : trs.get(0));
        cases.add(eol);

        Map<String, Object> json = new LinkedHashMap<>();
        json.put("java_test", "org.omegat.util.TMXWriterTest#testLevel2write");
        json.put("exported_by", EXPORTED_BY);
        json.put("cases", cases);
        writeJson(goldenRoot.resolve("engine/tmx_writer.json"), json);
        System.out.println("wrote engine/tmx_writer.json cases=" + cases.size());
    }

    private static String writeLevelTwoFragment(String segment) {
        java.util.regex.Pattern tags = java.util.regex.Pattern.compile("<(/?)([\\S&&[^/\\d]]+)(\\d+)(/?)>");
        StringBuilder out = new StringBuilder();
        java.util.regex.Matcher m = tags.matcher(segment);
        int pos = 0;
        while (m.find(pos)) {
            out.append(segment, pos, m.start());
            pos = m.end();
            boolean end = !m.group(1).isEmpty();
            boolean single = !m.group(4).isEmpty();
            String name = m.group(2);
            String num = m.group(3);
            if (single) {
                out.append("<ph x=\"").append(num).append("\">").append(m.group()).append("</ph>");
            } else if (end) {
                String start = "<" + name + num + ">";
                if (segment.contains(start)) {
                    out.append("<ept i=\"").append(num).append("\">").append(m.group()).append("</ept>");
                } else {
                    out.append("<it pos=\"end\" x=\"").append(num).append("\">").append(m.group())
                            .append("</it>");
                }
            } else {
                String close = "</" + name + num + ">";
                if (segment.contains(close)) {
                    out.append("<bpt i=\"").append(num).append("\" x=\"").append(num).append("\">")
                            .append(m.group()).append("</bpt>");
                } else {
                    out.append("<it pos=\"begin\" x=\"").append(num).append("\">").append(m.group())
                            .append("</it>");
                }
            }
        }
        out.append(segment.substring(pos));
        return out.toString();
    }

    private void patchCreationTool(File in, String tool, File out) throws Exception {
        String raw = Files.readString(in.toPath());
        raw = raw.replaceFirst("creationtool=\"[^\"]*\"", "creationtool=\"" + tool + "\"");
        Files.writeString(out.toPath(), raw);
    }

    private List<String> loadTmxSources(File file, boolean extLevel2, boolean useSlash) throws Exception {
        List<String> sources = new ArrayList<>();
        TMXReader2 reader = new TMXReader2.Builder().setExtTmxLevel2(extLevel2).setUseSlash(useSlash)
                .setSegmentingEnabled(false).setNeedValidate(true).build();
        reader.readTMX(file, new Language("en-US"), new Language("be-BY"),
                (tu, tuvSource, tuvTarget, isParagraphSegtype) -> {
                    sources.add(tuvSource.text);
                    return true;
                });
        return sources;
    }

    private List<String> loadTmxTranslations(File file, boolean extLevel2, boolean useSlash) throws Exception {
        List<String> trs = new ArrayList<>();
        TMXReader2 reader = new TMXReader2.Builder().setExtTmxLevel2(extLevel2).setUseSlash(useSlash)
                .setSegmentingEnabled(false).setNeedValidate(true).build();
        reader.readTMX(file, new Language("en-US"), new Language("be-BY"),
                (tu, tuvSource, tuvTarget, isParagraphSegtype) -> {
                    trs.add(tuvTarget.text);
                    return true;
                });
        return trs;
    }

    @SuppressWarnings("unchecked")
    private void exportFindMatchesTests() throws Exception {
        Path tmpDir = Files.createTempDirectory("omegat-find-matches");
        Preferences.setPreference(Preferences.EXT_TMX_SHOW_LEVEL2, false);
        Preferences.setPreference(Preferences.EXT_TMX_KEEP_FOREIGN_MATCH, true);
        Method search = FindMatches.class.getDeclaredMethod("search", String.class, boolean.class,
                IStopped.class, boolean.class);
        search.setAccessible(true);
        IStopped never = () -> false;
        Segmenter segmenter = new Segmenter(SRX.getDefault());
        List<Map<String, Object>> cases = new ArrayList<>();

        File tmxMatch = javaRoot.resolve("src/test/resources/data/tmx/test-match-stat-en-ca.tmx").toFile();
        File tmxEnUsSr = javaRoot.resolve("src/test/resources/data/tmx/en-US_sr.tmx").toFile();
        File tmxEnUsGb = javaRoot.resolve("src/test/resources/data/tmx/en-US_en-GB_fr_sr.tmx").toFile();
        File tmxSeg = javaRoot.resolve("src/test/resources/data/tmx/penalty-010/segment_1.tmx").toFile();
        File tmxSeg2 = javaRoot.resolve("src/test/resources/data/tmx/segment_2.tmx").toFile();
        File tmxMulti = javaRoot.resolve("src/test/resources/data/tmx/test-multiple-entries.tmx").toFile();

        String badge = "This badge is granted when you’ve invited 5 people who subsequently spent enough "
                + "time on the site to become full members. " + "Wow! "
                + "Thanks for expanding the diversity of our community with new members!";

        ProjectProperties prop = new ProjectProperties(tmpDir.toFile());
        prop.setSourceLanguage("en");
        prop.setTargetLanguage("ca");
        prop.setSupportDefaultTranslations(true);
        prop.setSentenceSegmentingEnabled(false);
        FindMatchesTest.TestProject project = new FindMatchesTest.TestProject(prop, tmxMatch, null,
                new LuceneEnglishTokenizer(), new DefaultTokenizer(), segmenter);
        FindMatches finder = new FindMatches(project, segmenter, OConsts.MAX_NEAR_STRINGS, false, 30);
        List<NearString> result = (List<NearString>) search.invoke(finder, badge, true, never, false);
        cases.add(findCase("testSegmented", "without_separate", badge, result));
        finder = new FindMatches(project, segmenter, OConsts.MAX_NEAR_STRINGS, false, 30);
        result = (List<NearString>) search.invoke(finder, badge, false, never, true);
        cases.add(findCase("testSegmented", "with_separate", badge, result));

        prop = new ProjectProperties(tmpDir.toFile());
        prop.setSourceLanguage("en");
        prop.setTargetLanguage("cnr");
        prop.setSupportDefaultTranslations(true);
        prop.setSentenceSegmentingEnabled(false);
        project = new FindMatchesTest.TestProject(prop, null, tmxEnUsSr, new LuceneEnglishTokenizer(),
                new DefaultTokenizer(), segmenter);
        finder = new FindMatches(project, segmenter, OConsts.MAX_NEAR_STRINGS, false, 30);
        result = (List<NearString>) search.invoke(finder, "XXX", false, never, true);
        cases.add(findCase("testSearchRFE1578", "rfe1578", "XXX", result));

        project = new FindMatchesTest.TestProject(prop, null, tmxEnUsGb, new LuceneEnglishTokenizer(),
                new DefaultTokenizer(), segmenter);
        finder = new FindMatches(project, segmenter, OConsts.MAX_NEAR_STRINGS, false, 30);
        result = (List<NearString>) search.invoke(finder, "XXX", false, never, true);
        cases.add(findCase("testSearchRFE1578_2", "rfe1578_2", "XXX", result));

        prop = new ProjectProperties(tmpDir.toFile());
        prop.setSourceLanguage("ja");
        prop.setTargetLanguage("fr");
        prop.setSupportDefaultTranslations(true);
        prop.setSentenceSegmentingEnabled(false);
        Segmenter srxDefault = new Segmenter(SRX.getDefault());
        project = new FindMatchesTest.TestProject(prop, null, tmxSeg, new LuceneCJKTokenizer(),
                new LuceneFrenchTokenizer(), srxDefault);
        String srcJa = project.getAllEntries().get(1).getSrcText();
        finder = new FindMatches(project, srxDefault, OConsts.MAX_NEAR_STRINGS, false, 30);
        result = (List<NearString>) search.invoke(finder, srcJa, false, never, true);
        cases.add(findCase("testSearchBUGS1251", "bugs1251", srcJa, result));

        project = new FindMatchesTest.TestProject(prop, null, tmxSeg2, new LuceneCJKTokenizer(),
                new LuceneFrenchTokenizer(), srxDefault);
        srcJa = project.getAllEntries().get(1).getSrcText();
        finder = new FindMatches(project, srxDefault, OConsts.MAX_NEAR_STRINGS, false, 30);
        result = (List<NearString>) search.invoke(finder, srcJa, false, never, true);
        cases.add(findCase("testSearchForeign", "foreign", srcJa, result));

        prop = new ProjectProperties(tmpDir.toFile());
        prop.setSourceLanguage("en");
        prop.setTargetLanguage("fr");
        prop.setSupportDefaultTranslations(true);
        prop.setSentenceSegmentingEnabled(false);
        project = new FindMatchesTest.TestProject(prop, null, tmxMatch, new LuceneEnglishTokenizer(),
                new DefaultTokenizer(), segmenter);
        finder = new FindMatches(project, segmenter, OConsts.MAX_NEAR_STRINGS, false, 30);
        result = (List<NearString>) search.invoke(finder, badge, false, never, true);
        cases.add(findCase("testSearchForeignSegmented", "foreign_segmented", badge, result));

        prop = new ProjectProperties(tmpDir.toFile());
        prop.setSourceLanguage("en-US");
        prop.setTargetLanguage("co");
        prop.setSupportDefaultTranslations(true);
        prop.setSentenceSegmentingEnabled(true);
        project = new FindMatchesTest.TestProject(prop, tmxMulti, null, new LuceneEnglishTokenizer(),
                new DefaultTokenizer(), segmenter);
        finder = new FindMatches(project, segmenter, OConsts.MAX_NEAR_STRINGS, true, 85);
        result = (List<NearString>) search.invoke(finder, "Other", false, never, false);
        cases.add(findCase("testSearchMulti", "multi", "Other", result));

        Map<String, Object> json = new LinkedHashMap<>();
        json.put("java_test", "org.omegat.core.statistics.FindMatchesTest#testSegmented");
        json.put("exported_by", EXPORTED_BY);
        json.put("cases", cases);
        writeJson(goldenRoot.resolve("engine/find_matches.json"), json);
        System.out.println("wrote engine/find_matches.json cases=" + cases.size());
    }

    private Map<String, Object> findCase(String method, String name, String query, List<NearString> result) {
        Map<String, Object> c = new LinkedHashMap<>();
        c.put("java_test", "org.omegat.core.statistics.FindMatchesTest#" + method);
        c.put("name", name);
        c.put("query", query);
        List<Map<String, Object>> hits = new ArrayList<>();
        for (NearString n : result) {
            Map<String, Object> h = new LinkedHashMap<>();
            h.put("source", n.source);
            h.put("translation", n.translation);
            h.put("score", n.scores[0].score);
            h.put("score_no_stem", n.scores[0].scoreNoStem);
            h.put("adjusted_score", n.scores[0].adjustedScore);
            h.put("penalty", n.scores[0].penalty);
            h.put("comes_from", n.comesFrom == null ? "" : n.comesFrom.name());
            h.put("proj", n.projs != null && n.projs.length > 0 ? n.projs[0] : "");
            if (n.key != null) {
                h.put("key_file", n.key.file);
            }
            hits.add(h);
        }
        c.put("hits", hits);
        return c;
    }

    private void exportCalcMatchStatisticsTests() throws Exception {
        Path tmpDir = Files.createTempDirectory("omegat-calc-stats");
        TestingProject project = new TestingProject(tmpDir);
        // Statistics.buildProjectStats constructs StatProjectProperties from
        // Core.getProject(), not the TestingProject passed to the calculator.
        Core.setProject(project);
        Segmenter segmenter = new Segmenter(SRX.getDefault());
        List<Map<String, Object>> cases = new ArrayList<>();

        TestingStatsConsumer standardConsumer = new TestingStatsConsumer();
        cases.add(runStatsCase("testStatistics", new CalcStandardStatistics(project, standardConsumer),
                standardConsumer));
        TestingStatsConsumer perFileConsumer = new TestingStatsConsumer();
        cases.add(runStatsCase("testPerFileCalcMatchStatistics",
                new CalcPerFileMatchStatistics(project, segmenter, perFileConsumer), perFileConsumer));
        TestingStatsConsumer matchConsumer = new TestingStatsConsumer();
        cases.add(runStatsCase("testCalcMatchStatics",
                new CalcMatchStatistics(project, segmenter, matchConsumer), matchConsumer));

        List<String> sources = new ArrayList<>();
        List<Map<String, Object>> perSource = new ArrayList<>();
        FindMatches finder = new FindMatches(project, segmenter, OConsts.MAX_NEAR_STRINGS, false, -1);
        ITokenizer tok = project.getSourceTokenizer();
        LevenshteinDistance distance = new LevenshteinDistance();
        java.util.Set<String> seenSrc = new java.util.HashSet<>();
        for (org.omegat.core.data.SourceTextEntry ste : project.getAllEntries()) {
            sources.add(ste.getSrcText());
            StatCount sc = new StatCount(ste);
            Map<String, Object> row = new LinkedHashMap<>();
            row.put("source", ste.getSrcText());
            row.put("words", sc.words);
            row.put("chars_nosp", sc.charsWithoutSpaces);
            row.put("chars", sc.charsWithSpaces);
            row.put("first", seenSrc.add(ste.getSrcText()));
            String srcTrans = ste.getSourceTranslation();
            row.put("source_translation", srcTrans == null ? "" : srcTrans);
            row.put("source_translation_fuzzy", ste.isSourceTranslationFuzzy());
            List<String> pps = new ArrayList<>();
            if (ste.getProtectedParts() != null) {
                for (ProtectedPart pp : ste.getProtectedParts()) {
                    pps.add(pp.getTextInSourceSegment());
                }
            }
            row.put("protected", pps);
            String srcNoXml = ste.getSrcText();
            if (ste.getProtectedParts() != null) {
                for (ProtectedPart pp : ste.getProtectedParts()) {
                    srcNoXml = srcNoXml.replace(pp.getTextInSourceSegment(),
                            pp.getReplacementMatchCalculation());
                }
            }
            List<NearString> nears = finder.search(srcNoXml, false, () -> false);
            Token[] strTokens = tok.tokenizeVerbatim(ste.getSrcText().toLowerCase(Locale.ENGLISH));
            int max = 0;
            String bestFrom = "";
            boolean bestFuzzy = false;
            for (NearString near : nears) {
                Token[] cand = tok.tokenizeVerbatim(near.source.toLowerCase(Locale.ENGLISH));
                int sim = FuzzyMatcher.calcSimilarity(distance, strTokens, cand);
                if (near.fuzzyMark) {
                    sim -= 40;
                }
                if (sim > max) {
                    max = sim;
                    bestFrom = near.comesFrom == null ? "" : near.comesFrom.toString();
                    bestFuzzy = near.fuzzyMark;
                }
                if (sim >= 95) {
                    break;
                }
            }
            row.put("percent", max);
            row.put("best_from", bestFrom);
            row.put("best_fuzzy", bestFuzzy);
            perSource.add(row);
        }

        Map<String, Object> json = new LinkedHashMap<>();
        json.put("java_test", "org.omegat.core.statistics.CalcMatchStatisticsTest#testCalcMatchStatics");
        json.put("exported_by", EXPORTED_BY);
        json.put("source_lang", "en");
        json.put("target_lang", "ca");
        json.put("tokenizer", "org.omegat.tokenizer.LuceneEnglishTokenizer");
        json.put("po", "src/test/resources/data/filters/po/file-POFilter-match-stat-en-ca.po");
        json.put("tmx", "src/test/resources/data/tmx/test-match-stat-en-ca.tmx");
        json.put("sources", sources);
        json.put("per_source", perSource);
        List<Map<String, Object>> verbatim = new ArrayList<>();
        String[] samples = {
            "can't have emoji",
            "you've invited 5 people",
            "Sorry, this account confirmation link is no longer valid. Perhaps your account is already active?",
            "Sorry, this account confirmation link is no longer valid. Perhaps your account is\n        already",
            "You cannot use the same bucket for 's3_upload_bucket' and 's3_backup_bucket'. Choose a different bucket or use a different path for each bucket.",
            "This badge is granted the first time you flag a post. Flagging is how we all help keep this a nice place for everyone. If you notice any posts that require moderator attention for any reason please don’t hesitate to flag. If you see a problem, :flag_black: flag it!\n",
            "<a href=\"https://blog.discourse.org/2018/06/understanding-discourse-trust-levels/\">Granted</a> invitations, group messaging, more likes"
        };
        org.omegat.tokenizer.DefaultTokenizer defTok = new org.omegat.tokenizer.DefaultTokenizer();
        for (String sample : samples) {
            Map<String, Object> v = new LinkedHashMap<>();
            v.put("text", sample);
            v.put("tokens", java.util.Arrays.asList(tok.tokenizeVerbatimToStrings(sample.toLowerCase(Locale.ENGLISH))));
            v.put("default_tokens", java.util.Arrays.asList(defTok.tokenizeVerbatimToStrings(sample.toLowerCase(Locale.ENGLISH))));
            verbatim.add(v);
        }
        json.put("verbatim_samples", verbatim);
        json.put("cases", cases);
        writeJson(goldenRoot.resolve("engine/calc_match_statistics.json"), json);
        System.out.println("wrote engine/calc_match_statistics.json cases=" + cases.size()
                + " sources=" + sources.size());
    }

    private Map<String, Object> runStatsCase(String method, ICalcStatistics calc, TestingStatsConsumer consumer)
            throws Exception {
        // The consumer passed to the constructor is the one that receives tables.
        // Recreate so the same instance is used.
        CancellationToken token = new CancellationToken();
        calc.run(token);
        Completion completion = consumer.completion().join();
        Map<String, Object> c = new LinkedHashMap<>();
        c.put("java_test", "org.omegat.core.statistics.CalcMatchStatisticsTest#" + method);
        c.put("name", method);
        c.put("success", completion.isSuccess());
        List<String[][]> tables = consumer.getTable();
        List<List<List<String>>> dumped = new ArrayList<>();
        for (String[][] table : tables) {
            List<List<String>> rows = new ArrayList<>();
            if (table != null) {
                for (String[] row : table) {
                    rows.add(List.of(row));
                }
            }
            dumped.add(rows);
        }
        c.put("tables", dumped);
        return c;
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
        return tokenWordsCase(tokenizer, lang, input, mode, "org.omegat.tokenizer.DefaultTokenizer#tokenizeWords");
    }

    private void bindTokenizer(ITokenizer tokenizer, String lang) {
        Core.setProject(new NotLoadedProject() {
            @Override
            public ITokenizer getSourceTokenizer() {
                return tokenizer;
            }

            @Override
            public ITokenizer getTargetTokenizer() {
                return tokenizer;
            }

            @Override
            public ProjectProperties getProjectProperties() {
                return new ProjectProperties() {
                    @Override
                    public Language getSourceLanguage() {
                        return new Language(lang);
                    }

                    @Override
                    public Language getTargetLanguage() {
                        return new Language(lang);
                    }
                };
            }
        });
    }

    private Map<String, Object> tokenWordsCase(ITokenizer tokenizer, String lang, String input,
            ITokenizer.StemmingMode mode, String javaTest) {
        Token[] tokens = tokenizer.tokenizeWords(input, mode);
        String[] texts = Token.getTextsFromString(tokens, input);
        String[] words = tokenizer.tokenizeWordsToStrings(input, mode);
        Map<String, Object> c = new LinkedHashMap<>();
        c.put("lang", lang);
        c.put("input", input);
        c.put("tokenizer", tokenizer.getClass().getName());
        c.put("stemming", mode.name());
        c.put("java_test", javaTest);
        c.put("tokens", List.of(texts));
        c.put("words", List.of(words));
        return c;
    }

    /**
     * Honesty surfaces: dialect tag sets, IEditor / menu / prefs inventories,
     * every *FilterTest#test* listing, and HTMLFilter2Test-per-method goldens.
     */
    private void exportHonesty() throws Exception {
        exportDialectTags();
        exportIEditorMethods();
        exportMenuActions();
        exportPreferenceKeys();
        exportFilterTestInventory();
        exportHtmlFilter2AllTests();
        exportHtmlOptionKeys();
        System.out.println("wrote honesty surfaces (dialect/IEditor/menu/prefs/filter_tests/html)");
    }

    private Object[][] languageTokenizerFixtures() {
        String jaWiki = "\u6211\u3005\u306E\u3059\u3079\u3066\u306F\u540C\u3058\uFF11\u500B\u306E\u60D1"
                + "\u661F\uFF08\u82F1\uFF1A\u300Ca planet\u300D\uFF09\u306B\u4F4F\u307F\u3001\u6211"
                + "\u3005\u306E\u3059\u3079\u3066\u306F\u305D\u306E\u751F\u7269\u570F\u306E1.5\u90E8"
                + "\u3067\u3042\u308B<x0/>\u3002";
        String zhWiki = "\u6F22\u8A9E\u7684\u6587\u5B57\u7CFB\u7D71\u2014\u2014\u6F22\u5B57\u662F"
                + "\u4E00\u7A2E\u610F\u97F3\u8A9E\u8A00\uFF0C\u8868\u610F\u7684\u540C\u6642\u4E5F"
                + "\u5177\u4E00\u5B9A\u7684\u8868\u97F3\u529F\u80FD\u3002";
        String trWiki = "\u201C\u0130stanbul a\u011Fz\u0131\u201D, T\u00FCrkiye T\u00FCrk\u00E7esi"
                + "yaz\u0131 dilinin kayna\u011F\u0131 olarak kabul edilir; yaz\u0131 dili bu"
                + "a\u011F\u0131z temelinde olu\u015Fmu\u015Ftur.";
        String enOrig = "The quick, brown <x0/> jumped over 1 \"lazy\" dog.";
        return new Object[][] {
                { new LuceneArabicTokenizer(), "ar",
                        "\u0627\u0644\u0644\u063A\u0629 \u0627\u0644\u0639\u0631\u0628\u064A\u0629 \u0647\u064A \u0623\u0643\u062B\u0631 \u0627\u0644\u0644\u063A\u0627\u062A \u0627\u0644\u0633\u0627\u0645\u064A\u0629 \u062A\u062D\u062F\u062B\u0627\u064B",
                        "org.omegat.tokenizer.LuceneArabicTokenizer#tokenizeWordsToStrings" },
                { new LuceneArmenianTokenizer(), "hy",
                        "\u0540\u0561\u0575\u0565\u0580\u0565\u0576\u0568 \u0570\u0561\u0575 \u056A\u0578\u0572\u0578\u057E\u0580\u0564\u056B \u0574\u0561\u0575\u0580\u0565\u0576\u056B \u056C\u0565\u0566\u0578\u0582\u0576 \u0567",
                        "org.omegat.tokenizer.LuceneArmenianTokenizer#tokenizeWordsToStrings" },
                { new LuceneBasqueTokenizer(), "eu", "Euskara Euskal Herriko hizkuntza da eta euskaldunek hitz egiten dute.",
                        "org.omegat.tokenizer.LuceneBasqueTokenizer#tokenizeWordsToStrings" },
                { new LuceneBrazilianTokenizer(), "pt-br",
                        "O portugu\u00eas brasileiro \u00e9 falado no Brasil por milh\u00f5es de pessoas.",
                        "org.omegat.tokenizer.LuceneBrazilianTokenizer#tokenizeWordsToStrings" },
                { new LuceneBulgarianTokenizer(), "bg",
                        "\u0411\u044A\u043B\u0433\u0430\u0440\u0441\u043A\u0438\u044F\u0442 \u0435\u0437\u0438\u043A \u0435 \u044E\u0436\u043D\u043E\u0441\u043B\u0430\u0432\u044F\u043D\u0441\u043A\u0438 \u0435\u0437\u0438\u043A.",
                        "org.omegat.tokenizer.LuceneBulgarianTokenizer#tokenizeWordsToStrings" },
                { new LuceneCatalanTokenizer(), "ca", "El catal\u00e0 \u00e9s una llengua rom\u00e0nica parlada a Catalunya.",
                        "org.omegat.tokenizer.LuceneCatalanTokenizer#tokenizeWordsToStrings" },
                { new LuceneCJKTokenizer(), "zh", "\u6C49\u5B57\u8BCD",
                        "org.omegat.tokenizer.LuceneCJKTokenizer#tokenizeWordsToStrings" },
                { new LuceneCzechTokenizer(), "cs", "\u010ce\u0161tina je z\u00e1padoslovansk\u00fd jazyk.",
                        "org.omegat.tokenizer.LuceneCzechTokenizer#tokenizeWordsToStrings" },
                { new LuceneDanishTokenizer(), "da", "Dansk er et nordisk sprog talt i Danmark.",
                        "org.omegat.tokenizer.LuceneDanishTokenizer#tokenizeWordsToStrings" },
                { new LuceneDutchTokenizer(), "nl", "Nederlands is een West-Germaanse taal.",
                        "org.omegat.tokenizer.LuceneDutchTokenizer#tokenizeWordsToStrings" },
                { new LuceneEnglishTokenizer(), "en", enOrig,
                        "org.omegat.tokenizer.TokenizerTest#testEnglish" },
                { new LuceneFinnishTokenizer(), "fi", "Suomi on uralilainen kieli jota puhutaan Suomessa.",
                        "org.omegat.tokenizer.LuceneFinnishTokenizer#tokenizeWordsToStrings" },
                { new LuceneFrenchTokenizer(), "fr", "Le fran\u00e7ais est une langue romane parl\u00e9e en France.",
                        "org.omegat.tokenizer.LuceneFrenchTokenizer#tokenizeWordsToStrings" },
                { new LuceneGalicianTokenizer(), "gl", "O galego \u00e9 unha lingua rom\u00e1nica.",
                        "org.omegat.tokenizer.LuceneGalicianTokenizer#tokenizeWordsToStrings" },
                { new LuceneGermanTokenizer(), "de", "Die pr\u00e4sentierte L\u00f6sung funktioniert in laufenden Tests.",
                        "org.omegat.tokenizer.TokenizerTest#testGerman" },
                { new LuceneGreekTokenizer(), "el",
                        "\u0397 \u03B5\u03BB\u03BB\u03B7\u03BD\u03B9\u03BA\u03AE \u03B3\u03BB\u03CE\u03C3\u03C3\u03B1 \u03B5\u03AF\u03BD\u03B1\u03B9 \u03B9\u03BD\u03B4\u03BF\u03B5\u03C5\u03C1\u03C9\u03C0\u03B1\u03CA\u03BA\u03AE.",
                        "org.omegat.tokenizer.LuceneGreekTokenizer#tokenizeWordsToStrings" },
                { new LuceneHindiTokenizer(), "hi",
                        "\u0939\u093F\u0928\u094D\u0926\u0940 \u092D\u093E\u0930\u0924 \u0915\u0940 \u090F\u0915 \u092A\u094D\u0930\u092E\u0941\u0916 \u092D\u093E\u0937\u093E \u0939\u0948",
                        "org.omegat.tokenizer.LuceneHindiTokenizer#tokenizeWordsToStrings" },
                { new LuceneHungarianTokenizer(), "hu", "A magyar nyelv ur\u00e1li nyelv amelyet Magyarorsz\u00e1gon besz\u00e9lnek.",
                        "org.omegat.tokenizer.LuceneHungarianTokenizer#tokenizeWordsToStrings" },
                { new LuceneIndonesianTokenizer(), "id",
                        "Bahasa Indonesia adalah bahasa resmi Republik Indonesia.",
                        "org.omegat.tokenizer.LuceneIndonesianTokenizer#tokenizeWordsToStrings" },
                { new LuceneIrishTokenizer(), "ga", "Is \u00ed an Ghaeilge teanga na h\u00c9ireann.",
                        "org.omegat.tokenizer.LuceneIrishTokenizer#tokenizeWordsToStrings" },
                { new LuceneItalianTokenizer(), "it", "I paesi europei sono molti e parlano lingue diverse.",
                        "org.omegat.tokenizer.TokenizerTest#testItalian" },
                { new LuceneJapaneseTokenizer(), "ja", jaWiki,
                        "org.omegat.tokenizer.TokenizerTest#testJapanese" },
                { new LuceneLatvianTokenizer(), "lv", "Latvie\u0161u valoda ir Baltijas valoda.",
                        "org.omegat.tokenizer.LuceneLatvianTokenizer#tokenizeWordsToStrings" },
                { new LuceneNorwegianTokenizer(), "nb", "Norsk bokm\u00e5l er et nordisk spr\u00e5k.",
                        "org.omegat.tokenizer.LuceneNorwegianTokenizer#tokenizeWordsToStrings" },
                { new LucenePersianTokenizer(), "fa",
                        "\u0641\u0627\u0631\u0633\u06CC \u0632\u0628\u0627\u0646 \u0631\u0633\u0645\u06CC \u0627\u06CC\u0631\u0627\u0646 \u0627\u0633\u062A",
                        "org.omegat.tokenizer.LucenePersianTokenizer#tokenizeWordsToStrings" },
                { new LucenePolishTokenizer(), "pl", "J\u0119zyk polski jest j\u0119zykiem s\u0142owia\u0144skim.",
                        "org.omegat.tokenizer.LucenePolishTokenizer#tokenizeWordsToStrings" },
                { new LucenePortugueseTokenizer(), "pt", "O portugu\u00eas \u00e9 uma l\u00edngua rom\u00e2nica.",
                        "org.omegat.tokenizer.LucenePortugueseTokenizer#tokenizeWordsToStrings" },
                { new LuceneRomanianTokenizer(), "ro", "Limba rom\u00e2n\u0103 este o limb\u0103 romanic\u0103.",
                        "org.omegat.tokenizer.LuceneRomanianTokenizer#tokenizeWordsToStrings" },
                { new LuceneRussianTokenizer(), "ru",
                        "\u0420\u0443\u0441\u0441\u043A\u0438\u0439 \u044F\u0437\u044B\u043A \u044F\u0432\u043B\u044F\u0435\u0442\u0441\u044F \u0441\u043B\u0430\u0432\u044F\u043D\u0441\u043A\u0438\u043C \u044F\u0437\u044B\u043A\u043E\u043C.",
                        "org.omegat.tokenizer.LuceneRussianTokenizer#tokenizeWordsToStrings" },
                { new LuceneSmartChineseTokenizer(), "zh", zhWiki,
                        "org.omegat.tokenizer.TokenizerTest#testChinese" },
                { new LuceneSpanishTokenizer(), "es", "El espa\u00f1ol es una lengua romance hablada en Espa\u00f1a.",
                        "org.omegat.tokenizer.LuceneSpanishTokenizer#tokenizeWordsToStrings" },
                { new LuceneSwedishTokenizer(), "sv", "Svenska \u00e4r ett nordiskt spr\u00e5k.",
                        "org.omegat.tokenizer.LuceneSwedishTokenizer#tokenizeWordsToStrings" },
                { new LuceneThaiTokenizer(), "th",
                        "\u0E20\u0E32\u0E29\u0E32\u0E44\u0E17\u0E22\u0E40\u0E1B\u0E47\u0E19\u0E20\u0E32\u0E29\u0E32\u0E23\u0E32\u0E0A\u0E01\u0E32\u0E23\u0E02\u0E2D\u0E07\u0E1B\u0E23\u0E30\u0E40\u0E17\u0E28\u0E44\u0E17\u0E22",
                        "org.omegat.tokenizer.LuceneThaiTokenizer#tokenizeWordsToStrings" },
                { new LuceneTurkishTokenizer(), "tr", trWiki,
                        "org.omegat.tokenizer.TokenizerTest#testTurkish" },
        };
    }

    private void exportDialectTags() throws Exception {
        Map<String, DefaultXMLDialect> dialects = new LinkedHashMap<>();
        dialects.put("android", new AndroidDialect());
        dialects.put("camtasia", new CamtasiaWindowsDialect());
        dialects.put("docbook", new DocBookDialect());
        dialects.put("flash", new FlashDialect());
        dialects.put("helpandmanual", new HelpAndManualDialect());
        dialects.put("infix", new InfixDialect());
        dialects.put("l10nmgr", new L10nmgrDialect());
        OpenDocDialect opendoc = new OpenDocDialect();
        opendoc.defineDialect(new OpenDocOptions(Collections.emptyMap()));
        dialects.put("opendoc", opendoc);
        OpenXMLDialect openxml = new OpenXMLDialect();
        openxml.defineDialect(new OpenXMLOptions(Collections.emptyMap()));
        dialects.put("openxml", openxml);
        dialects.put("propxml", new PropertiesDialect());
        dialects.put("relaxng", new RelaxNGDialect());
        dialects.put("resx", new ResXDialect());
        dialects.put("schematron", new SchematronDialect());
        dialects.put("scribus", new ScribusDialect());
        dialects.put("svg", new SvgDialect());
        dialects.put("txml", new TXMLDialect());
        dialects.put("typo3", new Typo3Dialect());
        dialects.put("visio", new VisioDialect());
        dialects.put("wix", new WiXDialect());
        dialects.put("wordpress", new WordpressDialect());
        XHTMLDialect xhtml = new XHTMLDialect();
        xhtml.defineDialect(new XHTMLOptions(Collections.emptyMap()));
        dialects.put("xhtml", xhtml);
        XLIFFDialect xliff = new XLIFFDialect();
        xliff.defineDialect(new XLIFFOptions(Collections.emptyMap()));
        dialects.put("xliff", xliff);
        dialects.put("xmlss", new XMLSpreadsheetDialect());

        Map<String, Object> out = new LinkedHashMap<>();
        List<Map<String, Object>> cases = new ArrayList<>();
        for (Map.Entry<String, DefaultXMLDialect> e : dialects.entrySet()) {
            cases.add(dumpDialect(e.getKey(), e.getValue()));
        }
        out.put("java_test", "org.omegat.filters3.xml.DefaultXMLDialect#getParagraphTags");
        out.put("exported_by", EXPORTED_BY);
        out.put("dialects", cases);
        writeJson(goldenRoot.resolve("engine/dialect_tags.json"), out);
        System.out.println("wrote engine/dialect_tags.json dialects=" + cases.size());
    }

    private Map<String, Object> dumpDialect(String id, DefaultXMLDialect dialect) throws Exception {
        Map<String, Object> m = new LinkedHashMap<>();
        m.put("id", id);
        m.put("class", dialect.getClass().getName());
        m.put("paragraph", sorted(dialect.getParagraphTags()));
        m.put("intact", sorted(dialect.getIntactTags()));
        m.put("out_of_turn", sorted(dialect.getOutOfTurnTags()));
        m.put("preformat", sorted(dialect.getPreformatTags()));
        m.put("attrs", sorted(dialect.getTranslatableAttributes()));
        m.put("tag_attrs", dumpTagAttrs(dialect.getTranslatableTagAttributes()));
        Map<String, String> constraints = new TreeMap<>();
        if (dialect.getConstraints() != null) {
            for (Map.Entry<Integer, Pattern> e : dialect.getConstraints().entrySet()) {
                constraints.put(constraintName(e.getKey()), e.getValue() == null ? "" : e.getValue().pattern());
            }
        }
        m.put("constraints", constraints);
        return m;
    }

    private static String constraintName(int key) {
        if (key == XMLDialect.CONSTRAINT_DOCTYPE) {
            return "doctype";
        }
        if (key == XMLDialect.CONSTRAINT_PUBLIC_DOCTYPE) {
            return "public_doctype";
        }
        if (key == XMLDialect.CONSTRAINT_SYSTEM_DOCTYPE) {
            return "system_doctype";
        }
        if (key == XMLDialect.CONSTRAINT_ROOT) {
            return "root";
        }
        if (key == XMLDialect.CONSTRAINT_XMLNS) {
            return "xmlns";
        }
        return "c" + key;
    }

    private static List<String> sorted(Set<String> set) {
        if (set == null) {
            return List.of();
        }
        List<String> out = new ArrayList<>(set);
        Collections.sort(out);
        return out;
    }

    @SuppressWarnings("unchecked")
    private Map<String, List<String>> dumpTagAttrs(MultiMap<String, String> mm) throws Exception {
        Map<String, List<String>> out = new TreeMap<>();
        if (mm == null) {
            return out;
        }
        Field f = MultiMap.class.getDeclaredField("map");
        f.setAccessible(true);
        Map<String, Set<String>> raw = (Map<String, Set<String>>) f.get(mm);
        if (raw == null) {
            return out;
        }
        for (Map.Entry<String, Set<String>> e : raw.entrySet()) {
            out.put(e.getKey(), sorted(e.getValue()));
        }
        return out;
    }

    private void exportIEditorMethods() throws Exception {
        Set<String> names = new TreeSet<>();
        for (Method m : IEditor.class.getDeclaredMethods()) {
            names.add(m.getName());
        }
        Map<String, Object> json = new LinkedHashMap<>();
        json.put("java_test", "org.omegat.gui.editor.IEditor#getCurrentEntry");
        json.put("exported_by", EXPORTED_BY);
        json.put("methods", new ArrayList<>(names));
        writeJson(goldenRoot.resolve("engine/ieditor_methods.json"), json);
        System.out.println("wrote engine/ieditor_methods.json methods=" + names.size());
    }

    private void exportMenuActions() throws Exception {
        List<String> actions = new ArrayList<>();
        for (Method m : MainWindowMenuHandler.class.getDeclaredMethods()) {
            if (m.getName().endsWith("ActionPerformed")) {
                actions.add(m.getName());
            }
        }
        Collections.sort(actions);
        Map<String, Object> json = new LinkedHashMap<>();
        json.put("java_test", "org.omegat.gui.main.MainWindowMenuHandler#projectNewMenuItemActionPerformed");
        json.put("exported_by", EXPORTED_BY);
        json.put("actions", actions);
        json.put("count", actions.size());
        writeJson(goldenRoot.resolve("engine/menu_actions.json"), json);
        System.out.println("wrote engine/menu_actions.json actions=" + actions.size());
    }

    private void exportPreferenceKeys() throws Exception {
        Path viewDir = javaRoot.resolve("src/main/java/org/omegat/gui/preferences/view");
        Pattern pref = Pattern.compile("Preferences\\.([A-Z][A-Z0-9_]+)");
        Map<String, List<String>> controllers = new TreeMap<>();
        if (Files.isDirectory(viewDir)) {
            try (var stream = Files.list(viewDir)) {
                for (Path p : stream.filter(x -> x.getFileName().toString().endsWith("Controller.java")).toList()) {
                    String src = Files.readString(p);
                    Set<String> keys = new TreeSet<>();
                    Matcher mt = pref.matcher(src);
                    while (mt.find()) {
                        String field = mt.group(1);
                        try {
                            Field f = Preferences.class.getField(field);
                            Object v = f.get(null);
                            if (v instanceof String s && !s.isEmpty()) {
                                keys.add(s);
                            }
                        } catch (ReflectiveOperationException ignored) {
                            keys.add(field);
                        }
                    }
                    controllers.put(p.getFileName().toString().replace(".java", ""), new ArrayList<>(keys));
                }
            }
        }
        Map<String, Object> json = new LinkedHashMap<>();
        json.put("java_test", "org.omegat.gui.preferences.view.GeneralOptionsController#persist");
        json.put("exported_by", EXPORTED_BY);
        json.put("controllers", controllers);
        writeJson(goldenRoot.resolve("engine/preference_keys.json"), json);
        System.out.println("wrote engine/preference_keys.json controllers=" + controllers.size());
    }

    private void exportFilterTestInventory() throws Exception {
        Path testDir = javaRoot.resolve("src/test/java/org/omegat/filters");
        Pattern testMethod = Pattern.compile("public void (test\\w+)\\s*\\(");
        List<Map<String, Object>> tests = new ArrayList<>();
        if (Files.isDirectory(testDir)) {
            try (var stream = Files.walk(testDir)) {
                for (Path p : stream.filter(x -> x.getFileName().toString().endsWith("Test.java")).toList()) {
                    String rel = javaRoot.relativize(p).toString().replace('\\', '/');
                    String className = rel.replace("src/test/java/", "").replace(".java", "").replace('/', '.');
                    String src = Files.readString(p);
                    Matcher mt = testMethod.matcher(src);
                    while (mt.find()) {
                        String method = mt.group(1);
                        Map<String, Object> row = new LinkedHashMap<>();
                        row.put("java_test", className + "#" + method);
                        row.put("class", className);
                        row.put("method", method);
                        row.put("golden", guessFilterGolden(className, method));
                        tests.add(row);
                    }
                }
            }
        }
        Map<String, Object> json = new LinkedHashMap<>();
        json.put("java_test", "org.omegat.filters.FiltersTest#testFiltersComparison");
        json.put("exported_by", EXPORTED_BY);
        json.put("tests", tests);
        writeJson(goldenRoot.resolve("engine/filter_tests.json"), json);
        System.out.println("wrote engine/filter_tests.json tests=" + tests.size());
    }

    private static String guessFilterGolden(String className, String method) {
        String simple = className.substring(className.lastIndexOf('.') + 1);
        String id = simple.replace("FilterTest", "").replace("Filter2Test", "").replace("Test", "").toLowerCase();
        if (simple.contains("HTML")) {
            id = "html";
        } else if (simple.contains("HHC")) {
            id = "hhc";
        } else if (simple.contains("PO")) {
            id = "po";
        } else if (simple.contains("INI")) {
            id = "ini";
        } else if (simple.contains("ResourceBundle")) {
            id = "properties";
        } else if (simple.contains("MozillaFTL")) {
            id = "mozftl";
        } else if (simple.contains("MozillaDTD")) {
            id = "mozdtd";
        } else if (simple.contains("MozillaLang")) {
            id = "mozlang";
        } else if (simple.contains("MoodlePHP")) {
            id = "moodlephp";
        } else if (simple.contains("DokuWiki")) {
            id = "dokuwiki";
        } else if (simple.contains("ILIAS")) {
            id = "ilias";
        } else if (simple.contains("Magento")) {
            id = "magento";
        } else if (simple.contains("Latex")) {
            id = "latex";
        } else if (simple.contains("HelpAndManual")) {
            id = "helpandmanual";
        } else if (simple.contains("XMLSpreadsheet")) {
            id = "xmlss";
        } else if (simple.contains("OpenXML")) {
            id = "openxml";
        } else if (simple.contains("OpenDoc")) {
            id = "opendoc";
        } else if (simple.contains("XLIFF")) {
            id = "xliff";
        } else if (simple.contains("XHTML")) {
            id = "xhtml";
        } else if (simple.contains("DocBook")) {
            id = "docbook";
        } else if (simple.contains("Android")) {
            id = "android";
        } else if (simple.contains("ResX")) {
            id = "resx";
        } else if (simple.contains("WiX")) {
            id = "wix";
        } else if (simple.contains("Svg")) {
            id = "svg";
        } else if (simple.contains("RelaxNG")) {
            id = "relaxng";
        } else if (simple.contains("Srt")) {
            id = "srt";
        } else if (simple.contains("Pdf")) {
            id = "pdf";
        } else if (simple.contains("Rc")) {
            id = "rc";
        } else if (simple.contains("Text")) {
            id = "text";
        } else if (simple.contains("Yaml")) {
            id = "yaml";
        }
        return "filters/" + id + "/" + method + ".json";
    }

    private void exportHtmlFilter2AllTests() throws Exception {
        exportFilter("html", "html/testParse.json", "html/file-HTMLFilter2.html",
                "org.omegat.filters.HTMLFilter2Test#testParse", new HTMLFilter2(), Collections.emptyMap(),
                "This is first line.", "Ceci est la premiere ligne.");
        Map<String, String> comments = new TreeMap<>();
        comments.put(HTMLOptions.OPTION_REMOVE_COMMENTS, "true");
        exportFilter("html", "html/testIgnoreCommentParse.json",
                "html/file-HTMLFilter2-ignored-comments-no-break-SF610.html",
                "org.omegat.filters.HTMLFilter2Test#testIgnoreCommentParse", new HTMLFilter2(), comments, null, null);
        exportFilter("html", "html/testParseAllBlockElements.json",
                "html/file-HTMLFilter2-all-block-elements.html",
                "org.omegat.filters.HTMLFilter2Test#testParseAllBlockElements", new HTMLFilter2(),
                Collections.emptyMap(), null, null);
        exportFilter("html", "html/testParseRegression-SF205.json",
                "html/file-HTMLFilter2-recurse-bugfix-SF205.html",
                "org.omegat.filters.HTMLFilter2Test#testParseRegression", new HTMLFilter2(), Collections.emptyMap(),
                null, null);
        exportFilter("html", "html/testParseRegression-SF609.json",
                "html/file-HTMLFilter2-tag-dropping-bugfix-SF609.html",
                "org.omegat.filters.HTMLFilter2Test#testParseRegression", new HTMLFilter2(), Collections.emptyMap(),
                null, null);
        exportFilter("html", "html/testParseRegression-SF613.json",
                "html/file-HTMLFilter2-tag-dropping-bugfix-SF613.html",
                "org.omegat.filters.HTMLFilter2Test#testParseRegression", new HTMLFilter2(), Collections.emptyMap(),
                null, null);
        exportFilter("html", "html/testParseRegression-SF873.json",
                "html/file-HTMLFilter2-tag-dropping-bugfix-SF873.html",
                "org.omegat.filters.HTMLFilter2Test#testParseRegression", new HTMLFilter2(), Collections.emptyMap(),
                null, null);
        exportFilter("html", "html/testParseRegression-OmegaT.json", "html/file-HTMLFilter2-OmegaT.html",
                "org.omegat.filters.HTMLFilter2Test#testParseRegression", new HTMLFilter2(), Collections.emptyMap(),
                null, null);
        exportFilter("html", "html/testTranslate.json", "html/file-HTMLFilter2.html",
                "org.omegat.filters.HTMLFilter2Test#testTranslate", new HTMLFilter2(), Collections.emptyMap(),
                null, null);
        exportFilter("html", "html/testLoad.json", "html/file-HTMLFilter2.html",
                "org.omegat.filters.HTMLFilter2Test#testLoad", new HTMLFilter2(), Collections.emptyMap(), null, null);
        exportFilter("html", "html/testLoad-SMP.json", "html/file-HTMLFilter2-SMP.html",
                "org.omegat.filters.HTMLFilter2Test#testLoad", new HTMLFilter2(), Collections.emptyMap(), null, null);
        exportFilter("html", "html/testTagsOptimization.json", "html/file-HTMLFilter2-tags-optimization.html",
                "org.omegat.filters.HTMLFilter2Test#testTagsOptimization", new HTMLFilter2(), Collections.emptyMap(),
                null, null);
        Map<String, String> never = new TreeMap<>();
        never.put(HTMLOptions.OPTION_REWRITE_ENCODING, "NEVER");
        exportFilter("html", "html/testLayout.json", "html/file-HTMLFilter2-layout.html",
                "org.omegat.filters.HTMLFilter2Test#testLayout", new HTMLFilter2(), never, null, null);
        Map<String, String> trim = new TreeMap<>();
        trim.put(HTMLOptions.OPTION_COMPRESS_WHITESPACE, "true");
        trim.put(HTMLOptions.OPTION_REWRITE_ENCODING, "NEVER");
        exportFilter("html", "html/testLayoutTrimWhitespace.json", "html/file-HTMLFilter2-layout.html",
                "org.omegat.filters.HTMLFilter2Test#testLayoutTrimWhitespace", new HTMLFilter2(), trim, null, null);
        exportFilter("html", "html/testLayoutPreserveWhitespace.json", "html/file-HTMLFilter2-layout.html",
                "org.omegat.filters.HTMLFilter2Test#testLayoutPreserveWhitespace", new HTMLFilter2(), never, null,
                null);
        Map<String, String> always = new TreeMap<>();
        always.put(HTMLOptions.OPTION_REWRITE_ENCODING, "ALWAYS");
        exportFilter("html", "html/testAddCharsetHeaderWhenNoHeader.json", "html/file-HTMLFilter2.html",
                "org.omegat.filters.HTMLFilter2Test#testAddCharsetHeaderWhenNoHeader", new HTMLFilter2(), always,
                null, null);
        exportFilter("html", "html/testAddCharsetHeaderWhenExistingHeader.json",
                "html/file-HTMLFilter2-headernocharset.html",
                "org.omegat.filters.HTMLFilter2Test#testAddCharsetHeaderWhenExistingHeader", new HTMLFilter2(),
                always, null, null);
        exportFilter("html", "html/testAddCharsetHeaderWhenExistingMeta.json",
                "html/file-HTMLFilter2-headerdifferentcharset.html",
                "org.omegat.filters.HTMLFilter2Test#testAddCharsetHeaderWhenExistingMeta", new HTMLFilter2(), always,
                null, null);
        exportFilter("html", "html/testAddCharsetHeaderHtml5WhenExistingMeta.json",
                "html/file-HTMLFilter2-HTML5-headerdifferentcharset.html",
                "org.omegat.filters.HTMLFilter2Test#testAddCharsetHeaderHtml5WhenExistingMeta", new HTMLFilter2(),
                always, null, null);
        Map<String, Object> entity = new LinkedHashMap<>();
        entity.put("id", "html");
        entity.put("fixture", "html/file-HTMLFilter2.html");
        entity.put("java_test", "org.omegat.filters.HTMLFilter2Test#testHtmlEntityDecode");
        entity.put("exported_by", EXPORTED_BY);
        entity.put("sources", List.of());
        entity.put("input", "foo &apos;bar&apos;");
        entity.put("decoded", HTMLUtils.entitiesToChars("foo &apos;bar&apos;"));
        writeJson(goldenRoot.resolve("filters/html/testHtmlEntityDecode.json"), entity);
    }

    private void exportHtmlOptionKeys() throws Exception {
        List<String> keys = new ArrayList<>();
        for (Field f : HTMLOptions.class.getFields()) {
            if (f.getName().startsWith("OPTION_") && f.getType() == String.class) {
                keys.add((String) f.get(null));
            }
        }
        Collections.sort(keys);
        Map<String, Object> json = new LinkedHashMap<>();
        json.put("java_test", "org.omegat.filters2.html2.HTMLOptions#getRewriteEncoding");
        json.put("exported_by", EXPORTED_BY);
        json.put("keys", keys);
        writeJson(goldenRoot.resolve("engine/html_options_keys.json"), json);
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
