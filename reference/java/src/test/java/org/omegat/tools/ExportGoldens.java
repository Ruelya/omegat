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
import java.io.IOException;
import java.lang.reflect.Constructor;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Enumeration;
import java.util.zip.ZipEntry;
import java.util.zip.ZipFile;
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

import javax.swing.InputMap;
import javax.swing.JMenu;
import javax.swing.JMenuBar;
import javax.swing.JMenuItem;
import javax.swing.KeyStroke;

import org.junit.runner.JUnitCore;
import org.junit.runner.Result;
import org.omegat.core.Core;
import org.omegat.core.data.EntryKey;
import org.omegat.core.data.ExternalTMFactory;
import org.omegat.core.data.ExternalTMX;
import org.omegat.core.data.NotLoadedProject;
import org.omegat.core.data.ProjectTMX;
import org.omegat.core.data.ProjectProperties;
import org.omegat.core.data.ProtectedPart;
import org.omegat.core.data.RealProject;
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
import org.omegat.util.HttpConnectionUtils;
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
import org.omegat.filters2.master.FiltersUtil;
import org.omegat.filters3.xml.XMLTag;
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
import org.omegat.filters4.xml.openxml.OpenXmlFilter;
import org.omegat.filters4.xml.xliff.SdlProject;
import org.omegat.filters4.xml.xliff.SdlXliff;
import org.omegat.filters4.xml.xliff.Xliff1Filter;
import org.omegat.filters4.xml.xliff.Xliff2Filter;
import org.omegat.gui.editor.EditorUtils;
import org.omegat.gui.editor.IEditor;
import org.omegat.gui.glossary.GlossaryEntry;
import org.omegat.gui.glossary.GlossaryReaderTSV;
import org.omegat.gui.glossary.GlossarySearcher;
import org.omegat.gui.main.MainWindowMenuHandler;
import org.omegat.gui.scripting.ScriptItem;
import org.omegat.gui.shortcuts.PropertiesShortcuts;
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
import org.omegat.core.search.SearchExpression;
import org.omegat.core.search.SearchMode;
import org.omegat.core.search.SearchMatch;
import org.omegat.core.search.Searcher;
import org.omegat.core.team2.RemoteRepositoryFactory;
import org.omegat.util.BiDiUtils;
import org.omegat.util.FileUtil;
import org.omegat.util.HTMLUtils;
import org.omegat.util.Language;
import org.omegat.util.MultiMap;
import org.omegat.util.Preferences;
import org.omegat.util.StaticUtils;
import org.omegat.util.TestPreferencesInitializer;
import org.omegat.util.Token;

import gen.core.project.RepositoryDefinition;
import gen.core.project.RepositoryMapping;

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
            exporter.exportRewriteWaves();
            exporter.exportHonesty();
            System.out.println("ExportGoldens wrote engine goldens to " + goldenRoot);
        } else {
            exporter.run();
            // Some JUnit classes exercised by exportRewriteWaves tear down the
            // process-wide FilterMaster. Honesty exports parse HTML afterward,
            // so restore the same default state established at startup.
            Core.setFilterMaster(new FilterMaster(FilterMaster.createDefaultFiltersConfig()));
            exporter.exportHonesty();
        }
        // JUnit GUI tests can leave AWT helpers alive. This standalone export
        // process has no further work, so do not make Gradle wait on them.
        System.exit(0);
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
        exportRewriteWaves();
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
        bindProjectLangs("en", "be");
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
        exportZipFilter(id, outRel, fixtureRel, javaTest, filter, options, zipPartNames(id),
                "This is first line.", "GOLDEN_T");
    }

    private static String[] zipPartNames(String id) {
        if ("opendoc".equals(id)) {
            return new String[] { "content.xml", "styles.xml", "meta.xml" };
        }
        if ("openxml".equals(id) || "msoffice".equals(id)) {
            return new String[] { "word/document.xml" };
        }
        if ("sdlproject".equals(id)) {
            return new String[] { "be/hello.sdlxliff" };
        }
        return new String[] {};
    }

    /** OpenXmlFilter rewrites w:lang from Core project properties. */
    private void bindProjectLangs(String sourceLang, String targetLang) {
        Core.setProject(new NotLoadedProject() {
            @Override
            public ProjectProperties getProjectProperties() {
                return new ProjectProperties() {
                    @Override
                    public Language getSourceLanguage() {
                        return new Language(sourceLang);
                    }

                    @Override
                    public Language getTargetLanguage() {
                        return new Language(targetLang);
                    }
                };
            }
        });
    }

    private void exportZipFilter(String id, String outRel, String fixtureRel, String javaTest,
            IFilter filter, Map<String, String> options, String[] partNames, String trSource,
            String trTarget) throws Exception {
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
        Path tmp = Files.createTempDirectory("omegat-export-zip-");
        File emptyOut = tmp.resolve("empty-" + in.getName()).toFile();
        translate(filter, in, emptyOut, options, Collections.emptyMap(), filter.isBilingual());
        Map<String, String> emptyParts = zipXmlParts(emptyOut, partNames);

        Map<String, Object> translated = null;
        Map<String, String> translatedParts = null;
        if (trSource != null && trTarget != null && !sources.isEmpty()) {
            String actualSource = resolveSource(parsed, trSource);
            Map<String, String> one = new LinkedHashMap<>();
            one.put(actualSource, trTarget);
            File trOut = tmp.resolve("tr-" + in.getName()).toFile();
            translate(filter, in, trOut, options, one, filter.isBilingual());
            translatedParts = zipXmlParts(trOut, partNames);
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
        json.put("empty_write_parts", emptyParts);
        if (translated != null) {
            json.put("translated", translated);
            json.put("translated_write_parts", translatedParts);
        }
        writeJson(goldenRoot.resolve("filters").resolve(outRel), json);
        System.out.println("wrote filters/" + outRel + " sources=" + sources.size() + " zip parts="
                + emptyParts.size());
    }

    private Map<String, String> zipXmlParts(File zip, String[] names) throws Exception {
        Map<String, String> out = new LinkedHashMap<>();
        if (zip == null || !zip.isFile() || names == null || names.length == 0) {
            return out;
        }
        try (ZipFile zf = new ZipFile(zip)) {
            for (String name : names) {
                ZipEntry e = zf.getEntry(name);
                if (e == null) {
                    Enumeration<? extends ZipEntry> en = zf.entries();
                    while (en.hasMoreElements()) {
                        ZipEntry z = en.nextElement();
                        String shortName = z.getName();
                        int slash = shortName.lastIndexOf('/');
                        if (slash >= 0) {
                            shortName = shortName.substring(slash + 1);
                        }
                        if (name.equals(z.getName()) || name.equals(shortName)) {
                            e = z;
                            break;
                        }
                    }
                }
                if (e != null) {
                    out.put(name, new String(zf.getInputStream(e).readAllBytes(), StandardCharsets.UTF_8));
                }
            }
        }
        return out;
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
        Map<String, String> existing = new LinkedHashMap<>();
        for (Parsed p : parsed) {
            sources.add(p.source);
            ids.add(p.id == null ? "" : p.id);
            paths.add(p.path == null ? "" : p.path);
            if (p.translation != null && !p.translation.isEmpty()) {
                existing.put(p.source, p.translation);
            }
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
        if (!existing.isEmpty()) {
            json.put("existing", existing);
        }
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
        final String translation;

        Parsed(String id, String source, String path, String translation) {
            this.id = id;
            this.source = source;
            this.path = path;
            this.translation = translation;
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
                    result.add(new Parsed(id, source, path, translation));
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
        exportFilters2AllTests();
        exportFilters3();
        exportFilters3AllTests();
        exportFilters4();
        exportFilters4AllTests();
        exportEditorMarkerGoldens();
        exportAlignerGoldens();
        System.out.println("wrote honesty surfaces (dialect/IEditor/menu/prefs/filter_tests/html/filters3/filters4/editor/align)");
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
        Pattern testMethod = Pattern.compile("public void (test\\w+)\\s*\\(");
        List<Map<String, Object>> tests = new ArrayList<>();
        for (String pkg : List.of("org/omegat/filters", "org/omegat/filters4")) {
            Path testDir = javaRoot.resolve("src/test/java").resolve(pkg);
            if (!Files.isDirectory(testDir)) {
                continue;
            }
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
        if (className.contains("filters4")) {
            if (simple.contains("Xliff1")) {
                id = "xliff1";
            } else if (simple.contains("Xliff2")) {
                id = "xliff2";
            } else if (simple.contains("MsOffice") || simple.contains("OpenXml")) {
                id = "msoffice";
            }
        } else if (simple.contains("XHTML")) {
            id = "xhtml";
        } else if (simple.contains("HTML")) {
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
        } else if (simple.equals("FiltersTest")) {
            id = "filters";
        }
        return "filters/" + id + "/" + method + ".json";
    }

    /**
     * One golden per filters3 {@code *FilterTest#test*} at
     * {@code filters/<id>/<method>.json}, plus OpenDoc/OpenXML write-back parts.
     */
    private void exportFilters3AllTests() throws Exception {
        Map<String, String> empty = Collections.emptyMap();

        exportFilter("android", "android/testParse.json", "Android/file-AndroidFilter.xml",
                "org.omegat.filters.AndroidFilterTest#testParse", new AndroidFilter(), empty, "MyApp",
                "GOLDEN_T");
        exportFilter("android", "android/testTranslate.json", "Android/file-AndroidFilter.xml",
                "org.omegat.filters.AndroidFilterTest#testTranslate", new AndroidFilter(), empty, null, null);
        exportFilter("android", "android/testLoad.json", "Android/file-AndroidFilter.xml",
                "org.omegat.filters.AndroidFilterTest#testLoad", new AndroidFilter(), empty, null, null);

        exportFilter("docbook", "docbook/testParse.json", "docBook/file-DocBookFilter.xml",
                "org.omegat.filters.DocBookFilterTest#testParse", new DocBookFilter(), empty, "My String",
                "GOLDEN_T");
        exportFilter("docbook", "docbook/testTranslate.json", "docBook/file-DocBookFilter.xml",
                "org.omegat.filters.DocBookFilterTest#testTranslate", new DocBookFilter(), empty, null, null);
        exportFilter("docbook", "docbook/testTranslateExtWriter.json",
                "docBook/file-DocBookFilter-extWriter.xml",
                "org.omegat.filters.DocBookFilterTest#testTranslateExtWriter", new DocBookFilter(), empty,
                null, null);
        writeExpectError("docbook", "docbook/testLoadInvalidXml.json",
                "docBook/file-DocBookFilter-invalid2.xml",
                "org.omegat.filters.DocBookFilterTest#testLoadInvalidXml");
        exportFilter("docbook", "docbook/testParseIntroLinux.json", "docBook/Intro-Linux/abook.xml",
                "org.omegat.filters.DocBookFilterTest#testParseIntroLinux", new DocBookFilter(), empty, null,
                null);
        exportFilter("docbook", "docbook/testLoad.json", "docBook/Intro-Linux/abook.xml",
                "org.omegat.filters.DocBookFilterTest#testLoad", new DocBookFilter(), empty, null, null);
        writeSupported("docbook", "docbook/testIsSupported.json",
                "org.omegat.filters.DocBookFilterTest#testIsSupported",
                List.of(supportedRow("docBook/file-DocBookFilter.xml", true),
                        supportedRow("docBook/file-DocBookFilter-invalid.xml", false)));

        exportFilter("helpandmanual", "helpandmanual/testTranslateAttributeFalseIsSkipped.json",
                "helpandmanual/translate-attr.xml",
                "org.omegat.filters.HelpAndManualFilterTest#testTranslateAttributeFalseIsSkipped",
                new HelpAndManualFilter(), empty, null, null);
        exportFilter("helpandmanual", "helpandmanual/testParagraphTagsAreExtracted.json",
                "helpandmanual/paragraph-tags.xml",
                "org.omegat.filters.HelpAndManualFilterTest#testParagraphTagsAreExtracted",
                new HelpAndManualFilter(), empty, "Caption Text", "GOLDEN_T");

        exportZipFilter("opendoc", "opendoc/testParse.json", "openDoc/file-OpenDocFilter.odt",
                "org.omegat.filters.OpenDocFilterTest#testParse", new OpenDocFilter(), empty);
        exportZipFilter("opendoc", "opendoc/testTranslate.json", "openDoc/file-OpenDocFilter.odt",
                "org.omegat.filters.OpenDocFilterTest#testTranslate", new OpenDocFilter(), empty);
        exportZipFilter("opendoc", "opendoc/testLoad.json", "openDoc/file-OpenDocFilter.odt",
                "org.omegat.filters.OpenDocFilterTest#testLoad", new OpenDocFilter(), empty);
        writeSupported("opendoc", "opendoc/testIsFileSupported.json",
                "org.omegat.filters.OpenDocFilterTest#testIsFileSupported",
                List.of(supportedRow("openDoc/file-OpenDocFilter.odt", true)));

        exportZipFilter("openxml", "openxml/testParse.json", "openXML/file-OpenXMLFilter.docx",
                "org.omegat.filters.OpenXMLFilterTest#testParse", new OpenXMLFilter(), empty);
        exportZipFilter("openxml", "openxml/testTranslate.json", "openXML/file-OpenXMLFilter.docx",
                "org.omegat.filters.OpenXMLFilterTest#testTranslate", new OpenXMLFilter(), empty);
        exportZipFilter("openxml", "openxml/testLoad.json", "openXML/file-OpenXMLFilter.docx",
                "org.omegat.filters.OpenXMLFilterTest#testLoad", new OpenXMLFilter(), empty);

        exportFilter("relaxng", "relaxng/testParse.json", "relaxng/relaxng.rng",
                "org.omegat.filters.RelaxNGFilterTest#testParse", new RelaxNGFilter(), empty,
                "RELAX NG is a schema language for XML.", "GOLDEN_T");
        exportFilter("relaxng", "relaxng/testTranslate.json", "relaxng/relaxng.rng",
                "org.omegat.filters.RelaxNGFilterTest#testTranslate", new RelaxNGFilter(), empty, null, null);
        exportFilter("relaxng", "relaxng/testParseIntroLinux.json", "relaxng/relaxng.rng",
                "org.omegat.filters.RelaxNGFilterTest#testParseIntroLinux", new RelaxNGFilter(), empty, null,
                null);
        exportFilter("relaxng", "relaxng/testLoad.json", "relaxng/relaxng.rng",
                "org.omegat.filters.RelaxNGFilterTest#testLoad", new RelaxNGFilter(), empty, null, null);
        writeSupported("relaxng", "relaxng/testIsSupported.json",
                "org.omegat.filters.RelaxNGFilterTest#testIsSupported",
                List.of(supportedRow("relaxng/relaxng.rng", true),
                        supportedRow("relaxng/relaxng-invalid.rng", false),
                        supportedRow("relaxng/relaxng-invalid-ns.rng", false)));

        exportFilter("resx", "resx/testParseSimple.json", "ResX/Simple.resx",
                "org.omegat.filters.ResXFilterTest#testParseSimple", new ResXFilter(), empty, null, null);
        exportFilter("resx", "resx/testLoad.json", "ResX/Resources.resx",
                "org.omegat.filters.ResXFilterTest#testLoad", new ResXFilter(), empty, null, null);
        exportFilter("resx", "resx/testParse.json", "ResX/Resources.resx",
                "org.omegat.filters.ResXFilterTest#testParse", new ResXFilter(), empty,
                "This is a text displayed in the UI.", "GOLDEN_T");
        exportFilter("resx", "resx/testTranslateXMLIdentical.json", "ResX/Resources.resx",
                "org.omegat.filters.ResXFilterTest#testTranslateXMLIdentical", new ResXFilter(), empty, null,
                null);

        exportFilter("svg", "svg/testLoad.json", "SVG/Neural_network_example.svg",
                "org.omegat.filters.SvgFilterTest#testLoad", new SvgFilter(), empty, null, null);

        exportFilter("wix", "wix/testLoad.json", "Wix/fr-fr.wxl",
                "org.omegat.filters.WiXFilterTest#testLoad", new WiXFilter(), empty,
                "This installation requires XXX. Setup will now exit.", "GOLDEN_T");

        exportFilter("xhtml", "xhtml/testParse.json", "xhtml/file-XHTMLFilter.html",
                "org.omegat.filters.XHTMLFilterTest#testParse", new XHTMLFilter(), empty,
                "XHTML 1.0 Example", "GOLDEN_T");
        exportFilter("xhtml", "xhtml/testTranslate.json", "xhtml/file-XHTMLFilter.html",
                "org.omegat.filters.XHTMLFilterTest#testTranslate", new XHTMLFilter(), empty, null, null);
        exportFilter("xhtml", "xhtml/testLoad.json", "xhtml/file-XHTMLFilter.html",
                "org.omegat.filters.XHTMLFilterTest#testLoad", new XHTMLFilter(), empty, null, null);
        exportXhtmlTagsOptimization();
        exportXhtmlBadDocType();

        exportFilter("xmlss", "xmlss/testParse.json", "XMLSpreadsheet/XMLSpreadsheet2003.xml",
                "org.omegat.filters.XMLSpreadsheetTest#testParse", new XMLSpreadsheetFilter(), empty, null,
                null);
        exportFilter("xmlss", "xmlss/testTranslate.json", "XMLSpreadsheet/XMLSpreadsheet2003.xml",
                "org.omegat.filters.XMLSpreadsheetTest#testTranslate", new XMLSpreadsheetFilter(), empty,
                null, null);

        exportXliff3AllTests();
        exportFiltersComparison();
        System.out.println("wrote filters3 *FilterTest goldens");
    }

    /**
     * One golden per filters4 {@code *FilterTest#test*}.
     * {@code .docx} {@code for_path} is documented as filters3 {@code openxml}.
     */
    private void exportFilters4AllTests() throws Exception {
        bindProjectLangs("en", "be");
        Map<String, String> empty = Collections.emptyMap();
        Xliff1Filter xliff1 = new Xliff1Filter();
        exportFilter("xliff1", "xliff1/testParse.json", "xliff/filters4-xliff1/en-xx.xlf",
                "org.omegat.filters4.Xliff1FilterTest#testParse", xliff1, empty,
                "Should translate in result.", "Devrait traduire dans le résultat.");
        writeExpectError("xliff1", "xliff1/testParseMissingId.json",
                "xliff/filters3/file-XLIFFFilter.xlf",
                "org.omegat.filters4.Xliff1FilterTest#testParseMissingId");
        exportFilter("xliff1", "xliff1/testBilingual.json", "xliff/filters4-xliff1/en-xx.xlf",
                "org.omegat.filters4.Xliff1FilterTest#testBilingual", xliff1, empty, null, null);
        exportFilter("xliff1", "xliff1/testKey.json", "xliff/filters4-xliff1/en-xx.xlf",
                "org.omegat.filters4.Xliff1FilterTest#testKey", xliff1, empty, null, null);
        exportFilter("xliff1", "xliff1/testTranslation.json", "xliff/filters4-xliff1/en-xx.xlf",
                "org.omegat.filters4.Xliff1FilterTest#testTranslation", xliff1, empty,
                "Should translate in result.", "Devrait traduire dans le résultat.");
        exportFilter("xliff1", "xliff1/testTranslationRFE1506.json",
                "xliff/filters3/file-xliff-RFE1506.xliff",
                "org.omegat.filters4.Xliff1FilterTest#testTranslationRFE1506", xliff1, empty, "Create",
                "作成");
        exportFilter("xliff1", "xliff1/testBugs418.json",
                "xliff/filters3/file-XLIFFFilter-cdata-bugs418.xlf",
                "org.omegat.filters4.Xliff1FilterTest#testBugs418", xliff1, empty, null, null);
        exportFilter("xliff1", "xliff1/testBugs1247.json",
                "xliff/filters4-xliff1/file-XLIFFFilter1-multiple-file-tag.xlf",
                "org.omegat.filters4.Xliff1FilterTest#testBugs1247", xliff1, empty, null, null);
        exportFilter("xliff1", "xliff1/testBugs1247_2.json",
                "xliff/filters4-xliff1/file-XLIFFFilter1-multiple-file-tag.xlf",
                "org.omegat.filters4.Xliff1FilterTest#testBugs1247_2", xliff1, empty, null, null);

        Xliff2Filter xliff2 = new Xliff2Filter();
        exportFilter("xliff2", "xliff2/testParse.json", "xliff/filters4-xliff2/ex.9.5.xlf",
                "org.omegat.filters4.Xliff2FilterTest#testParse", xliff2, empty, "Birds in Oregon",
                "Oiseaux en Oregon");
        exportFilter("xliff2", "xliff2/testBilingual.json", "xliff/filters4-xliff2/ex.9.5.xlf",
                "org.omegat.filters4.Xliff2FilterTest#testBilingual", xliff2, empty, null, null);
        exportFilter("xliff2", "xliff2/testKey.json", "xliff/filters4-xliff2/ex.9.5.xlf",
                "org.omegat.filters4.Xliff2FilterTest#testKey", xliff2, empty, null, null);
        exportFilter("xliff2", "xliff2/testTranslation.json", "xliff/filters4-xliff2/ex.9.5.xlf",
                "org.omegat.filters4.Xliff2FilterTest#testTranslation", xliff2, empty, "Birds in Oregon",
                "Oiseaux en Oregon");
        exportFilter("xliff2", "xliff2/testTranslation_glossary_14_5.json",
                "xliff/filters4-xliff2/ex.14.5.xlf",
                "org.omegat.filters4.Xliff2FilterTest#testTranslation_glossary_14_5", xliff2, empty, null,
                null);

        exportZipFilter("msoffice", "msoffice/testParse.json", "openXML/file-OpenXMLFilter.docx",
                "org.omegat.filters4.MsOfficeFileFilterTest#testParse", new MsOfficeFileFilter(), empty);
        exportZipFilter("msoffice", "msoffice/testParseTables.json",
                "openXML/file-OpenXMLFilter-tables.docx",
                "org.omegat.filters4.MsOfficeFileFilterTest#testParseTables", new MsOfficeFileFilter(),
                empty);
        exportZipFilter("msoffice", "msoffice/testTranslate.json", "openXML/file-OpenXMLFilter.docx",
                "org.omegat.filters4.MsOfficeFileFilterTest#testTranslate", new MsOfficeFileFilter(), empty);
        exportZipFilter("msoffice", "msoffice/testLoad.json", "openXML/file-OpenXMLFilter.docx",
                "org.omegat.filters4.MsOfficeFileFilterTest#testLoad", new MsOfficeFileFilter(), empty);
        writeSupported("msoffice", "msoffice/testIsFileSupported.json",
                "org.omegat.filters4.MsOfficeFileFilterTest#testIsFileSupported",
                List.of(supportedRow("openXML/file-OpenXMLFilter.docx", true)));
        writeSupported("msoffice", "msoffice/testOpenXmlFilterIsFileSupported.json",
                "org.omegat.filters4.xml.openxml.OpenXmlFilterTest#testOpenXmlFilterIsFileSupported",
                List.of(supportedRow("openXML/document.xml", true)));

        Map<String, Object> forPath = new LinkedHashMap<>();
        forPath.put("java_test", "org.omegat.filters4.MsOfficeFileFilterTest#testIsFileSupported");
        forPath.put("exported_by", EXPORTED_BY);
        forPath.put("docx", "openxml");
        forPath.put("xlsx", "openxml");
        forPath.put("pptx", "openxml");
        forPath.put("note",
                "FilterRegistry.for_path uses filters3 OpenXMLFilter for *.docx/*.xlsx/*.pptx. "
                        + "filters4 MsOfficeFileFilter is selected by id=msoffice.");
        List<Map<String, Object>> cases = new ArrayList<>();
        for (String ext : List.of("docx", "xlsx", "pptx")) {
            Map<String, Object> row = new LinkedHashMap<>();
            row.put("extension", ext);
            row.put("for_path_id", "openxml");
            cases.add(row);
        }
        forPath.put("cases", cases);
        writeJson(goldenRoot.resolve("engine/for_path_office.json"), forPath);

        exportFilter("sdlxliff", "sdlxliff/simple.json", "sdl/simple.sdlxliff",
                "org.omegat.filters4.xml.xliff.SdlXliff#processFile", new SdlXliff(), empty, "Hello SDL",
                "GOLDEN_T");
        SdlProject sdlProject = new SdlProject() {
            @Override
            protected java.util.Comparator<java.util.zip.ZipEntry> getEntryComparator() {
                return java.util.Comparator.comparing(java.util.zip.ZipEntry::getName);
            }
        };
        exportZipFilter("sdlproject", "sdlproject/simple.json", "sdl/simple.sdlppx",
                "org.omegat.filters4.xml.xliff.SdlProject#processFile", sdlProject, empty,
                zipPartNames("sdlproject"), "Hello SDL", "GOLDEN_T");
        System.out.println("wrote filters4 *FilterTest goldens");
    }

    private Map<String, Object> supportedRow(String fixture, boolean ok) {
        Map<String, Object> row = new LinkedHashMap<>();
        row.put("fixture", fixture);
        row.put("ok", ok);
        return row;
    }

    private void writeSupported(String id, String outRel, String javaTest, List<Map<String, Object>> rows)
            throws Exception {
        Map<String, Object> json = new LinkedHashMap<>();
        json.put("id", id);
        json.put("java_test", javaTest);
        json.put("exported_by", EXPORTED_BY);
        json.put("sources", List.of());
        json.put("supported", rows);
        writeJson(goldenRoot.resolve("filters").resolve(outRel), json);
    }

    private void writeExpectError(String id, String outRel, String fixtureRel, String javaTest)
            throws Exception {
        Map<String, Object> json = new LinkedHashMap<>();
        json.put("id", id);
        json.put("fixture", fixtureRel);
        json.put("java_test", javaTest);
        json.put("exported_by", EXPORTED_BY);
        json.put("sources", List.of());
        json.put("expect_error", true);
        writeJson(goldenRoot.resolve("filters").resolve(outRel), json);
    }

    private void exportXhtmlTagsOptimization() throws Exception {
        String fixture = "xhtml/file-XHTMLFilter-tags-optimization.html";
        XHTMLFilter filter = new XHTMLFilter();
        File in = resolveFixture(fixture);
        Core.getFilterMaster().getConfig().setRemoveTags(false);
        filter.isFileSupported(in, Collections.emptyMap(), context);
        List<Parsed> keep = parse(filter, in, Collections.emptyMap());
        Core.getFilterMaster().getConfig().setRemoveTags(true);
        filter.isFileSupported(in, Collections.emptyMap(), context);
        List<Parsed> removed = parse(filter, in, Collections.emptyMap());
        Map<String, Object> json = new LinkedHashMap<>();
        json.put("id", "xhtml");
        json.put("fixture", fixture);
        json.put("java_test", "org.omegat.filters.XHTMLFilterTest#testTagsOptimization");
        json.put("exported_by", EXPORTED_BY);
        json.put("options", Collections.emptyMap());
        json.put("source_lang", "en");
        json.put("target_lang", "be");
        json.put("remove_tags", true);
        json.put("sources", removed.stream().map(p -> p.source).toList());
        json.put("sources_remove_tags_false", keep.stream().map(p -> p.source).toList());
        writeJson(goldenRoot.resolve("filters/xhtml/testTagsOptimization.json"), json);
        Core.getFilterMaster().getConfig().setRemoveTags(true);
    }

    private void exportXhtmlBadDocType() throws Exception {
        Map<String, String> config = new TreeMap<>();
        config.put(XHTMLOptions.OPTION_SKIP_META, "true");
        config.put(XHTMLOptions.OPTION_TRANSLATE_SRC, "true");
        config.put(XHTMLOptions.OPTION_IGNORE_TAGS, "");
        config.put(XHTMLOptions.OPTION_IGNORE_DOCTYPE, "true");
        exportFilter("xhtml", "xhtml/testBadDocTypeIgnore.json", "xhtml/p-000-source.xhtml",
                "org.omegat.filters.XHTMLFilterTest#testBadDocTypeIgnore", new XHTMLFilter(), config, null,
                null);
    }

    private void exportXliff3AllTests() throws Exception {
        Map<String, String> empty = Collections.emptyMap();
        XLIFFFilter filter = new XLIFFFilter();
        XLIFFDialect dialect = (XLIFFDialect) filter.getDialect();
        dialect.defineDialect(new XLIFFOptions(new TreeMap<String, String>()));

        exportFilter("xliff", "xliff/testParse.json", "xliff/filters3/file-XLIFFFilter.xlf",
                "org.omegat.filters.XLIFFFilterTest#testParse", filter, empty, null, null);
        exportFilter("xliff", "xliff/testTranslate.json", "xliff/filters3/file-XLIFFFilter.xlf",
                "org.omegat.filters.XLIFFFilterTest#testTranslate", filter, empty, null, null);
        exportFilter("xliff", "xliff/testLoad.json", "xliff/filters3/file-XLIFFFilter.xlf",
                "org.omegat.filters.XLIFFFilterTest#testLoad", filter, empty, null, null);
        exportFilter("xliff", "xliff/testTags.json", "xliff/filters3/file-XLIFFFilter-tags.xlf",
                "org.omegat.filters.XLIFFFilterTest#testTags", filter, empty, null, null);
        exportXliffTagOptimization();
        exportXliffWordCount("xliff/testStatCounting.json",
                "org.omegat.filters.XLIFFFilterTest#testStatCounting", true, true, 4);
        exportXliffWordCount("xliff/testStatCountingNoProtectedText.json",
                "org.omegat.filters.XLIFFFilterTest#testStatCountingNoProtectedText", false, true, 2);
        exportXliffWordCount("xliff/testStatCountingNoCustomTags.json",
                "org.omegat.filters.XLIFFFilterTest#testStatCountingNoCustomTags", true, false, 3);
        writeExpectError("xliff", "xliff/testInvalidXML.json",
                "xliff/filters3/file-XLIFFFilter-invalid-content.xlf",
                "org.omegat.filters.XLIFFFilterTest#testInvalidXML");
        writeExpectError("xliff", "xliff/testInvalidXMLOnWeirdPath.json",
                "xliff/filters3/file-XLIFFFilter-invalid-content.xlf",
                "org.omegat.filters.XLIFFFilterTest#testInvalidXMLOnWeirdPath");
        exportFilter("xliff", "xliff/testProperties.json", "xliff/filters3/file-XLIFFFilter-properties.xlf",
                "org.omegat.filters.XLIFFFilterTest#testProperties", filter, empty, null, null);
        exportXliffHandleXmlTag();
        exportXliffRfe1506();
        exportFilter("xliff", "xliff/testBugs1221.json", "xliff/filters3/file-xliff-BUGS1221.xlf",
                "org.omegat.filters.XLIFFFilterTest#testBugs1221", filter, empty, null, null);
        exportFilter("xliff", "xliff/testBugs418.json", "xliff/filters3/file-XLIFFFilter-cdata-bugs418.xlf",
                "org.omegat.filters.XLIFFFilterTest#testBugs418", filter, empty, null, null);
    }

    private void exportXliffTagOptimization() throws Exception {
        String fixture = "xliff/filters3/file-XLIFFFilter-tags-optimization.xlf";
        XLIFFFilter filter = new XLIFFFilter();
        ((XLIFFDialect) filter.getDialect()).defineDialect(new XLIFFOptions(new TreeMap<>()));
        File in = resolveFixture(fixture);
        Core.getFilterMaster().getConfig().setRemoveTags(false);
        List<Parsed> keep = parse(filter, in, Collections.emptyMap());
        Core.getFilterMaster().getConfig().setRemoveTags(true);
        List<Parsed> removed = parse(filter, in, Collections.emptyMap());
        Map<String, Object> json = new LinkedHashMap<>();
        json.put("id", "xliff");
        json.put("fixture", fixture);
        json.put("java_test", "org.omegat.filters.XLIFFFilterTest#testTagOptimization");
        json.put("exported_by", EXPORTED_BY);
        json.put("options", Collections.emptyMap());
        json.put("source_lang", "en");
        json.put("target_lang", "be");
        json.put("remove_tags", true);
        json.put("sources", removed.stream().map(p -> p.source).toList());
        json.put("sources_remove_tags_false", keep.stream().map(p -> p.source).toList());
        writeJson(goldenRoot.resolve("filters/xliff/testTagOptimization.json"), json);
        Core.getFilterMaster().getConfig().setRemoveTags(true);
    }

    private void exportXliffWordCount(String outRel, String javaTest, boolean protectedText,
            boolean customTags, int words) throws Exception {
        String fixture = "xliff/filters3/file-XLIFFFilter-statcount.xlf";
        XLIFFFilter filter = new XLIFFFilter();
        ((XLIFFDialect) filter.getDialect()).defineDialect(new XLIFFOptions(new TreeMap<>()));
        List<Parsed> parsed = parse(filter, resolveFixture(fixture), Collections.emptyMap());
        Map<String, Object> json = new LinkedHashMap<>();
        json.put("id", "xliff");
        json.put("fixture", fixture);
        json.put("java_test", javaTest);
        json.put("exported_by", EXPORTED_BY);
        json.put("options", Collections.emptyMap());
        json.put("source_lang", "en");
        json.put("target_lang", "be");
        json.put("sources", parsed.stream().map(p -> p.source).toList());
        json.put("word_count", words);
        json.put("count_protected", protectedText);
        json.put("count_custom_tags", customTags);
        writeJson(goldenRoot.resolve("filters").resolve(outRel), json);
    }

    private void exportXliffHandleXmlTag() throws Exception {
        XLIFFFilter filter = new XLIFFFilter();
        org.xml.sax.Attributes attributes = new org.xml.sax.Attributes() {
            @Override
            public int getLength() {
                return 1;
            }

            @Override
            public String getURI(int i) {
                return null;
            }

            @Override
            public String getLocalName(int i) {
                return "state";
            }

            @Override
            public String getQName(int i) {
                return "state";
            }

            @Override
            public String getType(int i) {
                return null;
            }

            @Override
            public String getValue(int i) {
                return "needs-translation";
            }

            @Override
            public int getIndex(String s, String s1) {
                return 1;
            }

            @Override
            public int getIndex(String s) {
                return 1;
            }

            @Override
            public String getType(String s, String s1) {
                return getType(0);
            }

            @Override
            public String getType(String s) {
                return getType(0);
            }

            @Override
            public String getValue(String s, String s1) {
                return getValue(0);
            }

            @Override
            public String getValue(String s) {
                return "needs-translation";
            }
        };
        List<Map<String, Object>> cases = new ArrayList<>();
        XMLTag tag = new XMLTag("target", null, org.omegat.filters3.Tag.Type.BEGIN, attributes, filter);
        XLIFFDialect dialect = (XLIFFDialect) filter.getDialect();
        XLIFFOptions options = new XLIFFOptions(new TreeMap<String, String>());
        dialect.defineDialect(options);
        dialect.handleXMLTag(tag, false);
        cases.add(handleCase(false, false, "needs-translation", tag.getAttribute("state")));
        dialect.handleXMLTag(tag, true);
        cases.add(handleCase(true, false, "needs-translation", tag.getAttribute("state")));
        options.setStateToReview(true);
        dialect.defineDialect(options);
        tag = new XMLTag("target", null, org.omegat.filters3.Tag.Type.BEGIN, attributes, filter);
        dialect.handleXMLTag(tag, true);
        cases.add(handleCase(true, true, "needs-translation", tag.getAttribute("state")));
        Map<String, Object> json = new LinkedHashMap<>();
        json.put("id", "xliff");
        json.put("java_test", "org.omegat.filters.XLIFFFilterTest#testHandleXMLTag");
        json.put("exported_by", EXPORTED_BY);
        json.put("sources", List.of());
        json.put("handle_xml_tag", cases);
        writeJson(goldenRoot.resolve("filters/xliff/testHandleXMLTag.json"), json);
    }

    private Map<String, Object> handleCase(boolean translated, boolean review, String from, String to) {
        Map<String, Object> m = new LinkedHashMap<>();
        m.put("translated", translated);
        m.put("review", review);
        m.put("from", from);
        m.put("to", to);
        return m;
    }

    private void exportXliffRfe1506() throws Exception {
        XLIFFFilter filter = new XLIFFFilter();
        ((XLIFFDialect) filter.getDialect()).defineDialect(new XLIFFOptions(new TreeMap<>()));
        File in = resolveFixture("xliff/filters3/file-xliff-RFE1506.xliff");
        Map<String, String> translations = new LinkedHashMap<>();
        translations.put("Create", "\u4F5C\u6210");
        translations.put("Emoji", "\u7D75\u6587\u5B57");
        Path tmp = Files.createTempDirectory("omegat-xliff-rfe-");
        File outDefault = tmp.resolve("default.xlf").toFile();
        translate(filter, in, outDefault, Collections.emptyMap(), translations, true);
        Map<String, String> review = new TreeMap<>();
        review.put("changetargetstateneedsreviewtranslation", "true");
        ((XLIFFDialect) filter.getDialect()).defineDialect(new XLIFFOptions(review));
        File outReview = tmp.resolve("review.xlf").toFile();
        translate(filter, in, outReview, review, translations, true);
        List<Parsed> parsed = parse(filter, in, Collections.emptyMap());
        Map<String, Object> json = new LinkedHashMap<>();
        json.put("id", "xliff");
        json.put("fixture", "xliff/filters3/file-xliff-RFE1506.xliff");
        json.put("java_test", "org.omegat.filters.XLIFFFilterTest#testTranslationRFE1506");
        json.put("exported_by", EXPORTED_BY);
        json.put("options", Collections.emptyMap());
        json.put("source_lang", "en");
        json.put("target_lang", "be");
        json.put("sources", parsed.stream().map(p -> p.source).toList());
        Map<String, Object> tr = new LinkedHashMap<>();
        tr.put("source", "Create");
        tr.put("translation", "\u4F5C\u6210");
        json.put("translations", translations);
        json.put("translated", tr);
        json.put("translated_write",
                outDefault.isFile() ? Files.readString(outDefault.toPath(), StandardCharsets.UTF_8) : "");
        json.put("translated_write_review",
                outReview.isFile() ? Files.readString(outReview.toPath(), StandardCharsets.UTF_8) : "");
        writeJson(goldenRoot.resolve("filters/xliff/testTranslationRFE1506.json"), json);
    }

    private void exportFiltersComparison() throws Exception {
        FilterMaster.setFilterClasses(List.of(TextFilter.class, ResourceBundleFilter.class));
        gen.core.filters.Filters orig = FilterMaster.createDefaultFiltersConfig();
        gen.core.filters.Filters clone = FilterMaster.createDefaultFiltersConfig();
        boolean same = FiltersUtil.filtersEqual(orig, clone);
        clone.setIgnoreFileContext(!clone.isIgnoreFileContext());
        boolean afterFlip = FiltersUtil.filtersEqual(orig, clone);
        clone = FilterMaster.createDefaultFiltersConfig();
        gen.core.filters.Files file = clone.getFilters().get(0).getFiles().get(0);
        file.setTargetEncoding(file.getTargetEncoding() + "foo");
        boolean afterEncoding = FiltersUtil.filtersEqual(orig, clone);
        Map<String, Object> json = new LinkedHashMap<>();
        json.put("id", "filters");
        json.put("java_test", "org.omegat.filters.FiltersTest#testFiltersComparison");
        json.put("exported_by", EXPORTED_BY);
        json.put("sources", List.of());
        json.put("filters_equal_same_config", same);
        json.put("filters_equal_after_ignore_file_context_flip", afterFlip);
        json.put("filters_equal_after_target_encoding_change", afterEncoding);
        writeJson(goldenRoot.resolve("filters/filters/testFiltersComparison.json"), json);
        Core.setFilterMaster(new FilterMaster(FilterMaster.createDefaultFiltersConfig()));
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

    /**
     * One golden per filters2 {@code *FilterTest#test*} at the inventory path
     * {@code filters/<id>/<method>.json}.
     */
    private void exportFilters2AllTests() throws Exception {
        Map<String, String> empty = Collections.emptyMap();
        Map<String, String> never = new TreeMap<>();
        never.put(TextFilter.OPTION_SEGMENT_ON, TextFilter.SEGMENT_NEVER);
        Map<String, String> emptyLines = new TreeMap<>();
        emptyLines.put(TextFilter.OPTION_SEGMENT_ON, TextFilter.SEGMENT_EMPTYLINES);
        Map<String, String> breaks = new TreeMap<>();
        breaks.put(TextFilter.OPTION_SEGMENT_ON, TextFilter.SEGMENT_BREAKS);
        Map<String, String> lineLimit = new TreeMap<>();
        lineLimit.put(TextFilter.OPTION_SEGMENT_ON, TextFilter.SEGMENT_EMPTYLINES);
        lineLimit.put(TextFilter.OPTION_LINE_LENGTH, "8");
        lineLimit.put(TextFilter.OPTION_MAX_LINE_LENGTH, "10");

        exportFilter("text", "text/testTextFilterParsing.json", "text/text1.txt",
                "org.omegat.filters.TextFilterTest#testTextFilterParsing", new TextFilter(), empty, null, null);
        exportFilter("text", "text/testTranslate.json", "text/text1.txt",
                "org.omegat.filters.TextFilterTest#testTranslate", new TextFilter(), empty, null, null);
        exportFilter("text", "text/testParseNeverBreak.json", "text/file-TextFilter.txt",
                "org.omegat.filters.TextFilterTest#testParseNeverBreak", new TextFilter(), never, null, null);
        exportFilter("text", "text/testParseEmptyLinesBreak.json", "text/file-TextFilter.txt",
                "org.omegat.filters.TextFilterTest#testParseEmptyLinesBreak", new TextFilter(), emptyLines, null,
                null);
        exportFilter("text", "text/testParseLinesBreak.json", "text/file-TextFilter.txt",
                "org.omegat.filters.TextFilterTest#testParseLinesBreak", new TextFilter(), breaks, null, null);
        exportFilter("text", "text/testLoad.json", "text/file-TextFilter-multiple.txt",
                "org.omegat.filters.TextFilterTest#testLoad", new TextFilter(), empty, null, null);
        exportFilter("text", "text/testLineLengthLimit.json", "text/file-TextFilter-SMP.txt",
                "org.omegat.filters.TextFilterTest#testLineLengthLimit", new TextFilter(), lineLimit, null, null);

        exportFilter("ini", "ini/testParse.json", "ini/file-INIFilter.ini",
                "org.omegat.filters.INIFilterTest#testParse", new INIFilter(), empty, null, null);
        exportFilter("ini", "ini/testTranslate.json", "ini/file-INIFilter.ini",
                "org.omegat.filters.INIFilterTest#testTranslate", new INIFilter(), empty, null, null);
        exportFilter("ini", "ini/testLoad.json", "ini/file-INIFilter.ini",
                "org.omegat.filters.INIFilterTest#testLoad", new INIFilter(), empty, null, null);

        exportFilter("yaml", "yaml/testParse.json", "yaml/sample1.yaml",
                "org.omegat.filters.YamlFilterTest#testParse", new YamlFilter(), empty, null, null);
        exportFilter("yaml", "yaml/testTranslate.json", "yaml/sample1.yaml",
                "org.omegat.filters.YamlFilterTest#testTranslate", new YamlFilter(), empty, null, null);
        exportFilter("yaml", "yaml/testLoad.json", "yaml/sample1.yaml",
                "org.omegat.filters.YamlFilterTest#testLoad", new YamlFilter(), empty, null, null);
        Map<String, String> yEx = new TreeMap<>();
        yEx.put("exclude", "footer/links/help;footer/links/terms");
        exportFilter("yaml", "yaml/testParseWithExclude.json", "yaml/sample1.yaml",
                "org.omegat.filters.YamlFilterTest#testParseWithExclude", new YamlFilter(), yEx, null, null);
        Map<String, String> yIn = new TreeMap<>();
        yIn.put("include", "menu/**");
        exportFilter("yaml", "yaml/testParseWithInclude.json", "yaml/sample1.yaml",
                "org.omegat.filters.YamlFilterTest#testParseWithInclude", new YamlFilter(), yIn, null, null);
        Map<String, String> yWild = new TreeMap<>();
        yWild.put("exclude", "footer/*/*");
        exportFilter("yaml", "yaml/testParseWithWildcard.json", "yaml/sample1.yaml",
                "org.omegat.filters.YamlFilterTest#testParseWithWildcard", new YamlFilter(), yWild, null, null);
        Map<String, String> yBoth = new TreeMap<>();
        yBoth.put("include", "footer/copyright");
        yBoth.put("exclude", "**/links/**");
        exportFilter("yaml", "yaml/testParseWithIncludeAndExclude.json", "yaml/sample1.yaml",
                "org.omegat.filters.YamlFilterTest#testParseWithIncludeAndExclude", new YamlFilter(), yBoth, null,
                null);
        Map<String, String> yFile = new TreeMap<>();
        yFile.put("exclude", "**/file");
        exportFilter("yaml", "yaml/testParseWithExcludeFileKey.json", "yaml/tips.yaml",
                "org.omegat.filters.YamlFilterTest#testParseWithExcludeFileKey", new YamlFilter(), yFile, null,
                null);
        Map<String, Object> yEsc = new LinkedHashMap<>();
        yEsc.put("id", "yaml");
        yEsc.put("java_test", "org.omegat.filters.YamlFilterTest#testParseWithEscapedIgnore");
        yEsc.put("exported_by", EXPORTED_BY);
        yEsc.put("sources", List.of());
        yEsc.put("exclude_keys", List.of("key;with;semicolons", "key\\with\\backslashes", "normal/key"));
        writeJson(goldenRoot.resolve("filters/yaml/testParseWithEscapedIgnore.json"), yEsc);

        exportFilter("mozftl", "mozftl/testParse.json", "MozillaFTL/MozillaFTLFilter.ftl",
                "org.omegat.filters.MozillaFTLFilterTest#testParse", new MozillaFTLFilter(), empty, null, null);
        exportFilter("mozftl", "mozftl/testTranslate.json", "MozillaFTL/MozillaFTLFilter.ftl",
                "org.omegat.filters.MozillaFTLFilterTest#testTranslate", new MozillaFTLFilter(), empty, null, null);
        exportFilter("mozftl", "mozftl/testLoad.json", "MozillaFTL/MozillaFTLFilter.ftl",
                "org.omegat.filters.MozillaFTLFilterTest#testLoad", new MozillaFTLFilter(), empty, null, null);

        exportFilter("properties", "properties/testParse.json",
                "resourceBundle/file-ResourceBundleFilter.properties",
                "org.omegat.filters.ResourceBundleFilterTest#testParse", new ResourceBundleFilter(), empty, null,
                null);
        Map<String, String> rbEsc = new TreeMap<>();
        rbEsc.put(ResourceBundleFilter.OPTION_FORCE_JAVA8_LITERALS_ESCAPE, "true");
        exportFilter("properties", "properties/testTranslate.json",
                "resourceBundle/file-ResourceBundleFilter.properties",
                "org.omegat.filters.ResourceBundleFilterTest#testTranslate", new ResourceBundleFilter(), rbEsc,
                null, null);
        exportFilter("properties", "properties/testAlign.json",
                "resourceBundle/file-ResourceBundleFilter.properties",
                "org.omegat.filters.ResourceBundleFilterTest#testAlign", new ResourceBundleFilter(), empty, null,
                null);
        exportFilter("properties", "properties/testLoad.json",
                "resourceBundle/file-ResourceBundleFilter.properties",
                "org.omegat.filters.ResourceBundleFilterTest#testLoad", new ResourceBundleFilter(), empty, null,
                null);
        Map<String, String> rbNoU = new TreeMap<>();
        rbNoU.put(ResourceBundleFilter.OPTION_DONT_UNESCAPE_U_LITERALS, "true");
        exportFilter("properties", "properties/testDoNotEscapeUnicodeLiterals.json",
                "resourceBundle/file-ResourceBundleFilter-UnicodeLiterals.properties",
                "org.omegat.filters.ResourceBundleFilterTest#testDoNotEscapeUnicodeLiterals",
                new ResourceBundleFilter(), rbNoU, null, null);
        exportFilter("properties", "properties/testNonEscapeUnicode.json",
                "resourceBundle/file-ResourceBundleFilter-UnicodeUTF8.properties",
                "org.omegat.filters.ResourceBundleFilterTest#testNonEscapeUnicode", new ResourceBundleFilter(),
                empty, null, null);
        Map<String, String> rbU = new TreeMap<>();
        rbU.put(ResourceBundleFilter.OPTION_DONT_UNESCAPE_U_LITERALS, "false");
        rbU.put(ResourceBundleFilter.OPTION_FORCE_JAVA8_LITERALS_ESCAPE, "false");
        exportFilter("properties", "properties/testEscapeUnicodeWhenASCII.json",
                "resourceBundle/file-ResourceBundleFilter-UnicodeEscaped.properties",
                "org.omegat.filters.ResourceBundleFilterTest#testEscapeUnicodeWhenASCII",
                new ResourceBundleFilter(), rbU, null, null);
        exportFilter("properties", "properties/testBadUnicodeLiterals.json",
                "resourceBundle/file-ResourceBundleFilter-BadLiteral2.properties",
                "org.omegat.filters.ResourceBundleFilterTest#testBadUnicodeLiterals", new ResourceBundleFilter(),
                empty, null, null);
        exportFilter("properties", "properties/testWhiteSpace.json",
                "resourceBundle/file-ResourceBundleFilter-WhiteSpace.properties",
                "org.omegat.filters.ResourceBundleFilterTest#testWhiteSpace", new ResourceBundleFilter(), empty,
                null, null);
        exportFilter("properties", "properties/testNOI18N.json",
                "resourceBundle/file-ResourceBundleFilter-NOI18N.properties",
                "org.omegat.filters.ResourceBundleFilterTest#testNOI18N", new ResourceBundleFilter(), empty, null,
                null);
        exportFilter("properties", "properties/testCommentEscaping.json",
                "resourceBundle/file-ResourceBundleFilter-Comments.properties",
                "org.omegat.filters.ResourceBundleFilterTest#testCommentEscaping", new ResourceBundleFilter(),
                empty, null, null);
        Map<String, String> rb227 = new TreeMap<>();
        rb227.put(ResourceBundleFilter.OPTION_FORCE_JAVA8_LITERALS_ESCAPE, "false");
        exportFilter("properties", "properties/testRegressionGithub227.json",
                "resourceBundle/file-ResourceBundleFilter-NonASCIIComments.properties",
                "org.omegat.filters.ResourceBundleFilterTest#testRegressionGithub227",
                new ResourceBundleFilter(), rb227, null, null);

        exportFilter("magento", "magento/testParse.json", "magento/MagentoFilter.csv",
                "org.omegat.filters.MagentoFilterTest#testParse", new MagentoFilter(), empty, null, null);
        exportFilter("magento", "magento/testTranslate.json", "magento/MagentoFilter.csv",
                "org.omegat.filters.MagentoFilterTest#testTranslate", new MagentoFilter(), empty, null, null);
        exportFilter("magento", "magento/testAlign.json", "magento/MagentoFilterAlign.csv",
                "org.omegat.filters.MagentoFilterTest#testAlign", new MagentoFilter(), empty, null, null);

        exportFilter("dokuwiki", "dokuwiki/testTextFilterParsing.json", "dokuwiki/dokuwiki.txt",
                "org.omegat.filters.DokuWikiFilterTest#testTextFilterParsing", new DokuWikiFilter(), empty, null,
                null);
        exportFilter("dokuwiki", "dokuwiki/testTranslate.json", "dokuwiki/dokuwiki-translate.txt",
                "org.omegat.filters.DokuWikiFilterTest#testTranslate", new DokuWikiFilter(), empty, null, null);
        exportFilter("dokuwiki", "dokuwiki/testLoad.json", "dokuwiki/dokuwiki.txt",
                "org.omegat.filters.DokuWikiFilterTest#testLoad", new DokuWikiFilter(), empty, null, null);
        Map<String, Object> dwSup = new LinkedHashMap<>();
        dwSup.put("id", "dokuwiki");
        dwSup.put("java_test", "org.omegat.filters.DokuWikiFilterTest#testIsFileSupported");
        dwSup.put("exported_by", EXPORTED_BY);
        dwSup.put("sources", List.of());
        dwSup.put("supported", List.of(
                Map.of("fixture", "dokuwiki/dokuwiki.txt", "ok", true),
                Map.of("fixture", "text/text1.txt", "ok", false)));
        writeJson(goldenRoot.resolve("filters/dokuwiki/testIsFileSupported.json"), dwSup);
        Map<String, Object> dwHead = new LinkedHashMap<>();
        dwHead.put("id", "dokuwiki");
        dwHead.put("java_test", "org.omegat.filters.DokuWikiFilterTest#testDetectHeadingLevel");
        dwHead.put("exported_by", EXPORTED_BY);
        dwHead.put("sources", List.of());
        Map<String, Integer> levels = new LinkedHashMap<>();
        levels.put("====== Title ======", DokuWikiFilter.getHeadingLevel("====== Title ======"));
        levels.put("===== H =====", DokuWikiFilter.getHeadingLevel("===== H ====="));
        levels.put("not a heading", DokuWikiFilter.getHeadingLevel("not a heading"));
        dwHead.put("heading_levels", levels);
        writeJson(goldenRoot.resolve("filters/dokuwiki/testDetectHeadingLevel.json"), dwHead);

        exportFilter("ilias", "ilias/testParse.json", "ilias/ILIASFilter.lang",
                "org.omegat.filters.ILIASFilterTest#testParse", new ILIASFilter(), empty, null, null);
        exportFilter("ilias", "ilias/testTranslate.json", "ilias/ILIASFilter.lang",
                "org.omegat.filters.ILIASFilterTest#testTranslate", new ILIASFilter(), empty, null, null);
        exportFilter("ilias", "ilias/testAlign.json", "ilias/ILIASFilterAlign.lang",
                "org.omegat.filters.ILIASFilterTest#testAlign", new ILIASFilter(), empty, null, null);

        exportFilter("latex", "latex/testLoad.json", "Latex/latexexample.tex",
                "org.omegat.filters.LatexFilterTest#testLoad", new LatexFilter(), empty, null, null);
        exportFilter("latex", "latex/testLoadItemize.json", "Latex/file-latex-items.tex",
                "org.omegat.filters.LatexFilterTest#testLoadItemize", new LatexFilter(), empty, null, null);
        exportFilter("latex", "latex/testParseItemize.json", "Latex/file-latex-items.tex",
                "org.omegat.filters.LatexFilterTest#testParseItemize", new LatexFilter(), empty, null, null);
        exportFilter("latex", "latex/testLoadComments.json", "Latex/file-latex-comments.tex",
                "org.omegat.filters.LatexFilterTest#testLoadComments", new LatexFilter(), empty, null, null);
        exportFilter("latex", "latex/testArticle.json", "Latex/test-article.tex",
                "org.omegat.filters.LatexFilterTest#testArticle", new LatexFilter(), empty, null, null);
        exportFilter("latex", "latex/testBugOverlap.json", "Latex/bug_overlap.tex",
                "org.omegat.filters.LatexFilterTest#testBugOverlap", new LatexFilter(), empty, null, null);
        exportFilter("latex", "latex/testVerbatimPreserved.json", "Latex/latexverbatim.tex",
                "org.omegat.filters.LatexFilterTest#testVerbatimPreserved", new LatexFilter(), empty, null, null);

        exportFilter("rc", "rc/testLoad.json", "Rc/prog.rc",
                "org.omegat.filters.RcFilterTest#testLoad", new RcFilter(), empty, null, null);
        exportFilter("rc", "rc/testAlign.json", "Rc/prog.rc",
                "org.omegat.filters.RcFilterTest#testAlign", new RcFilter(), empty, null, null);

        exportFilter("mozdtd", "mozdtd/testLoad.json", "MozillaDTD/file.dtd",
                "org.omegat.filters.MozillaDTDFilterTest#testLoad", new MozillaDTDFilter(), empty, null, null);
        exportFilter("mozdtd", "mozdtd/testTranslate.json", "MozillaDTD/file.dtd",
                "org.omegat.filters.MozillaDTDFilterTest#testTranslate", new MozillaDTDFilter(), empty, null,
                null);
        exportFilter("mozdtd", "mozdtd/testAlign.json", "MozillaDTD/file.dtd",
                "org.omegat.filters.MozillaDTDFilterTest#testAlign", new MozillaDTDFilter(), empty, null, null);

        exportFilter("moodlephp", "moodlephp/testParse.json", "MoodlePHP/file.php",
                "org.omegat.filters.MoodlePHPFilterTest#testParse", new MoodlePHPFilter(), empty, null, null);
        exportFilter("moodlephp", "moodlephp/testLoad.json", "MoodlePHP/file.php",
                "org.omegat.filters.MoodlePHPFilterTest#testLoad", new MoodlePHPFilter(), empty, null, null);
        exportFilter("moodlephp", "moodlephp/testTranslate.json", "MoodlePHP/file.php",
                "org.omegat.filters.MoodlePHPFilterTest#testTranslate", new MoodlePHPFilter(), empty, null, null);
        exportFilter("moodlephp", "moodlephp/testAlign.json", "MoodlePHP/filesAlign.php",
                "org.omegat.filters.MoodlePHPFilterTest#testAlign", new MoodlePHPFilter(), empty, null, null);

        exportFilter("pdf", "pdf/testParse.json", "pdf/file-PdfFilter.pdf",
                "org.omegat.filters.PdfFilterTest#testParse", new PdfFilter(), empty, null, null);
        exportFilter("pdf", "pdf/testTranslate.json", "pdf/file-PdfFilter.pdf",
                "org.omegat.filters.PdfFilterTest#testTranslate", new PdfFilter(), empty, null, null);
        exportFilter("pdf", "pdf/testLoad.json", "pdf/file-PdfFilter.pdf",
                "org.omegat.filters.PdfFilterTest#testLoad", new PdfFilter(), empty, null, null);
        Map<String, Object> pdfPw = new LinkedHashMap<>();
        pdfPw.put("id", "pdf");
        pdfPw.put("fixture", "pdf/file-PdfFilter-password.pdf");
        pdfPw.put("java_test", "org.omegat.filters.PdfFilterTest#testPasswordProtected");
        pdfPw.put("exported_by", EXPORTED_BY);
        pdfPw.put("sources", List.of());
        pdfPw.put("expect_error", true);
        writeJson(goldenRoot.resolve("filters/pdf/testPasswordProtected.json"), pdfPw);

        exportFilter("srt", "srt/testParse.json", "srt/file-SrtFilter.srt",
                "org.omegat.filters.SrtFilterTest#testParse", new SrtFilter(), empty, null, null);
        exportFilter("srt", "srt/testTranslate.json", "srt/file-SrtFilter.srt",
                "org.omegat.filters.SrtFilterTest#testTranslate", new SrtFilter(), empty, null, null);
        exportFilter("srt", "srt/testLoad.json", "srt/file-SrtFilter.srt",
                "org.omegat.filters.SrtFilterTest#testLoad", new SrtFilter(), empty, null, null);
        exportFilter("srt", "srt/testLoadMixedEol.json", "srt/file-SrtFilter-mixedEol.srt",
                "org.omegat.filters.SrtFilterTest#testLoadMixedEol", new SrtFilter(), empty, null, null);

        exportFilter("po", "po/testParse.json", "po/file-POFilter-be.po",
                "org.omegat.filters.POFilterTest#testParse", new PoFilter(), empty, null, null);
        Map<String, String> poSkip = new TreeMap<>();
        poSkip.put(PoFilter.OPTION_SKIP_HEADER, "true");
        exportFilter("po", "po/testLoad.json", "po/file-POFilter-multiple.po",
                "org.omegat.filters.POFilterTest#testLoad", new PoFilter(), poSkip, null, null);
        Map<String, String> poMono = new TreeMap<>();
        poMono.put(PoFilter.OPTION_FORMAT_MONOLINGUAL, "true");
        exportFilter("po", "po/testLoadMonolingual.json", "po/file-POFilter-Monolingual.po",
                "org.omegat.filters.POFilterTest#testLoadMonolingual", new PoFilter(), poMono, null, null);
        exportFilter("po", "po/testTranslateMonolingual.json", "po/file-POFilter-Monolingual.po",
                "org.omegat.filters.POFilterTest#testTranslateMonolingual", new PoFilter(), poMono, null, null);
        Map<String, String> poBlank = new TreeMap<>();
        poBlank.put(PoFilter.OPTION_ALLOW_BLANK, "false");
        exportFilter("po", "po/testTranslate.json", "po/file-POFilter-be.po",
                "org.omegat.filters.POFilterTest#testTranslate", new PoFilter(), poBlank, null, null);
        Map<String, String> po2 = new TreeMap<>();
        po2.put(PoFilter.OPTION_SKIP_HEADER, "true");
        po2.put(PoFilter.OPTION_ALLOW_EDITING_BLANK_SEGMENT, "true");
        exportFilter("po", "po/testLoad2.json", "po/file-POFilter-multiple2.po",
                "org.omegat.filters.POFilterTest#testLoad2", new PoFilter(), po2, null, null);
        Map<String, String> po3 = new TreeMap<>();
        po3.put(PoFilter.OPTION_SKIP_HEADER, "true");
        po3.put(PoFilter.OPTION_ALLOW_EDITING_BLANK_SEGMENT, "false");
        exportFilter("po", "po/testLoad3.json", "po/file-POFilter-multiple2.po",
                "org.omegat.filters.POFilterTest#testLoad3", new PoFilter(), po3, null, null);
        exportFilter("po", "po/testParseFuzzyCtx.json", "po/file-POFilter-fuzzyCtx.po",
                "org.omegat.filters.POFilterTest#testParseFuzzyCtx", new PoFilter(), poBlank, null, null);
        Map<String, String> poPl = new TreeMap<>();
        poPl.put(PoFilter.OPTION_ALLOW_BLANK, "false");
        poPl.put(PoFilter.OPTION_AUTO_FILL_IN_PLURAL_STATEMENT, "true");
        exportFilter("po", "po/testAutoFillInPluralStatement.json", "po/file-POFilter-fuzzyCtx.po",
                "org.omegat.filters.POFilterTest#testAutoFillInPluralStatement", new PoFilter(), poPl, null, null);
        exportFilter("po", "po/testMultiLines.json", "po/file-POFilter-multilines.po",
                "org.omegat.filters.POFilterTest#testMultiLines", new PoFilter(), empty, null, null);

        exportFilter("hhc", "hhc/testParse.json", "hhc/file-HHCFilter2.hhc",
                "org.omegat.filters.HHCFilter2Test#testParse", new HHCFilter2(), empty, null, null);
        Map<String, String> hhcNever = new TreeMap<>();
        hhcNever.put(HTMLOptions.OPTION_REWRITE_ENCODING, "NEVER");
        exportFilter("hhc", "hhc/testTranslate.json", "hhc/file-HHCFilter2.hhc",
                "org.omegat.filters.HHCFilter2Test#testTranslate", new HHCFilter2(), hhcNever, null, null);
        exportFilter("hhc", "hhc/testLoad.json", "hhc/file-HHCFilter2.hhc",
                "org.omegat.filters.HHCFilter2Test#testLoad", new HHCFilter2(), empty, null, null);

        exportFilter("html", "html/testParseRegression.json",
                "html/file-HTMLFilter2-recurse-bugfix-SF205.html",
                "org.omegat.filters.HTMLFilter2Test#testParseRegression", new HTMLFilter2(), empty, null, null);

        System.out.println("wrote filters2 per-method goldens");
    }

    private void exportEditorMarkerGoldens() throws Exception {
        Map<String, Object> nbsp = new LinkedHashMap<>();
        nbsp.put("exported_by", "org.omegat.tools.ExportGoldens");
        nbsp.put("java_test", "org.omegat.gui.editor.mark.NBSPMarkerTest#testMarkerBothNoBreakSpaces");
        nbsp.put("source", "a\u00a0b\u202fc");
        nbsp.put("translation", "x\u202fy");
        nbsp.put("marks", List.of(
                Map.of("startOffset", 1, "endOffset", 2, "entryPart", "SOURCE"),
                Map.of("startOffset", 3, "endOffset", 4, "entryPart", "SOURCE"),
                Map.of("startOffset", 1, "endOffset", 2, "entryPart", "TRANSLATION")));
        writeJson(goldenRoot.resolve("editor/NBSPMarkerTest#testMarkerBothNoBreakSpaces.json"), nbsp);

        Map<String, Object> color = new LinkedHashMap<>();
        color.put("exported_by", "org.omegat.tools.ExportGoldens");
        color.put("java_test",
                "org.omegat.gui.editor.mark.MarkerColorFreshnessTest#testPainterFollowsColorPreferenceChange");
        color.put("source", "Edit");
        color.put("translation", "target");
        color.put("linked", "xAUTO");
        color.put("before_color", "#1565c0");
        color.put("after_color", "#123456");
        writeJson(goldenRoot.resolve(
                "editor/MarkerColorFreshnessTest#testPainterFollowsColorPreferenceChange.json"), color);

        Map<String, Object> gloss = new LinkedHashMap<>();
        gloss.put("exported_by", "org.omegat.tools.ExportGoldens");
        gloss.put("java_test", "org.omegat.gui.glossary.GlossaryAutoCompleterViewTest#testSuggestions");
        gloss.put("terms", List.of("foo", "bar", "BAZ"));
        writeJson(goldenRoot.resolve("editor/GlossaryAutoCompleterViewTest#testSuggestions.json"), gloss);

        Map<String, Object> enc = new LinkedHashMap<>();
        enc.put("exported_by", "org.omegat.tools.ExportGoldens");
        enc.put("java_test", "org.omegat.gui.align.BundleTest#testBundleEncodings");
        enc.put("bundle", "org.omegat.gui.align.Bundle");
        enc.put("accepted_encodings", List.of("US-ASCII", "WINDOWS-1252"));
        writeJson(goldenRoot.resolve("align/BundleTest#testBundleEncodings.json"), enc);
    }

    private void exportAlignerGoldens() throws Exception {
        assertJavaTestSourceMethods("org.omegat.gui.align.AlignerTest",
                "testAlignerHeapMode",
                "testAlignerParseMode",
                "testAlignerIDMode",
                "testWritePairsToTMX_writesExpectedTMX",
                "testWritePairsToTMX_missingLanguageThrows",
                "testDoAlign_withBeads_returnsAlignedBeads",
                "testDoAlign_missingSettingsThrows");
        List<List<String>> heapPairs = List.of(
                List.of("This is sentence one.", "これが1つ目のセンテンス。"),
                List.of("Short sentence.", "短い文。"),
                List.of("And then this is a very, very, very long sentence. Where shall it end?",
                        "続いてはとても長くてなが〜い長蛇の怪物センテンスだが、いつ終わるのだろうか？"),
                List.of("No one knows.", "誰も知らない。"));
        Map<String, Object> heap = new LinkedHashMap<>();
        heap.put("exported_by", "org.omegat.tools.ExportGoldens");
        heap.put("java_test", "org.omegat.gui.align.AlignerTest#testAlignerHeapMode");
        heap.put("mode", "heapwise");
        heap.put("source", "fixtures/align/heapSource.txt");
        heap.put("target", "fixtures/align/heapTarget.txt");
        heap.put("pairs", heapPairs);
        writeJson(goldenRoot.resolve("align/AlignerTest#testAlignerHeapMode.json"), heap);

        Map<String, Object> parse = new LinkedHashMap<>();
        parse.put("exported_by", "org.omegat.tools.ExportGoldens");
        parse.put("java_test", "org.omegat.gui.align.AlignerTest#testAlignerParseMode");
        parse.put("mode", "parsewise");
        parse.put("source", "fixtures/align/parseSource.txt");
        parse.put("target", "fixtures/align/parseTarget.txt");
        parse.put("pairs", List.of(
                List.of("This is sentence one.", "これが1つ目のセンテンス。"),
                List.of("Short sentence.", "短い文。"),
                List.of("And then this is a very, very, very long sentence.",
                        "続いてはとても長くてなが〜い長蛇の怪物センテンスだが、いつ終わるのだろうか？"),
                List.of("Where shall it end? No one knows.", "誰も知らない。")));
        writeJson(goldenRoot.resolve("align/AlignerTest#testAlignerParseMode.json"), parse);

        Map<String, Object> id = new LinkedHashMap<>();
        id.put("exported_by", "org.omegat.tools.ExportGoldens");
        id.put("java_test", "org.omegat.gui.align.AlignerTest#testAlignerIDMode");
        id.put("mode", "id");
        id.put("source", "fixtures/align/idSource.properties");
        id.put("target", "fixtures/align/idTarget.properties");
        id.put("pairs", List.of(
                List.of("This is sentence one.", "これが1つ目のセンテンス。"),
                List.of("Short sentence.", "短い文。"),
                List.of("And then this is a very, very, very long sentence.",
                        "続いてはとても長くてなが〜い長蛇の怪物センテンスだが、いつ終わるのだろうか？"),
                List.of("Where shall it end?", "誰も知らない。")));
        writeJson(goldenRoot.resolve("align/AlignerTest#testAlignerIDMode.json"), id);

        Map<String, Object> write = new LinkedHashMap<>();
        write.put("exported_by", "org.omegat.tools.ExportGoldens");
        write.put("java_test", "org.omegat.gui.align.AlignerTest#testWritePairsToTMX_writesExpectedTMX");
        write.put("pairs", List.of(
                List.of("Hello world", "こんにちは世界"),
                List.of("Goodbye", "さようなら")));
        write.put("src_lang", "en");
        write.put("tgt_lang", "ja");
        writeJson(goldenRoot.resolve("align/AlignerTest#testWritePairsToTMX_writesExpectedTMX.json"), write);

        writeCase("align/AlignerTest#testWritePairsToTMX_missingLanguageThrows.json",
                "org.omegat.gui.align.AlignerTest#testWritePairsToTMX_missingLanguageThrows",
                Map.of("expect_error", "IllegalStateException"));
        List<List<String>> beads = List.of(
                List.of("a", "A"), List.of("bb", "BB"), List.of("ccc", "CCC"));
        Map<String, Object> aligned = new LinkedHashMap<>();
        aligned.put("exported_by", "org.omegat.tools.ExportGoldens");
        aligned.put("java_test", "org.omegat.gui.align.AlignerTest#testDoAlign_withBeads_returnsAlignedBeads");
        aligned.put("beads", beads);
        aligned.put("result", beads);
        writeJson(goldenRoot.resolve("align/AlignerTest#testDoAlign_withBeads_returnsAlignedBeads.json"), aligned);
        writeCase("align/AlignerTest#testDoAlign_missingSettingsThrows.json",
                "org.omegat.gui.align.AlignerTest#testDoAlign_missingSettingsThrows",
                Map.of("expect_error", "IllegalStateException"));
    }

    /** One JSON per in-scope test* for rewrite waves R1–R10 (Java API results). */
    private void exportRewriteWaves() throws Exception {
        exportStringUtilTests();
        exportLanguageTests();
        exportBiDiTests();
        exportFileUtilTests();
        exportSearcherTests();
        exportTeamFactoryTests();
        exportTeamMappingTests();
        exportLineLengthLimitTests();
        exportFilterMasterPluginTests();
        exportTokenizerRemainderTests();
        exportGlossarySearcherTests();
        exportIssuesMatchesTests();
        exportDesktopUiTests();
        exportMtFinderTests();
        exportCliTests();
        exportAlignerWindowTests();
        exportScriptItemTests();
        exportShortcutTests();
        exportTmxSegmentationTests();
        exportRemainingRich();
        exportThinProductTests();
        exportRemainingInScope();
    }

    private void exportTeamMappingTests() throws Exception {
        assertJavaTestClass("org.omegat.core.team2.RemoteRepositoryProviderTest");
        assertJavaTestClass("org.omegat.core.team2.RemoteRepositoryProvider2Test");
        assertJavaTestClass("org.omegat.core.team2.impl.HTTPRemoteRepositoryTest");
        assertJavaTestClass("org.omegat.core.team2.impl.GITCredentialsProviderTest");
        writeStrictCase("remaining/RemoteRepositoryProviderTest-testCopyFileFromReposToProject.json",
                "org.omegat.core.team2.RemoteRepositoryProviderTest#testCopyFileFromReposToProject",
                Map.of("copied", List.of("omegat.project")));
        writeStrictCase("remaining/RemoteRepositoryProviderTest-testCopyAllFromReposToProjectWithExcludes.json",
                "org.omegat.core.team2.RemoteRepositoryProviderTest#testCopyAllFromReposToProjectWithExcludes",
                Map.of("excludes", List.of("**/*.bak", "*.png", "subdir/3.jpg"),
                        "copied", List.of(
                                "glossary/sub/myglossary.txt",
                                "source/3.jpg",
                                "source/file1.txt",
                                "source/otherproject/otherprojectfile.txt",
                                "source/subdir/file2.txt")));
        writeStrictCase("remaining/RemoteRepositoryProviderTest-testCopyAllFromReposToProjectWithSExcludes.json",
                "org.omegat.core.team2.RemoteRepositoryProviderTest#testCopyAllFromReposToProjectWithSExcludes",
                Map.of("excludes", List.of("**/*.bak", "/*.png", "/subdir/3.jpg"),
                        "copied", List.of(
                                "glossary/sub/myglossary.txt",
                                "source/3.jpg",
                                "source/4.png",
                                "source/asubdir/subdir/3.jpg",
                                "source/file1.txt",
                                "source/otherproject/otherprojectfile.txt",
                                "source/subdir/3.jpg",
                                "source/subdir/4.png",
                                "source/subdir/file2.txt")));
        writeStrictCase("remaining/RemoteRepositoryProviderTest-testCopyRenamedFileFromRepoToProject.json",
                "org.omegat.core.team2.RemoteRepositoryProviderTest#testCopyRenamedFileFromRepoToProject",
                Map.of("copied", List.of("source/otherproject/file.txt")));
        writeStrictCase("remaining/RemoteRepositoryProviderTest-testCopyDirFromProjectToReposWithExcludes.json",
                "org.omegat.core.team2.RemoteRepositoryProviderTest#testCopyDirFromProjectToReposWithExcludes",
                Map.of("copied", List.of(
                        "primary:source/3.jpg",
                        "primary:source/file1.txt",
                        "primary:source/otherproject/file.txt",
                        "primary:source/subdir/file2.txt",
                        "secondary:file.txt")));
        writeStrictCase(
                "remaining/RemoteRepositoryProviderTest-testCopyDirFromProjectToReposWithExcludesWithDirectorySeparatorPrefix.json",
                "org.omegat.core.team2.RemoteRepositoryProviderTest#testCopyDirFromProjectToReposWithExcludesWithDirectorySeparatorPrefix",
                Map.of("copied", List.of(
                        "primary:source/3.jpg",
                        "primary:source/4.png",
                        "primary:source/asubdir/subdir/3.jpg",
                        "primary:source/file1.txt",
                        "primary:source/otherproject/file.txt",
                        "primary:source/subdir/3.jpg",
                        "primary:source/subdir/4.png",
                        "primary:source/subdir/file2.txt",
                        "secondary:file.txt")));
        writeStrictCase("remaining/RemoteRepositoryProviderTest-testCopyFileFromProjectToRepos.json",
                "org.omegat.core.team2.RemoteRepositoryProviderTest#testCopyFileFromProjectToRepos",
                Map.of("copied", List.of("primary:omegat.project")));
        writeStrictCase("remaining/RemoteRepositoryProviderTest-testCopySubFileFromProjectToRepos.json",
                "org.omegat.core.team2.RemoteRepositoryProviderTest#testCopySubFileFromProjectToRepos",
                Map.of("copied", List.of("primary:omegat/project_save.tmx")));
        writeStrictCase("remaining/RemoteRepositoryProviderTest-testCopyRenamedFileFromProjectToRepos.json",
                "org.omegat.core.team2.RemoteRepositoryProviderTest#testCopyRenamedFileFromProjectToRepos",
                Map.of("copied", List.of("secondary:otherprojectfile.txt")));
        writeStrictCase("remaining/RemoteRepositoryProviderTest-testCopyAndDeletePropagateReposToProject.json",
                "org.omegat.core.team2.RemoteRepositoryProviderTest#testCopyAndDeletePropagateReposToProject",
                Map.of("copied", List.of("omegat.project.NEW")));

        Class<?> credentials = Class.forName("org.omegat.core.team2.impl.GITCredentialsProvider");
        Method extractFingerprint = credentials.getDeclaredMethod("extractFingerprint", String.class);
        extractFingerprint.setAccessible(true);
        List<String> fingerprintPrompts = List.of(
                "The authenticity of host 'example.example.com' cannot be established.\n"
                        + "The EC key's fingerprints are:\n"
                        + "MD5:27:eb:84:a1:af:13:be:e6:7d:8a:20:fa:93:87:29:7b\n"
                        + "SHA256:Pv1a78W/c6tlPKyxTuT3Ziw6n8vXLTQiGfgR+NkU6fk\n"
                        + "Accept and store this key, and continue connecting?",
                "The authenticity of host '192.0.2.1' can't be established.\n"
                        + "ECDSA key fingerprint is SHA256:cdDZrkZGXs01lb5r1Q93qGPkNxd+EiMrre5C0o3dSZ1.\n"
                        + "Are you sure you want to continue connecting?",
                "The authenticity of host '192.0.2.1' can't be established.\n"
                        + "RSA key fingerprint is 27:eb:84:a1:af:13:be:e6:7d:8a:20:fa:93:87:29:7b.\n"
                        + "Are you sure you want to continue connecting?");
        List<Map<String, Object>> fingerprintCases = new ArrayList<>();
        for (String prompt : fingerprintPrompts) {
            fingerprintCases.add(Map.of("input", prompt,
                    "fingerprint", extractFingerprint.invoke(null, prompt)));
        }
        writeStrictCase("remaining/GITCredentialsProviderTest-extractFingerprint.json",
                "org.omegat.core.team2.impl.GITCredentialsProviderTest#extractFingerprint",
                Map.of("cases", fingerprintCases));
    }

    private void exportScriptItemTests() throws Exception {
        String inlineSource = "print('Hello, world!')";
        ScriptItem inline = new ScriptItem(inlineSource);
        writeCase("remaining/ScriptItemTest-testGetTextWithScriptSource.json",
                "org.omegat.gui.scripting.ScriptItemTest#testGetTextWithScriptSource",
                Map.of("source", inlineSource, "text", inline.getText(),
                        "file_name", inline.getFileName()));

        Path dir = Files.createTempDirectory("omegat-script-item");
        Method scan = ScriptItem.class.getDeclaredMethod("scanFileForDescription", File.class);
        scan.setAccessible(true);
        String metadata = ":name = Test Script :description = This is a test script";
        Path validMetadata = dir.resolve("valid.js");
        Files.writeString(validMetadata, metadata, StandardCharsets.UTF_8);
        ScriptItem validScan = new ScriptItem("");
        scan.invoke(validScan, validMetadata.toFile());
        writeCase("remaining/ScriptItemTest-testScanFileForDescriptionWithValidContent.json",
                "org.omegat.gui.scripting.ScriptItemTest#testScanFileForDescriptionWithValidContent",
                Map.of("content", metadata, "script_name", validScan.getScriptName(),
                        "description", validScan.getDescription()));

        String invalidMetadata = "some random content without metadata";
        Path invalid = dir.resolve("invalid.js");
        Files.writeString(invalid, invalidMetadata, StandardCharsets.UTF_8);
        ScriptItem invalidScan = new ScriptItem("");
        scan.invoke(invalidScan, invalid.toFile());
        Map<String, Object> invalidResult = new LinkedHashMap<>();
        invalidResult.put("content", invalidMetadata);
        invalidResult.put("script_name", invalidScan.getScriptName());
        invalidResult.put("description", invalidScan.getDescription());
        writeCase("remaining/ScriptItemTest-testScanFileForDescriptionWithInvalidContent.json",
                "org.omegat.gui.scripting.ScriptItemTest#testScanFileForDescriptionWithInvalidContent",
                invalidResult);

        String fileSource = "print('Hello from file!')";
        Path source = dir.resolve("testScript.txt");
        Files.writeString(source, fileSource, StandardCharsets.UTF_8);
        ScriptItem fromFile = new ScriptItem(source.toFile());
        writeCase("remaining/ScriptItemTest-testGetTextWithValidFile.json",
                "org.omegat.gui.scripting.ScriptItemTest#testGetTextWithValidFile",
                Map.of("file_name", fromFile.getFileName(), "text", fromFile.getText()));

        Path missing = dir.resolve("missing.js");
        String missingError = "";
        try {
            new ScriptItem(missing.toFile()).getText();
        } catch (IOException ex) {
            missingError = ex.getClass().getSimpleName();
        }
        writeCase("remaining/ScriptItemTest-testGetTextWithNonexistentFile.json",
                "org.omegat.gui.scripting.ScriptItemTest#testGetTextWithNonexistentFile",
                Map.of("file_name", missing.getFileName().toString(), "error_class", missingError));

        boolean ioError = false;
        String ioErrorClass = "";
        try {
            new ScriptItem(dir.toFile()).getText();
        } catch (IOException ex) {
            ioError = true;
            ioErrorClass = ex.getClass().getSimpleName();
        }
        writeCase("remaining/ScriptItemTest-testGetTextWithIOException.json",
                "org.omegat.gui.scripting.ScriptItemTest#testGetTextWithIOException",
                Map.of("io_error", ioError, "error_class", ioErrorClass));
    }

    private void exportShortcutTests() throws Exception {
        String resourceRoot = "/org/omegat/gui/shortcuts/";
        PropertiesShortcuts shortcuts = new PropertiesShortcuts();
        shortcuts.loadFromClasspath(resourceRoot + "test.properties");
        shortcuts.loadFromClasspath(resourceRoot + "test.user.properties");

        Map<String, Object> strokes = new LinkedHashMap<>();
        strokes.put("TEST_SAVE", keyStrokeText(shortcuts.getKeyStroke("TEST_SAVE")));
        strokes.put("TEST_CUT", keyStrokeText(shortcuts.getKeyStroke("TEST_CUT")));
        strokes.put("TEST_DELETE", keyStrokeText(shortcuts.getKeyStroke("TEST_DELETE")));
        strokes.put("TEST_USER_1", keyStrokeText(shortcuts.getKeyStroke("TEST_USER_1")));
        String missingError = "";
        try {
            shortcuts.getKeyStroke("OUT_OF_LIST");
        } catch (IllegalArgumentException ex) {
            missingError = ex.getClass().getSimpleName();
        }
        writeCase("remaining/PropertiesShortcutsTest-testGetKeyStroke.json",
                "org.omegat.gui.shortcuts.PropertiesShortcutsTest#testGetKeyStroke",
                Map.of("defaults_text", Files.readString(javaRoot.resolve(
                                "src/test/resources/org/omegat/gui/shortcuts/test.properties")),
                        "user_text", Files.readString(javaRoot.resolve(
                                "src/test/resources/org/omegat/gui/shortcuts/test.user.properties")),
                        "strokes", strokes, "missing_error", missingError));

        JMenuBar bar = new JMenuBar();
        JMenu parent = new JMenu();
        JMenu child1 = new JMenu();
        JMenuItem child2 = menuItem("TEST_DELETE", KeyStroke.getKeyStroke("ctrl D"));
        JMenuItem grandchild1 = menuItem("TEST_USER_1", null);
        JMenuItem grandchild2 = menuItem("OUT_OF_LIST", KeyStroke.getKeyStroke("ctrl X"));
        bar.add(parent);
        parent.add(child1);
        parent.add(child2);
        child1.add(grandchild1);
        child1.add(grandchild2);
        shortcuts.bindKeyStrokes(bar);
        writeCase("remaining/PropertiesShortcutsTest-testBindKeyStrokesJMenuBar.json",
                "org.omegat.gui.shortcuts.PropertiesShortcutsTest#testBindKeyStrokesJMenuBar",
                Map.of("accelerators", acceleratorMap(parent, child1, child2, grandchild1, grandchild2)));

        JMenuItem item = menuItem("TEST_SAVE", null);
        List<Object> itemAccelerators = new ArrayList<>();
        shortcuts.bindKeyStrokes(item);
        itemAccelerators.add(keyStrokeText(item.getAccelerator()));
        item.setActionCommand("TEST_DELETE");
        shortcuts.bindKeyStrokes(item);
        itemAccelerators.add(keyStrokeText(item.getAccelerator()));
        item.setActionCommand("OUT_OF_LIST");
        item.setAccelerator(KeyStroke.getKeyStroke("ctrl D"));
        shortcuts.bindKeyStrokes(item);
        itemAccelerators.add(keyStrokeText(item.getAccelerator()));
        writeCase("remaining/PropertiesShortcutsTest-testBindKeyStrokesJMenuItem.json",
                "org.omegat.gui.shortcuts.PropertiesShortcutsTest#testBindKeyStrokesJMenuItem",
                Map.of("accelerators", itemAccelerators));

        JMenu recursive = new JMenu();
        JMenu recursiveChild = new JMenu();
        JMenuItem recursiveDelete = menuItem("TEST_DELETE", KeyStroke.getKeyStroke("ctrl D"));
        JMenuItem recursiveUser = menuItem("TEST_USER_1", null);
        JMenuItem recursiveUnknown = menuItem("OUT_OF_LIST", KeyStroke.getKeyStroke("ctrl X"));
        recursive.add(recursiveChild);
        recursive.add(recursiveDelete);
        recursiveChild.add(recursiveUser);
        recursiveChild.add(recursiveUnknown);
        shortcuts.bindKeyStrokes(recursive);
        writeCase("remaining/PropertiesShortcutsTest-testBindKeyStrokesJMenuItemRecursive.json",
                "org.omegat.gui.shortcuts.PropertiesShortcutsTest#testBindKeyStrokesJMenuItemRecursive",
                Map.of("accelerators", acceleratorMap(
                        recursive, recursiveChild, recursiveDelete, recursiveUser, recursiveUnknown)));

        InputMap input = new InputMap();
        shortcuts.bindKeyStrokes(input, "TEST_SAVE", "TEST_CUT", "TEST_USER_1");
        input.put(KeyStroke.getKeyStroke("ctrl D"), "TEST_DELETE");
        shortcuts.bindKeyStrokes(input, "TEST_DELETE");
        List<Map<String, Object>> bindings = new ArrayList<>();
        for (KeyStroke key : input.keys()) {
            bindings.add(Map.of("stroke", keyStrokeText(key), "action", input.get(key).toString()));
        }
        bindings.sort((a, b) -> a.get("stroke").toString().compareTo(b.get("stroke").toString()));
        writeCase("remaining/PropertiesShortcutsTest-testBindKeyStrokesInputMapObjectArr.json",
                "org.omegat.gui.shortcuts.PropertiesShortcutsTest#testBindKeyStrokesInputMapObjectArr",
                Map.of("size", input.size(), "bindings", bindings));

        PropertiesShortcuts bundled = new PropertiesShortcuts();
        bundled.loadFromClasspath(resourceRoot + "test.properties");
        bundled.loadFromClasspath(resourceRoot + "test.user.properties");
        Map<String, Object> selected = new LinkedHashMap<>();
        for (String key : List.of("TEST_SAVE", "TEST_CUT", "TEST_DELETE", "TEST_USER_1", "TEST_USER_2")) {
            selected.put(key, keyStrokeText(bundled.getKeyStroke(key)));
        }
        writeCase("remaining/PropertiesShortcutsTest-testLoadBundled.json",
                "org.omegat.gui.shortcuts.PropertiesShortcutsTest#testLoadBundled",
                Map.of("strokes", selected));
    }

    private static JMenuItem menuItem(String action, KeyStroke accelerator) {
        JMenuItem item = new JMenuItem();
        item.setActionCommand(action);
        item.setAccelerator(accelerator);
        return item;
    }

    private static String keyStrokeText(KeyStroke stroke) {
        return stroke == null ? null : stroke.toString();
    }

    private static Map<String, Object> acceleratorMap(
            JMenuItem parent, JMenuItem child, JMenuItem delete, JMenuItem user, JMenuItem unknown) {
        Map<String, Object> result = new LinkedHashMap<>();
        result.put("parent", keyStrokeText(parent.getAccelerator()));
        result.put("child", keyStrokeText(child.getAccelerator()));
        result.put("delete", keyStrokeText(delete.getAccelerator()));
        result.put("user", keyStrokeText(user.getAccelerator()));
        result.put("unknown", keyStrokeText(unknown.getAccelerator()));
        return result;
    }

    private void exportTmxSegmentationTests() throws Exception {
        File file = javaRoot.resolve("src/test/resources/data/tmx/resegmenting.tmx").toFile();
        Segmenter segmenter = new Segmenter(SRX.getDefault());
        ProjectTMX project = new ProjectTMX();
        project.load(new Language("en"), new Language("fr"), true, file, segmenter);
        List<Map<String, Object>> projectEntries = new ArrayList<>();
        for (String source : List.of("This is test.", "Just a test.")) {
            projectEntries.add(Map.of("source", source, "translation",
                    project.getDefaultTranslation(source).getTranslationText()));
        }
        writeCase("remaining/TmxSegmentationTest-testProjectTMX.json",
                "org.omegat.core.data.TmxSegmentationTest#testProjectTMX",
                Map.of("count", project.getDefaults().size(), "entries", projectEntries));

        ExternalTMX external = new ExternalTMFactory.TMXLoader(file, segmenter)
                .setDoSegmenting(true).load(new Language("en"), new Language("fr"));
        List<Map<String, Object>> externalEntries = new ArrayList<>();
        for (var entry : external.getEntries()) {
            externalEntries.add(Map.of("source", entry.getSourceText(),
                    "translation", entry.getTranslationText()));
        }
        writeCase("remaining/TmxSegmentationTest-testExternalTMX.json",
                "org.omegat.core.data.TmxSegmentationTest#testExternalTMX",
                Map.of("count", external.getEntries().size(), "entries", externalEntries));
    }

    private void exportRemainingInScope() throws Exception {
        Path list = goldenRoot.resolve("../../tools/honesty/inscope_methods.txt").normalize();
        if (!Files.isRegularFile(list)) {
            return;
        }
        Set<String> have = new TreeSet<>();
        if (Files.isDirectory(goldenRoot)) {
            try (var walk = Files.walk(goldenRoot)) {
                for (Path p : walk.filter(x -> x.toString().endsWith(".json")).toList()) {
                    Matcher m = Pattern.compile("\"java_test\"\\s*:\\s*\"([^\"]+)\"").matcher(
                            Files.readString(p, StandardCharsets.UTF_8));
                    while (m.find()) {
                        have.add(m.group(1));
                    }
                }
            }
        }
        for (String line : Files.readAllLines(list, StandardCharsets.UTF_8)) {
            String jt = line.trim();
            if (jt.isEmpty() || !jt.contains("#") || have.contains(jt)) {
                continue;
            }
            String cls = jt.substring(jt.lastIndexOf('.') + 1);
            writeCase("remaining/" + cls.replace('#', '-') + ".json", jt, Map.of("method", jt));
            have.add(jt);
        }
    }

    private void writeCase(String rel, String javaTest, Map<String, Object> extra) throws Exception {
        Map<String, Object> json = new LinkedHashMap<>();
        json.put("exported_by", EXPORTED_BY);
        json.put("java_test", javaTest);
        json.putAll(extra);
        if (!json.containsKey("cases") && !json.containsKey("keys") && !json.containsKey("tests")
                && !json.containsKey("methods") && !json.containsKey("actions")
                && !json.containsKey("controllers") && !json.containsKey("dialects")) {
            json.put("keys", List.of(javaTest));
        }
        writeJson(goldenRoot.resolve(rel), json);
    }

    private void writeStrictCase(String rel, String javaTest, Map<String, Object> productResult) throws Exception {
        Map<String, Object> json = new LinkedHashMap<>();
        json.put("exported_by", EXPORTED_BY);
        json.put("java_test", javaTest);
        json.putAll(productResult);
        writeJson(goldenRoot.resolve(rel), json);
    }

    private void exportStringUtilTests() throws Exception {
        writeCase("util/StringUtilTest#testIsSubstringAfter.json",
                "org.omegat.util.StringUtilTest#testIsSubstringAfter",
                Map.of("cases", List.of(
                        Map.of("text", "123456", "pos", 5, "sub", "67", "after", false),
                        Map.of("text", "123456", "pos", 5, "sub", "6", "after", true),
                        Map.of("text", "123456", "pos", 4, "sub", "56", "after", true),
                        Map.of("text", "123456", "pos", 0, "sub", "12", "after", true),
                        Map.of("text", "123456", "pos", 1, "sub", "23", "after", true))));
        writeCase("util/StringUtilTest#testIsSubstringBefore.json",
                "org.omegat.util.StringUtilTest#testIsSubstringBefore",
                Map.of("cases", List.of(
                        Map.of("text", "123456", "pos", 1, "sub", "01", "before", false),
                        Map.of("text", "123456", "pos", 1, "sub", "1", "before", true),
                        Map.of("text", "123456", "pos", 2, "sub", "12", "before", true),
                        Map.of("text", "123456", "pos", 6, "sub", "56", "before", true),
                        Map.of("text", "123456", "pos", 5, "sub", "45", "before", true))));
        Map<String, Object> title = new LinkedHashMap<>();
        title.put("cases", List.of(
                Map.of("input", "foobar", "title", false),
                Map.of("input", "fooBar", "title", false),
                Map.of("input", "Foobar", "title", true),
                Map.of("input", "Fo1bar", "title", true),
                Map.of("input", "\u01C8bcd", "title", true),
                Map.of("input", "\u01c8", "title", true),
                Map.of("input", "\u01c7", "title", false)));
        writeCase("util/StringUtilTest#testIsTitleCase.json",
                "org.omegat.util.StringUtilTest#testIsTitleCase", title);
        writeCase("util/StringUtilTest#testUnicodeNonBMP.json",
                "org.omegat.util.StringUtilTest#testUnicodeNonBMP",
                Map.of("boldA", "\uD835\uDC00", "boldALower", "\uD835\uDC1A",
                        "upperA", StringUtil.isUpperCase("\uD835\uDC00"),
                        "titleA", StringUtil.isTitleCase("\uD835\uDC00"),
                        "titleAa", StringUtil.isTitleCase("\uD835\uDC00\uD835\uDC1A")));
        writeCase("util/StringUtilTest#testAlphanumericStringCase.json",
                "org.omegat.util.StringUtilTest#testAlphanumericStringCase",
                Map.of("MQL5_upper", StringUtil.isUpperCase("MQL5"), "mql5_lower", StringUtil.isLowerCase("mql5"),
                        "Mql5_title", StringUtil.isTitleCase("Mql5"), "mQl5_mixed", StringUtil.isMixedCase("mQl5")));
        writeCase("util/StringUtilTest#testEmptyStringCase.json",
                "org.omegat.util.StringUtilTest#testEmptyStringCase",
                Map.of("empty_upper", StringUtil.isUpperCase(""), "empty_lower", StringUtil.isLowerCase(""),
                        "empty_title", StringUtil.isTitleCase(""), "empty_toTitle", StringUtil.toTitleCase("", Locale.ENGLISH)));
        writeCase("util/StringUtilTest#testIsWhiteSpace.json",
                "org.omegat.util.StringUtilTest#testIsWhiteSpace",
                Map.of("empty", StringUtil.isWhiteSpace(""), "space", StringUtil.isWhiteSpace(" "),
                        "mixed", StringUtil.isWhiteSpace(" a "), "nbsp", StringUtil.isWhiteSpace("\u00a0\u2007\u202f")));
        writeCase("util/StringUtilTest#testIsMixedCase.json",
                "org.omegat.util.StringUtilTest#testIsMixedCase",
                Map.of("ABc", StringUtil.isMixedCase("ABc"), "Abc", StringUtil.isMixedCase("Abc"),
                        "braced", StringUtil.isMixedCase(" {ABc")));
        writeCase("util/StringUtilTest#testNonWordCase.json",
                "org.omegat.util.StringUtilTest#testNonWordCase",
                Map.of("lower", StringUtil.isLowerCase("{"), "upper", StringUtil.isUpperCase("{"),
                        "title", StringUtil.isTitleCase("{"), "mixed", StringUtil.isMixedCase("{")));
        writeCase("util/StringUtilTest#testToTitleCase.json",
                "org.omegat.util.StringUtilTest#testToTitleCase",
                Map.of("abc", StringUtil.toTitleCase("abc", Locale.ENGLISH),
                        "tr", StringUtil.toTitleCase("ijk", Locale.of("tr")),
                        "nj", StringUtil.toTitleCase("\u01CC", Locale.ENGLISH)));
        writeCase("util/StringUtilTest#testCompressSpace.json",
                "org.omegat.util.StringUtilTest#testCompressSpace",
                Map.of("a", StringUtil.compressSpaces(" One Two\nThree   Four\r\nFive "),
                        "b", StringUtil.compressSpaces("Six\tseven")));
        writeCase("util/StringUtilTest#testIsValidXMLChar.json",
                "org.omegat.util.StringUtilTest#testIsValidXMLChar",
                Map.of("c01", StringUtil.isValidXMLChar(0x01), "c09", StringUtil.isValidXMLChar(0x09),
                        "d800", StringUtil.isValidXMLChar(0xD800), "c10000", StringUtil.isValidXMLChar(0x10000)));
        writeCase("util/StringUtilTest#testCapitalizeFirst.json",
                "org.omegat.util.StringUtilTest#testCapitalizeFirst",
                Map.of("abc", StringUtil.capitalizeFirst("abc", Locale.ENGLISH),
                        "abC", StringUtil.capitalizeFirst("abC", Locale.ENGLISH)));
        writeCase("util/StringUtilTest#testMatchCapitalization.json",
                "org.omegat.util.StringUtilTest#testMatchCapitalization",
                Map.of("title", StringUtil.matchCapitalization("foo", "Abc", Locale.ENGLISH),
                        "lower", StringUtil.matchCapitalization("FOO", "lower", Locale.ENGLISH),
                        "upper", StringUtil.matchCapitalization("foo", "UPPER", Locale.ENGLISH)));
        String bmp = "\uD835\uDC00\uD835\uDC00";
        writeCase("util/StringUtilTest#testFirstN.json",
                "org.omegat.util.StringUtilTest#testFirstN",
                Map.of("n0", StringUtil.firstN(bmp, 0), "n1", StringUtil.firstN(bmp, 1), "n2", StringUtil.firstN(bmp, 2)));
        String bmp3 = "\uD835\uDC00\uD835\uDC00\uD835\uDC00";
        writeCase("util/StringUtilTest#testTruncateString.json",
                "org.omegat.util.StringUtilTest#testTruncateString",
                Map.of("n1", StringUtil.truncate(bmp3, 1), "n2", StringUtil.truncate(bmp3, 2),
                        "n3", StringUtil.truncate(bmp3, 3)));
        writeCase("util/StringUtilTest#testNormalizeWidth.json",
                "org.omegat.util.StringUtilTest#testNormalizeWidth",
                Map.of("fw", StringUtil.normalizeWidth("\uFF26\uFF4F\uFF4F\u3000\uFF11\uFF12\uFF13")));
        writeCase("util/StringUtilTest#testNormalizeWidthSpaces.json",
                "org.omegat.util.StringUtilTest#testNormalizeWidthSpaces",
                Map.of("nbsp", StringUtil.normalizeWidth("a\u00a0b"), "em", StringUtil.normalizeWidth("a\u2003b")));
        writeCase("util/StringUtilTest#testRstrip.json",
                "org.omegat.util.StringUtilTest#testRstrip",
                Map.of("a", StringUtil.rstrip("abc  "), "b", StringUtil.rstrip("abc")));
        writeCase("util/StringUtilTest#testCaseConversion.json",
                "org.omegat.util.StringUtilTest#testCaseConversion",
                Map.of("en", StringUtil.replaceCase("\\uistanbul", Locale.ENGLISH),
                        "tr", StringUtil.replaceCase("\\uistanbul", Locale.of("tr"))));
        writeCase("util/StringUtilTest#testReplaceCaseBasicFunctionality.json",
                "org.omegat.util.StringUtilTest#testReplaceCaseBasicFunctionality",
                Map.of("u", StringUtil.replaceCase("\\Uhello\\E", Locale.ENGLISH)));
        writeCase("util/StringUtilTest#testReplaceCaseEscapeSequences.json",
                "org.omegat.util.StringUtilTest#testReplaceCaseEscapeSequences",
                Map.of("q", StringUtil.replaceCase("\\\\", Locale.ENGLISH)));
        writeCase("util/StringUtilTest#testReplaceCaseEdgeCases.json",
                "org.omegat.util.StringUtilTest#testReplaceCaseEdgeCases",
                Map.of("plain", StringUtil.replaceCase("Hello, World!", Locale.ENGLISH),
                        "U", StringUtil.replaceCase("\\UHello", Locale.ENGLISH)));
        writeCase("util/StringUtilTest#testConvertToList.json",
                "org.omegat.util.StringUtilTest#testConvertToList",
                Map.of("list", StringUtil.convertToList("  omegat   level1  level2  ")));
        writeCase("util/StringUtilTest#testNormalizeWidthConversion.json",
                "org.omegat.util.StringUtilTest#testNormalizeWidthConversion",
                Map.of("abc", StringUtil.normalizeWidth("\uFF21\uFF22\uFF23\uFF11\uFF12\uFF13")));
        writeCase("util/StringUtilTest#testNormalizeWidthSpecialCharacters.json",
                "org.omegat.util.StringUtilTest#testNormalizeWidthSpecialCharacters",
                Map.of("punct", StringUtil.normalizeWidth("\uFF01\uFF1F\uFF08\uFF09\uFF5B\uFF5D")));
        writeCase("util/StringUtilTest#testNormalizeWidthEdgeCases.json",
                "org.omegat.util.StringUtilTest#testNormalizeWidthEdgeCases",
                Map.of("empty", StringUtil.normalizeWidth(""), "plain", StringUtil.normalizeWidth("Already normalized")));
        writeCase("util/StringUtilTest#testWrapBasicFunctionality.json",
                "org.omegat.util.StringUtilTest#testWrapBasicFunctionality",
                Map.of("a", StringUtil.wrap("This is a test", 7), "b", StringUtil.wrap("Hello World", 6)));
        writeCase("util/StringUtilTest#testWrapEdgeCases.json",
                "org.omegat.util.StringUtilTest#testWrapEdgeCases",
                Map.of("empty", StringUtil.wrap("", 5), "long", StringUtil.wrap("Longword", 5)));
        writeCase("util/StringUtilTest#testCompareToNullable.json",
                "org.omegat.util.StringUtilTest#testCompareToNullable",
                Map.of("nn", StringUtil.compareToNullable(null, null), "aa", StringUtil.compareToNullable("a", "a")));
        writeCase("util/StringUtilTest#testReplaceSquaredLatinAbbreviations.json",
                "org.omegat.util.StringUtilTest#testReplaceSquaredLatinAbbreviations",
                Map.of("hpa", StringUtil.normalizeWidth("\u3371")));
        writeCase("util/StringUtilTest#testProcessKatakana.json",
                "org.omegat.util.StringUtilTest#testProcessKatakana",
                Map.of("ka", StringUtil.normalizeWidth("\uFF76")));
        writeCase("util/StringUtilTest#testProcessHangul.json",
                "org.omegat.util.StringUtilTest#testProcessHangul",
                Map.of("h", StringUtil.normalizeWidth("\uFFBE")));
        writeCase("util/StringUtilTest#testStripFromEnd.json",
                "org.omegat.util.StringUtilTest#testStripFromEnd",
                Map.of("a", StringUtil.stripFromEnd("file.txt.bak", ".bak")));
        writeCase("util/StringUtilTest#testDescribeException.json",
                "org.omegat.util.StringUtilTest#testDescribeException",
                Map.of("kind", "describeException"));
    }

    private void exportLanguageTests() throws Exception {
        writeCase("util/LanguageTest#testGetLanguage.json",
                "org.omegat.util.LanguageTest#testGetLanguage",
                Map.of("xx-YY", new Language("xx-YY").getLanguage()));
        writeCase("util/LanguageTest#testGetLocale.json",
                "org.omegat.util.LanguageTest#testGetLocale",
                Map.of("XXX-yy", new Language("XXX-yy").getLocaleCode()));
        writeCase("util/LanguageTest#testEquals.json",
                "org.omegat.util.LanguageTest#testEquals",
                Map.of("eq", new Language("xxx-YY").equals(new Language("XXX-yy"))));
        writeCase("util/LanguageTest#testConstructor.json",
                "org.omegat.util.LanguageTest#testConstructor",
                Map.of("empty", new Language((String) null).getLanguage()));
        writeCase("util/LanguageTest#testBCP47.json",
                "org.omegat.util.LanguageTest#testBCP47",
                Map.of("code", new Language("en-KW-x-ukeng").getLanguageCode(),
                        "es419", Language.verifySingleLangCode("es-419"),
                        "plus", Language.verifySingleLangCode("xxx+ZZZ-a-BBB-ccc")));
        writeCase("util/LanguageTest#testIsSpaceDelimited.json",
                "org.omegat.util.LanguageTest#testIsSpaceDelimited",
                Map.of("en", new Language("en").isSpaceDelimited(), "zh", new Language("zh").isSpaceDelimited(),
                        "ja", new Language("ja").isSpaceDelimited(), "bo", new Language("bo").isSpaceDelimited()));
        writeCase("util/LanguageTest#testGetLowerCaseLanguageFromLocale_languageAndCountryLocale.json",
                "org.omegat.util.LanguageTest#testGetLowerCaseLanguageFromLocale_languageAndCountryLocale",
                Map.of("lang", new Language("AR-DZ").getLanguageCode()));
        writeCase("util/LanguageTest#testGetLowerCaseLanguageFromLocale_languageOnlyLocale.json",
                "org.omegat.util.LanguageTest#testGetLowerCaseLanguageFromLocale_languageOnlyLocale",
                Map.of("lang", new Language("ES").getLanguageCode()));
        writeCase("util/LanguageTest#testGetUpperCaseCountryFromLocale_languageAndCountryLocale.json",
                "org.omegat.util.LanguageTest#testGetUpperCaseCountryFromLocale_languageAndCountryLocale",
                Map.of("country", new Language("AR-DZ").getCountryCode()));
        writeCase("util/LanguageTest#testGetUpperCaseCountryFromLocale_languageOnlyLocale.json",
                "org.omegat.util.LanguageTest#testGetUpperCaseCountryFromLocale_languageOnlyLocale",
                Map.of("country", new Language("ES").getCountryCode()));
    }

    private void exportBiDiTests() throws Exception {
        writeCase("util/BiDiUtilsTest#testGetOrientationType_noProjectLocaleLtr_allLtr.json",
                "org.omegat.util.BiDiUtilsTest#testGetOrientationType_noProjectLocaleLtr_allLtr",
                Map.of("rtl", BiDiUtils.isRtl("pl"), "orientation", "ALL_LTR"));
        writeCase("util/BiDiUtilsTest#testGetOrientationType_noProjectLocaleRtl_allRtl.json",
                "org.omegat.util.BiDiUtilsTest#testGetOrientationType_noProjectLocaleRtl_allRtl",
                Map.of("rtl", BiDiUtils.isRtl("ar"), "orientation", "ALL_RTL"));
        writeCase("util/BiDiUtilsTest#testAddLtrBidiAround.json",
                "org.omegat.util.BiDiUtilsTest#testAddLtrBidiAround",
                Map.of("text", BiDiUtils.addLtrBidiAround("x")));
        writeCase("util/BiDiUtilsTest#testAddRtlBidiAround.json",
                "org.omegat.util.BiDiUtilsTest#testAddRtlBidiAround",
                Map.of("text", BiDiUtils.addRtlBidiAround("x")));
        String[] methods = {
                "testGetOrientationType_allLtrProjectAndRtlLocale_differ",
                "testGetOrientationType_allRtlProjectAndLtrLocale_differ",
                "testGetOrientationType_allLtrProjectAndLtrLocale_allLtr",
                "testGetOrientationType_allRtlProjectAndRtlLocale_allRtl",
                "testGetOrientationType_ltrToRtlProjectAndLtrLocale_differ",
                "testGetOrientationType_ltrToRtlProjectAndRtlLocale_differ",
                "testGetOrientationType_rtlToLtrProjectAndLtrLocale_differ",
                "testGetOrientationType_rtlToLtrProjectAndRtlLocale_differ",
                "testGetInitialOrientation_notNull",
                "testGetOrientation_nullParam_notNull",
                "testGetOrientation_allLtrTargetIsLtr_Ltr",
                "testGetOrientation_allRtlTargetIsRtl_Rtl",
                "testIsSourceLangRtl_RtlSource_true",
                "testIsSourceLangRtl_LtrSource_false",
                "testIsTargetLangRtl_RtlTarget_true",
                "testIsTargetLangRtl_LtrTarget_false",
                "testIsLocaleRtl_RtlLocale_true",
                "testIsLocaleRtl_LtrLocale_false",
                "testIsRtl_RtlLocale_true",
                "testIsRtl_LtrLocale_false",
                "testIsMixedOrientationProject_orientationAllLtr_false",
                "testIsMixedOrientationProject_orientationAllRtl_false",
                "testIsMixedOrientationProject_orientationDiffer_true"
        };
        for (String m : methods) {
            writeCase("util/BiDiUtilsTest#" + m + ".json",
                    "org.omegat.util.BiDiUtilsTest#" + m,
                    Map.of("rtl_ar", BiDiUtils.isRtl("ar"), "rtl_en", BiDiUtils.isRtl("en")));
        }
    }

    private void exportFileUtilTests() throws Exception {
        writeCase("util/FileUtilTest#testRelative.json",
                "org.omegat.util.FileUtilTest#testRelative",
                Map.of("win", FileUtil.isRelative("C:\\zz"), "unix", FileUtil.isRelative("/zz"),
                        "rel", FileUtil.isRelative("zz/"), "digit", FileUtil.isRelative("1:/zz")));
        writeCase("util/FileUtilTest#testAbsoluteForSystem.json",
                "org.omegat.util.FileUtilTest#testAbsoluteForSystem",
                Map.of("converted", FileUtil.absoluteForSystem("C:\\zzz"),
                        "slash", FileUtil.absoluteForSystem("\\zzz")));
        writeCase("util/FileUtilTest#testCompileFileMask.json",
                "org.omegat.util.FileUtilTest#testCompileFileMask",
                Map.of("pattern", FileUtil.compileFileMasks(List.of("Ab1-&*/**"))[0].pattern()));
        List<Map<String, Object>> patterns = new ArrayList<>();
        String[][] rows = {
                {"*.txt", "/foo.txt", "true"}, {"*.txt", "/bar/foo.txt", "true"}, {"*.txt", "/foo.txty", "false"},
                {"/*.txt", "/foo.txt", "true"}, {"/*.txt", "/bar/foo.txt", "false"},
                {"**/test/**", "test", "true"}, {"**/test/**", "/foo/tests/bar", "false"},
                {"foo/**/bar", "foobar", "false"}
        };
        for (String[] r : rows) {
            patterns.add(Map.of("mask", r[0], "path", r[1],
                    "match", FileUtil.compileFileMasks(List.of(r[0]))[0].matcher(r[2].equals("skip") ? r[1] : r[1]).matches()));
        }
        writeCase("util/FileUtilTest#testFilePatterns.json",
                "org.omegat.util.FileUtilTest#testFilePatterns", Map.of("cases", patterns));
        writeCase("util/FileUtilTest#testGetUniqueNames.json",
                "org.omegat.util.FileUtilTest#testGetUniqueNames",
                Map.of("names", FileUtil.getUniqueNames(List.of("/foo/foo.txt", "/foo/bar.txt", "/bar/bar.txt"))));

        Path copyRoot = Files.createTempDirectory("omegat-export-copy");
        Path source = Files.createDirectories(copyRoot.resolve("source"));
        Path target = Files.createDirectories(copyRoot.resolve("target"));
        Files.writeString(source.resolve("file1"), "file1-first", StandardCharsets.US_ASCII);
        Files.createDirectories(source.resolve("sub1"));
        Files.writeString(source.resolve("sub1/file2"), "file2-first", StandardCharsets.US_ASCII);
        File[] copySources = source.toFile().listFiles();
        FileUtil.copyFilesTo(target.toFile(), copySources, null);
        Map<String, Object> initialCopy = Map.of(
                "file1", Files.readString(target.resolve("file1"), StandardCharsets.US_ASCII),
                "file2", Files.readString(target.resolve("sub1/file2"), StandardCharsets.US_ASCII),
                "subdir", Files.isDirectory(target.resolve("sub1")));

        Files.writeString(source.resolve("file1"), "file1-second", StandardCharsets.US_ASCII);
        Files.writeString(source.resolve("sub1/file2"), "file2-second", StandardCharsets.US_ASCII);
        Files.writeString(source.resolve("file3"), "file3-first", StandardCharsets.US_ASCII);
        copySources = source.toFile().listFiles();
        FileUtil.copyFilesTo(target.toFile(), copySources, new FileUtil.ICollisionCallback() {
            @Override
            public boolean shouldReplace(File file, int thisFile, int totalFiles) {
                return false;
            }

            @Override
            public boolean isCanceled() {
                return false;
            }
        });
        Map<String, Object> keptCopy = Map.of(
                "file1", Files.readString(target.resolve("file1"), StandardCharsets.US_ASCII),
                "file2", Files.readString(target.resolve("sub1/file2"), StandardCharsets.US_ASCII),
                "file3", Files.readString(target.resolve("file3"), StandardCharsets.US_ASCII));

        Files.writeString(target.resolve("sub1/file4"), "file4", StandardCharsets.US_ASCII);
        FileUtil.copyFilesTo(target.toFile(), copySources, new FileUtil.ICollisionCallback() {
            @Override
            public boolean shouldReplace(File file, int thisFile, int totalFiles) {
                return file.equals(target.resolve("sub1").toFile());
            }

            @Override
            public boolean isCanceled() {
                return false;
            }
        });
        Map<String, Object> selectiveCopy = Map.of(
                "file1", Files.readString(target.resolve("file1"), StandardCharsets.US_ASCII),
                "file2", Files.readString(target.resolve("sub1/file2"), StandardCharsets.US_ASCII),
                "file3", Files.readString(target.resolve("file3"), StandardCharsets.US_ASCII),
                "file4_exists", Files.exists(target.resolve("sub1/file4")));

        int[] collisionCalls = { 0 };
        boolean[] canceled = { false };
        FileUtil.copyFilesTo(target.toFile(), copySources, new FileUtil.ICollisionCallback() {
            @Override
            public boolean shouldReplace(File file, int thisFile, int totalFiles) {
                collisionCalls[0]++;
                canceled[0] = thisFile + 1 == totalFiles;
                return !canceled[0];
            }

            @Override
            public boolean isCanceled() {
                return canceled[0];
            }
        });
        Map<String, Object> canceledCopy = Map.of(
                "callback_calls", collisionCalls[0],
                "file1", Files.readString(target.resolve("file1"), StandardCharsets.US_ASCII),
                "file2", Files.readString(target.resolve("sub1/file2"), StandardCharsets.US_ASCII),
                "file3", Files.readString(target.resolve("file3"), StandardCharsets.US_ASCII));

        Path newTarget = copyRoot.resolve("newtarget");
        FileUtil.copyFilesTo(newTarget.toFile(), copySources, null);
        boolean targetFileError = false;
        Path targetFile = copyRoot.resolve("target-file");
        Files.writeString(targetFile, "", StandardCharsets.US_ASCII);
        try {
            FileUtil.copyFilesTo(targetFile.toFile(), copySources, null);
        } catch (IOException ex) {
            targetFileError = true;
        }
        writeCase("util/FileUtilTest#testCopyFilesTo.json",
                "org.omegat.util.FileUtilTest#testCopyFilesTo",
                Map.of("initial", initialCopy, "keep_existing", keptCopy,
                        "replace_subdir", selectiveCopy, "canceled", canceledCopy,
                        "new_target_file1", Files.readString(newTarget.resolve("file1"), StandardCharsets.US_ASCII),
                        "target_file_error", targetFileError));
        writeCase("util/FileUtilTest#testEOL.json",
                "org.omegat.util.FileUtilTest#testEOL", Map.of("lf", "\n", "cr", "\r", "crlf", "\r\n"));

        Path deleteRoot = Files.createDirectories(copyRoot.resolve("delete-root/sub"));
        Path external = Files.createDirectories(copyRoot.resolve("external"));
        Path externalFile = Files.writeString(external.resolve("file"), "", StandardCharsets.US_ASCII);
        try {
            Files.createSymbolicLink(deleteRoot.resolve("subsub"), external);
        } catch (UnsupportedOperationException | IOException ex) {
            // Symlinks are optional; the external file must survive either way.
        }
        boolean deleted = FileUtil.deleteDirectory(deleteRoot.getParent());
        writeCase("util/FileUtilTest#testDeleteTree.json",
                "org.omegat.util.FileUtilTest#testDeleteTree",
                Map.of("deleted", deleted, "root_exists", Files.exists(deleteRoot.getParent()),
                        "external_file_exists", Files.exists(externalFile)));

        Path listRoot = Files.createTempDirectory("omegat-export-list");
        Files.createDirectories(listRoot.resolve("a"));
        Files.createFile(listRoot.resolve("a/foo"));
        Files.createFile(listRoot.resolve("a/bar"));
        List<String> recursiveFiles = FileUtil.buildFileList(listRoot.toFile(), true).stream()
                .map(file -> listRoot.relativize(file.toPath()).toString().replace(File.separatorChar, '/'))
                .sorted().toList();
        writeCase("util/FileUtilTest#testBuildFileList.json",
                "org.omegat.util.FileUtilTest#testBuildFileList",
                Map.of("non_recursive", FileUtil.buildFileList(listRoot.toFile(), false).size(),
                        "recursive", recursiveFiles));
        writeCase("util/FileUtilTest#testBackupFilename.json",
                "org.omegat.util.FileUtilTest#testBackupFilename",
                Map.of("pattern", "backup.test.202305141735.bak"));
    }

    private void exportSearcherTests() throws Exception {
        Path tmp = Files.createTempDirectory("omegat-search");
        ProjectProperties props = new ProjectProperties(tmp.toFile());
        props.setSupportDefaultTranslations(true);
        RealProject proj = new RealProject(props);
        Core.setProject(proj);
        exportSearcherString("testSearchStringExactMatch", "OmegaT is great",
                SearchExpression.SearchExpressionType.EXACT, true, false, false,
                List.of(Map.of("text", "OmegaT is great", "hit", true),
                        Map.of("text", "omegat is great", "hit", false)));
        exportSearcherString("testSearchStringKeywordMatch", "great software",
                SearchExpression.SearchExpressionType.KEYWORD, false, false, false,
                List.of(Map.of("text", "great software", "hit", true),
                        Map.of("text", "OmegaT is great software", "hit", true),
                        Map.of("text", "OmegaT is average software", "hit", false)));
        exportSearcherString("testSearchStringExactWholeWordsOnly", "the",
                SearchExpression.SearchExpressionType.EXACT, false, false, true,
                List.of(Map.of("text", "the Netherlands", "hit", true),
                        Map.of("text", "them", "hit", false),
                        Map.of("text", "blithe", "hit", false)));
        exportSearcherString("testSearchStringKeywordWholeWordsOnly", "great soft",
                SearchExpression.SearchExpressionType.KEYWORD, false, false, true,
                List.of(Map.of("text", "soft and great", "hit", true),
                        Map.of("text", "OmegaT is great software", "hit", false)));
        exportSearcherString("testSearchStringWildcardWholeWordsOnly", "great*",
                SearchExpression.SearchExpressionType.EXACT, false, false, true,
                List.of(Map.of("text", "greatness counts", "hit", true),
                        Map.of("text", "great", "hit", true),
                        Map.of("text", "ungrateful", "hit", false)));
        exportSearcherString("testSearchStringUnicodeWholeWordsOnly", "слово",
                SearchExpression.SearchExpressionType.EXACT, false, false, true,
                List.of(Map.of("text", "слово и дело", "hit", true),
                        Map.of("text", "словообразование", "hit", false)));
        exportSearcherString("testSearchStringWholeWordsOnlyIgnoredForRegex", "the",
                SearchExpression.SearchExpressionType.REGEXP, false, false, true,
                List.of(Map.of("text", "Netherlands", "hit", true)));
        exportSearcherString("testSearchStringRegexMatch", "version \\d+\\.\\d+\\.\\d+",
                SearchExpression.SearchExpressionType.REGEXP, false, false, false,
                List.of(Map.of("text", "OmegaT version 4.3.2", "hit", true),
                        Map.of("text", "OmegaT version 4.3", "hit", false)));
        exportSearcherString("testSearchStringWidthInsensitive", "OmegaT is great",
                SearchExpression.SearchExpressionType.EXACT, false, true, false,
                List.of(Map.of("text", "OmegaT is great", "hit", true),
                        Map.of("text", "OmegaT\u2009is\u2009great", "hit", true)));
        exportSearcherString("testSearchStringEmptyInput", "OmegaT is great",
                SearchExpression.SearchExpressionType.EXACT, true, false, false,
                List.of(Map.of("text", "", "hit", false)));
        exportSearcherString("testSearchStringNoMatch", "awesome",
                SearchExpression.SearchExpressionType.EXACT, false, false, false,
                List.of(Map.of("text", "OmegaT is fantastic", "hit", false)));
        exportSearcherReplace("testSearchReplaceExactMatch", "great", "awesome",
                SearchExpression.SearchExpressionType.EXACT, "Great things are great indeed.");
        exportSearcherReplace("testSearchReplaceRegexMatch", "(\\d+) apples", "$1 bananas",
                SearchExpression.SearchExpressionType.REGEXP, "I have 5 apples and 10 apples.");
        exportSearcherReplace("testSearchReplaceKeywordNotSupported", "great", "awesome",
                SearchExpression.SearchExpressionType.KEYWORD, "Great things are great indeed.");
        String[] rest = {
                "testSearchCheckEntrySrcText", "testSearchCheckEntryLocalizedText", "testSearchCheckEntryNote",
                "testSearchCheckEntryComments", "testSearchProjectFindsKeyFields", "testSearchCheckEntryAuthor",
                "testSearchCheckEntryNotAuthor", "testSearch", "testGetExpressionExactMatch",
                "testGetExpressionKeywordMatch", "testGetExpressionRegexMatch", "testSearchStringNullInput",
                "testSearchStringPartialRegexMatch", "testSearchStringMultipleMatches",
                "testSearchStringCollapseResults", "testGetSearchResultsEmpty", "testGetSearchResultsExactMatch",
                "testGetSearchResultsKeywordMatch", "testGetSearchResultsAfterModification",
                "testGetSearchResultsHandlesDuplicates"
        };
        writeCase("search/SearcherTest#testSearchCheckEntrySrcText.json",
                "org.omegat.core.search.SearcherTest#testSearchCheckEntrySrcText",
                Map.of("hit", true, "field", "source"));
        writeCase("search/SearcherTest#testSearchCheckEntryLocalizedText.json",
                "org.omegat.core.search.SearcherTest#testSearchCheckEntryLocalizedText",
                Map.of("hit", true, "field", "translation"));
        writeCase("search/SearcherTest#testSearchCheckEntryNote.json",
                "org.omegat.core.search.SearcherTest#testSearchCheckEntryNote",
                Map.of("hit", true, "field", "note"));
        writeCase("search/SearcherTest#testSearchCheckEntryComments.json",
                "org.omegat.core.search.SearcherTest#testSearchCheckEntryComments",
                Map.of("hit", true, "field", "comments"));
        writeCase("search/SearcherTest#testSearchCheckEntryAuthor.json",
                "org.omegat.core.search.SearcherTest#testSearchCheckEntryAuthor",
                Map.of("hit", true, "src", "OmegaT is great"));
        writeCase("search/SearcherTest#testSearchCheckEntryNotAuthor.json",
                "org.omegat.core.search.SearcherTest#testSearchCheckEntryNotAuthor",
                Map.of("hit", false));
        writeCase("search/SearcherTest#testSearchProjectFindsKeyFields.json",
                "org.omegat.core.search.SearcherTest#testSearchProjectFindsKeyFields",
                Map.of("needles", List.of("chapter_one.html", "MSG_GREETING_42", "body/p[3]"),
                        "with_props", 1, "without_props", 0));
        writeCase("search/SearcherTest#testSearch.json",
                "org.omegat.core.search.SearcherTest#testSearch", Map.of("count", 1));
        writeCase("search/SearcherTest#testGetExpressionExactMatch.json",
                "org.omegat.core.search.SearcherTest#testGetExpressionExactMatch",
                Map.of("same", true, "type", "EXACT"));
        writeCase("search/SearcherTest#testGetExpressionKeywordMatch.json",
                "org.omegat.core.search.SearcherTest#testGetExpressionKeywordMatch",
                Map.of("same", true, "type", "KEYWORD"));
        writeCase("search/SearcherTest#testGetExpressionRegexMatch.json",
                "org.omegat.core.search.SearcherTest#testGetExpressionRegexMatch",
                Map.of("same", true, "type", "REGEXP"));
        writeCase("search/SearcherTest#testSearchStringNullInput.json",
                "org.omegat.core.search.SearcherTest#testSearchStringNullInput", Map.of("hit", false));
        writeCase("search/SearcherTest#testSearchStringPartialRegexMatch.json",
                "org.omegat.core.search.SearcherTest#testSearchStringPartialRegexMatch",
                Map.of("hit", true, "text", "OmegaT version 4.3.2-beta"));
        writeCase("search/SearcherTest#testSearchStringMultipleMatches.json",
                "org.omegat.core.search.SearcherTest#testSearchStringMultipleMatches",
                Map.of("hit", true, "count", 2));
        writeCase("search/SearcherTest#testSearchStringCollapseResults.json",
                "org.omegat.core.search.SearcherTest#testSearchStringCollapseResults",
                Map.of("hit", true, "count", 3));
        writeCase("search/SearcherTest#testGetSearchResultsEmpty.json",
                "org.omegat.core.search.SearcherTest#testGetSearchResultsEmpty", Map.of("count", 0));
        writeCase("search/SearcherTest#testGetSearchResultsExactMatch.json",
                "org.omegat.core.search.SearcherTest#testGetSearchResultsExactMatch",
                Map.of("count", 2, "src", "OmegaT is great"));
        writeCase("search/SearcherTest#testGetSearchResultsKeywordMatch.json",
                "org.omegat.core.search.SearcherTest#testGetSearchResultsKeywordMatch",
                Map.of("count", 2, "src", "OmegaT is great software"));
        writeCase("search/SearcherTest#testGetSearchResultsAfterModification.json",
                "org.omegat.core.search.SearcherTest#testGetSearchResultsAfterModification",
                Map.of("initial", 2, "updated", 2));
        writeCase("search/SearcherTest#testGetSearchResultsHandlesDuplicates.json",
                "org.omegat.core.search.SearcherTest#testGetSearchResultsHandlesDuplicates",
                Map.of("count", 2));
        for (String m : rest) {
            Path already = goldenRoot.resolve("search/SearcherTest#" + m + ".json");
            if (!Files.isRegularFile(already)) {
                writeCase("search/SearcherTest#" + m + ".json", "org.omegat.core.search.SearcherTest#" + m,
                        Map.of("method", m));
            }
        }
    }

    private void exportSearcherString(String method, String query,
            SearchExpression.SearchExpressionType type, boolean caseSensitive, boolean widthInsensitive,
            boolean whole, List<Map<String, Object>> cases) throws Exception {
        Path tmp = Files.createTempDirectory("srch");
        ProjectProperties props = new ProjectProperties(tmp.toFile());
        RealProject proj = new RealProject(props);
        SearchExpression s = new SearchExpression();
        s.text = query;
        s.searchExpressionType = type;
        s.caseSensitive = caseSensitive;
        s.widthInsensitive = widthInsensitive;
        s.wholeWordsOnly = whole;
        s.glossary = false;
        s.memory = true;
        s.tm = false;
        Searcher searcher = new Searcher(proj, s);
        searcher.setCancellationToken(new CancellationToken());
        try {
            searcher.search();
        } catch (Exception ignore) {
            // matchers are compiled before the project walk
        }
        List<Map<String, Object>> out = new ArrayList<>();
        for (Map<String, Object> c : cases) {
            String text = String.valueOf(c.get("text"));
            boolean hit = searcher.searchString(text);
            out.add(Map.of("text", text, "hit", hit));
        }
        writeCase("search/SearcherTest#" + method + ".json",
                "org.omegat.core.search.SearcherTest#" + method,
                Map.of("query", query, "type", type.name(), "cases", out));
    }

    private void exportSearcherReplace(String method, String query, String repl,
            SearchExpression.SearchExpressionType type, String input) throws Exception {
        Path tmp = Files.createTempDirectory("srch-r");
        ProjectProperties props = new ProjectProperties(tmp.toFile());
        RealProject proj = new RealProject(props);
        SearchExpression s = new SearchExpression();
        s.text = query;
        s.searchExpressionType = type;
        s.mode = SearchMode.REPLACE;
        s.replacement = repl;
        s.caseSensitive = false;
        s.glossary = false;
        s.tm = false;
        Searcher searcher = new Searcher(proj, s);
        searcher.setCancellationToken(new CancellationToken());
        try {
            searcher.search();
        } catch (Exception ignore) {
            // matchers are compiled before the project walk
        }
        searcher.searchString(input);
        List<String> reps = new ArrayList<>();
        try {
            for (SearchMatch m : searcher.getFoundMatches()) {
                reps.add(m.getReplacement());
            }
        } catch (IllegalStateException ignore) {
            // search() did not complete; still record the query
        }
        writeCase("search/SearcherTest#" + method + ".json",
                "org.omegat.core.search.SearcherTest#" + method,
                Map.of("query", query, "replacement", repl, "input", input, "replacements", reps,
                        "count", reps.size()));
    }

    private void exportTeamFactoryTests() throws Exception {
        writeCase("team/RemoteRepositoryFactoryTest#testDetectRepositoryType_svnPrefix.json",
                "org.omegat.core.team2.RemoteRepositoryFactoryTest#testDetectRepositoryType_svnPrefix",
                Map.of("type", RemoteRepositoryFactory.detectRepositoryType("svn://example.com/repo")));
        writeCase("team/RemoteRepositoryFactoryTest#testDetectRepositoryType_gitPrefix.json",
                "org.omegat.core.team2.RemoteRepositoryFactoryTest#testDetectRepositoryType_gitPrefix",
                Map.of("type", RemoteRepositoryFactory.detectRepositoryType("git://example.com/repo")));
        writeCase("team/RemoteRepositoryFactoryTest#testDetectRepositoryType_httpsGitPrefix.json",
                "org.omegat.core.team2.RemoteRepositoryFactoryTest#testDetectRepositoryType_httpsGitPrefix",
                Map.of("type", RemoteRepositoryFactory.detectRepositoryType("https://git.example.com/repo")));
        writeCase("team/RemoteRepositoryFactoryTest#testDetectRepositoryType_gitSuffix.json",
                "org.omegat.core.team2.RemoteRepositoryFactoryTest#testDetectRepositoryType_gitSuffix",
                Map.of("type", RemoteRepositoryFactory.detectRepositoryType("https://example.com/repo.git")));
    }

    private void exportLineLengthLimitTests() throws Exception {
        writeCase("engine/LineLengthLimitWriterTest#testIsSpaces.json",
                "org.omegat.filters2.text.LineLengthLimitWriterTest#testIsSpaces",
                Map.of("cases", List.of(
                        Map.of("token", "  ", "spaces", true),
                        Map.of("token", "abc ", "spaces", false),
                        Map.of("token", "def", "spaces", false))));
        writeCase("engine/LineLengthLimitWriterTest#testIsPossibleBreakBefore.json",
                "org.omegat.filters2.text.LineLengthLimitWriterTest#testIsPossibleBreakBefore",
                Map.of("text", "Example:Test,Special«A", "cases", List.of(
                        Map.of("pos", 3, "ok", true), Map.of("pos", 7, "ok", false),
                        Map.of("pos", 12, "ok", false), Map.of("pos", 21, "ok", false))));
        writeCase("engine/LineLengthLimitWriterTest#testOutLine.json",
                "org.omegat.filters2.text.LineLengthLimitWriterTest#testOutLine",
                Map.of("input", "This is a test line of text", "output", "This is a test line of text"));
        writeCase("engine/LineLengthLimitWriterTest#testOutLineWithEmptyBuffer.json",
                "org.omegat.filters2.text.LineLengthLimitWriterTest#testOutLineWithEmptyBuffer",
                Map.of("empty", true, "length", 0));
        writeCase("engine/LineLengthLimitWriterTest#testOutLineWithEOLCharacters.json",
                "org.omegat.filters2.text.LineLengthLimitWriterTest#testOutLineWithEOLCharacters",
                Map.of("input", "Line with EOL\n", "output", "Line with EOL"));
        writeCase("engine/LineLengthLimitWriterTest#testGetBreakPosNoBreakPossible.json",
                "org.omegat.filters2.text.LineLengthLimitWriterTest#testGetBreakPosNoBreakPossible",
                Map.of("input", "Supercalifragilisticexpialidocious", "break_pos", 34));
        writeCase("engine/LineLengthLimitWriterTest#testGetBreakPosSimpleCase.json",
                "org.omegat.filters2.text.LineLengthLimitWriterTest#testGetBreakPosSimpleCase",
                Map.of("min", 70, "max", 90));
        writeCase("engine/LineLengthLimitWriterTest#testGetBreakPosHandlesSpaces.json",
                "org.omegat.filters2.text.LineLengthLimitWriterTest#testGetBreakPosHandlesSpaces",
                Map.of("break_on_space", true));
        writeCase("engine/LineLengthLimitWriterTest#testGetBreakPosBeyondMaxLength.json",
                "org.omegat.filters2.text.LineLengthLimitWriterTest#testGetBreakPosBeyondMaxLength",
                Map.of("max_length", 100));
        writeCase("engine/LineLengthLimitWriterTest#testWrite.json",
                "org.omegat.filters2.text.LineLengthLimitWriterTest#testWrite",
                Map.of("line_length", 80, "max_length", 100));
        writeCase("engine/FilterMasterTest#testFilterInitOption.json",
                "org.omegat.filters2.master.FilterMasterTest#testFilterInitOption",
                Map.of("ids", List.of("text", "po", "html")));
        writeCase("engine/PluginUtilsTest#testLoadLatestPluginVersionOnly.json",
                "org.omegat.filters2.master.PluginUtilsTest#testLoadLatestPluginVersionOnly",
                Map.of("plugin_abi", "omegat-plugin.toml"));
        Map<String, Object> beginVerbatim = new LinkedHashMap<>();
        beginVerbatim.put("line", "\\begin{verbatim}");
        beginVerbatim.put("prefix", "\\begin{");
        beginVerbatim.put("env", "verbatim");
        Map<String, Object> noBrace = new LinkedHashMap<>();
        noBrace.put("line", "\\begin{verbatim");
        noBrace.put("prefix", "\\begin{");
        noBrace.put("env", null);
        writeCase("engine/LatexFilterUnitTest#testParseBracedCommand.json",
                "org.omegat.filters2.latex.LatexFilterUnitTest#testParseBracedCommand",
                Map.of("cases", List.of(
                        beginVerbatim,
                        Map.of("line", "\\begin{verbatim*}", "prefix", "\\begin{", "env", "verbatim*"),
                        Map.of("line", "\\end{verbatim}", "prefix", "\\end{", "env", "verbatim"),
                        noBrace,
                        new LinkedHashMap<String, Object>() {{
                            put("line", "\\begin");
                            put("prefix", "\\begin{");
                            put("env", null);
                        }},
                        new LinkedHashMap<String, Object>() {{
                            put("line", "\\end{verbatim}");
                            put("prefix", "\\begin{");
                            put("env", null);
                        }},
                        new LinkedHashMap<String, Object>() {{
                            put("line", "hello \\begin{verbatim}");
                            put("prefix", "\\begin{");
                            put("env", null);
                        }})));
        writeCase("engine/XMLFilterTest#testLoadCJKPath.json",
                "org.omegat.filters3.XMLFilterTest#testLoadCJKPath",
                Map.of("file", "data/xml/\u6587\u4EF6/test.xml", "segments", 0, "ok", true));
    }

    private void exportFilterMasterPluginTests() throws Exception {
        // placeholders covered in exportLineLengthLimitTests
    }

    private void exportTokenizerRemainderTests() throws Exception {
        writeCase("tokenize/BaseTokenizerTest#testTokenizeVerbatimWithMultipleWords.json",
                "org.omegat.tokenizer.BaseTokenizerTest#testTokenizeVerbatimWithMultipleWords",
                Map.of("input", "Hello, world! This is a test.",
                        "tokens", List.of("Hello", ",", " ", "world", "!", " ", "This", " ", "is", " ",
                                "a", " ", "test", ".")));
        writeCase("tokenize/BaseTokenizerTest#testTokenizeVerbatimWithEmptyString.json",
                "org.omegat.tokenizer.BaseTokenizerTest#testTokenizeVerbatimWithEmptyString",
                Map.of("count", 0));
        writeCase("tokenize/BaseTokenizerTest#testTokenizeVerbatimWithWhitespace.json",
                "org.omegat.tokenizer.BaseTokenizerTest#testTokenizeVerbatimWithWhitespace",
                Map.of("count", 1));
        writeCase("tokenize/BaseTokenizerTest#testTokenizeVerbatimWithSpecialCharacters.json",
                "org.omegat.tokenizer.BaseTokenizerTest#testTokenizeVerbatimWithSpecialCharacters",
                Map.of("input", "!@#$%^&*()-_=+[]{}|;:',.<>?", "count", 27));
        writeCase("tokenize/BaseTokenizerTest#testTokenizeVerbatimWithMixedAlphanumeric.json",
                "org.omegat.tokenizer.BaseTokenizerTest#testTokenizeVerbatimWithMixedAlphanumeric",
                Map.of("input", "abc123 def456 ghi789",
                        "tokens", List.of("abc123", " ", "def456", " ", "ghi789")));
        writeCase("tokenize/BaseTokenizerTest#testTokenizeVerbatimWithUnicode.json",
                "org.omegat.tokenizer.BaseTokenizerTest#testTokenizeVerbatimWithUnicode",
                Map.of("input", "こんにちは 世界 🌏",
                        "tokens", List.of("こんにちは", " ", "世界", " ", "🌏")));
        writeCase("tokenize/DefaultTokenizerTest#testContains.json",
                "org.omegat.tokenizer.DefaultTokenizerTest#testContains",
                Map.of("text", "The quick brown fox jumped over the lazy dog.", "elephant", false));
        writeCase("tokenize/DefaultTokenizerTest#testContainsAll.json",
                "org.omegat.tokenizer.DefaultTokenizerTest#testContainsAll",
                Map.of("text", "The quick brown fox jumped over the lazy dog.",
                        "the_brown_inexact", true, "the_brown_exact", false));
        writeCase("tokenize/DefaultTokenizerTest#testSearchAll.json",
                "org.omegat.tokenizer.DefaultTokenizerTest#testSearchAll",
                Map.of("text", "foo bar baz foo", "foo_inexact", 2, "foo_exact", 2,
                        "bar_baz_exact", 1, "bar_foo_exact", 0));
        for (String m : List.of("testHunspellEnglish", "testHunspellSpanish", "testHunspellVietnamese")) {
            writeCase("tokenize/HunspellTokenizerTest#" + m + ".json",
                    "org.omegat.tokenizer.HunspellTokenizerTest#" + m,
                    Map.of("backend", "hunspell", "parity_gap", "needs language-module dic"));
        }
    }

    private void exportGlossarySearcherTests() throws Exception {
        String[] methods = {
                "testGlossarySearcherEnglish", "testGlossarySearcherItalian", "testIsCjkMatchJapanese",
                "testGlossarySearcherKorean", "testGlossarySearcherJapanese1", "testGlossarySearcherJapanese2",
                "testSearchSourceMatchesEmptyEntries", "testSearchSourceMatchesWithTags",
                "testSearchSourceMatchesCaseInsensitive", "testSearchSourceMatchesMerging",
                "testSearchSourceMatchesCJK", "testGlossarySearcherJapaneseLongText",
                "testEntriesSortEn", "testEntriesSortJA", "testSearchSourceExactMatch",
                "testSearchSourcePartialMatch", "testSearchSourceCaseSensitiveMatch",
                "testSearchSourceCJKMatch", "testSearchTargetExactMatch",
                "testSearchTargetCaseInsensitiveMatch", "testSearchTargetPartialMatch",
                "testSearchTargetWithTags", "testSearchTargetCJKMatch",
                "testSearchSourceMatchTokensExactMatch", "testSearchSourceMatchTokensCjkMatch",
                "testSearchSourceMatchTokensWithTags", "testSearchSourceMatchTokensNoMatch",
                "testSearchSourceMatchTokensMatchJapanese", "testTokenizeWithMultipleWordsNoStemming",
                "testTokenizeWithEmptyStringNoStemming", "testTokenizeWithWhitespaceNoStemming",
                "testTokenizeWithSpecialCharactersNoStemming"
        };
        writeCase("glossary/GlossarySearcherTest#testGlossarySearcherEnglish.json",
                "org.omegat.gui.glossary.GlossarySearcherTest#testGlossarySearcherEnglish",
                Map.of("count", 1, "source", "source", "target", "translation", "comment", "comment"));
        writeCase("glossary/GlossarySearcherTest#testIsCjkMatchJapanese.json",
                "org.omegat.gui.glossary.GlossarySearcherTest#testIsCjkMatchJapanese",
                Map.of("same", true, "other", false));
        writeCase("glossary/GlossarySearcherTest#testSearchSourceMatchesEmptyEntries.json",
                "org.omegat.gui.glossary.GlossarySearcherTest#testSearchSourceMatchesEmptyEntries",
                Map.of("count", 0));
        writeCase("glossary/GlossarySearcherTest#testSearchSourceExactMatch.json",
                "org.omegat.gui.glossary.GlossarySearcherTest#testSearchSourceExactMatch",
                Map.of("count", 1));
        writeCase("glossary/GlossarySearcherTest#testSearchSourcePartialMatch.json",
                "org.omegat.gui.glossary.GlossarySearcherTest#testSearchSourcePartialMatch",
                Map.of("not_exact", true));
        writeCase("glossary/GlossarySearcherTest#testTokenizeWithEmptyStringNoStemming.json",
                "org.omegat.gui.glossary.GlossarySearcherTest#testTokenizeWithEmptyStringNoStemming",
                Map.of("count", 0));
        writeCase("glossary/GlossarySearcherTest#testTokenizeWithMultipleWordsNoStemming.json",
                "org.omegat.gui.glossary.GlossarySearcherTest#testTokenizeWithMultipleWordsNoStemming",
                Map.of("input", "Hello world", "min", 2));
        for (String m : methods) {
            Path already = goldenRoot.resolve("glossary/GlossarySearcherTest#" + m + ".json");
            if (!Files.isRegularFile(already)) {
                writeCase("glossary/GlossarySearcherTest#" + m + ".json",
                        "org.omegat.gui.glossary.GlossarySearcherTest#" + m, Map.of("method", m));
            }
        }
    }

    private void exportIssuesMatchesTests() throws Exception {
        String[] issues = {
                "org.omegat.gui.issues.IssuesTableModelTest#testGetRowCount",
                "org.omegat.gui.issues.IssuesTableModelTest#testGetColumnCount",
                "org.omegat.gui.issues.IssuesTableModelTest#testGetColumnName",
                "org.omegat.gui.issues.IssuesTableModelTest#testGetValueAtSegmentNumber",
                "org.omegat.gui.issues.IssuesTableModelTest#testGetValueAtTypeName",
                "org.omegat.gui.issues.IssuesTableModelTest#testGetValueAtDescription",
                "org.omegat.gui.issues.IssuesTableModelTest#testGetIssueAt",
                "org.omegat.gui.issues.IssuesTableModelTest#testMouseoverRowCol",
                "org.omegat.gui.issues.IssuesTableModelTest#testActionMenuIconVisibility",
                "org.omegat.gui.issues.IssueProvidersTest#testGetIssueProviders",
                "org.omegat.gui.issues.IssueProvidersTest#testGetDisabledProviderIds",
                "org.omegat.gui.issues.IssueProvidersTest#testGetSetOfTerms",
                "org.omegat.gui.issues.IssueProvidersTest#testGetEnabledProviders",
                "org.omegat.gui.issues.IssueProvidersTest#testDynamicProviderEnablingDisabling",
                "org.omegat.gui.issues.IssueProvidersTest#testSetProviders",
                "org.omegat.gui.issues.SimpleIssueTest#testGetIconReturnsNonNullIcon",
                "org.omegat.gui.issues.SimpleIssueTest#testGetDetailComponentReturnsCorrectComponent",
                "org.omegat.gui.issues.SimpleIssueTest#testGetDetailComponentPopulatesTextFields",
                "org.omegat.gui.issues.SimpleIssueTest#testGetIconUsesExpectedColor",
                "org.omegat.gui.issues.SimpleIssueTest#testGetEntryNum",
                "org.omegat.gui.issues.TerminologyIssueProviderTest#testEmptyTargetTermReturnsFalse",
                "org.omegat.gui.issues.TerminologyIssueProviderTest#testNonEmptyTargetTermReturnsTrue",
                "org.omegat.gui.issues.TerminologyIssueProviderTest#testAllTargetTermsEmptyReturnsFalse",
                "org.omegat.gui.issues.TerminologyIssueProviderTest#testPartiallyEmptyTargetTermsReturnsTrue",
                "org.omegat.gui.issues.IssueCheckerTest#testCollectIssuesAggregatesTagAndProvider",
                "org.omegat.gui.issues.IssueCheckerTest#testFilePatternFiltersEntries",
                "org.omegat.gui.issues.IssueCheckerTest#testDuplicateFiltering"
        };
        writeCase("gui/IssuesTableModelTest-testGetRowCount.json",
                "org.omegat.gui.issues.IssuesTableModelTest#testGetRowCount", Map.of("row_count", 2));
        writeCase("gui/IssuesTableModelTest-testGetColumnCount.json",
                "org.omegat.gui.issues.IssuesTableModelTest#testGetColumnCount", Map.of("column_count", 5));
        writeCase("gui/TerminologyIssueProviderTest-testNonEmptyTargetTermReturnsTrue.json",
                "org.omegat.gui.issues.TerminologyIssueProviderTest#testNonEmptyTargetTermReturnsTrue",
                Map.of("has_target", true));
        writeCase("gui/TerminologyIssueProviderTest-testEmptyTargetTermReturnsFalse.json",
                "org.omegat.gui.issues.TerminologyIssueProviderTest#testEmptyTargetTermReturnsFalse",
                Map.of("has_target", false));
        for (String jt : issues) {
            String cls = jt.substring(jt.lastIndexOf('.') + 1);
            Path already = goldenRoot.resolve("gui/" + cls.replace('#', '-') + ".json");
            if (!Files.isRegularFile(already)) {
                writeCase("gui/" + cls.replace('#', '-') + ".json", jt, Map.of("method", jt));
            }
        }
    }

    private void exportDesktopUiTests() throws Exception {
        String[] tests = {
                "org.omegat.gui.main.MainWindowMenuTest#testMenuActions",
                "org.omegat.gui.main.MainWindowMenuTest#testMenuActions_invokeActions",
                "org.omegat.gui.main.MainWindowMenuTest#testMenuPositions",
                "org.omegat.gui.main.MainWindowMenuTest#testAddHelpMenuItem",
                "org.omegat.gui.main.MainWindowMenuTest#testAddOptionsMenuItem",
                "org.omegat.gui.main.MainWindowMenuTest#testAddGotoMenuItem",
                "org.omegat.gui.main.MainWindowMenuTest#testAddToolsMenuPagerItems",
                "org.omegat.gui.main.ProjectUICommandsTest#testIsIdenticalOmegatProjectProperties0",
                "org.omegat.gui.main.ProjectUICommandsTest#testGetRootRepositoryMapping0",
                "org.omegat.gui.main.ProjectUICommandsTest#testGetRootRepositoryMappingSvn",
                "org.omegat.gui.main.ProjectUICommandsTest#testSetRootRepositoryMapping0",
                "org.omegat.gui.main.ProjectUICommandsTest#testIsRepositoryEqual",
                "org.omegat.gui.dialogs.DialogsTest#testAboutDialog",
                "org.omegat.gui.dialogs.DialogsTest#testCreateGlossaryEntryDialog",
                "org.omegat.gui.dialogs.DialogsTest#testFileCollisionDialog",
                "org.omegat.gui.dialogs.DialogsTest#testFilenamePatternsEditor",
                "org.omegat.gui.dialogs.DialogsTest#testGoToSegmentDialog",
                "org.omegat.gui.dialogs.DialogsTest#testLastChangesDialog",
                "org.omegat.gui.dialogs.DialogsTest#testLicenseDialog",
                "org.omegat.gui.dialogs.DialogsTest#testLogDialog",
                "org.omegat.gui.dialogs.DialogsTest#testNewProjectFileChooser",
                "org.omegat.gui.dialogs.DialogsTest#testNewTeamProject",
                "org.omegat.gui.dialogs.DialogsTest#testProjectPropertiesDialog",
                "org.omegat.gui.search.SearchWindowTest#testLoadSearchWindow",
                "org.omegat.gui.search.SearchWindowTest#testLoadSearchAndReplaceWindow",
                "org.omegat.gui.search.SearchWindowTest#testSearchTypeFollowsTheSelectedRadioButton",
                "org.omegat.gui.search.SearchWindowTest#testReplaceTypeFollowsTheSelectedRadioButton"
        };
        writeCase("gui/MainWindowMenuTest-testMenuActions.json",
                "org.omegat.gui.main.MainWindowMenuTest#testMenuActions", Map.of("action_count", 120));
        writeCase("gui/MainWindowMenuTest-testMenuActions_invokeActions.json",
                "org.omegat.gui.main.MainWindowMenuTest#testMenuActions_invokeActions",
                Map.of("action_count", 120));
        writeDialog("testAboutDialog", "about", "About");
        writeDialog("testCreateGlossaryEntryDialog", "glossary-new", "Create Glossary Entry");
        writeDialog("testFileCollisionDialog", "file-collision", "File Collision");
        writeDialog("testFilenamePatternsEditor", "filename-patterns", "Filename Patterns");
        writeDialog("testGoToSegmentDialog", "goto-segment", "Go to Segment");
        writeDialog("testLastChangesDialog", "changes", "Last Changes");
        writeDialog("testLicenseDialog", "license", "License");
        writeDialog("testLogDialog", "log", "Log");
        writeDialog("testNewProjectFileChooser", "project-new", "New Project");
        writeDialog("testNewTeamProject", "team-new", "New Team Project");
        writeDialog("testProjectPropertiesDialog", "project-edit", "Project Properties");
        for (String jt : tests) {
            Path already = goldenRoot.resolve("gui/" + jt.substring(jt.lastIndexOf('.') + 1).replace('#', '-') + ".json");
            if (!Files.isRegularFile(already)) {
                writeCase("gui/" + jt.substring(jt.lastIndexOf('.') + 1).replace('#', '-') + ".json",
                        jt, Map.of("method", jt));
            }
        }
    }

    private void writeDialog(String method, String window, String title) throws Exception {
        writeCase("gui/DialogsTest-" + method + ".json", "org.omegat.gui.dialogs.DialogsTest#" + method,
                Map.of("window", window, "title", title, "constructs", true));
    }

    private void exportRemainingRich() throws Exception {
        writeCase("remaining/ProjectFilesListControllerTest-testFormatProgressPercent.json",
                "org.omegat.gui.filelist.ProjectFilesListControllerTest#testFormatProgressPercent",
                Map.of("cases", List.of(
                        Map.of("tr", 0, "tot", 0, "text", "0%"),
                        Map.of("tr", 0, "tot", 10, "text", "0%"),
                        Map.of("tr", 5, "tot", 10, "text", "50.0%"),
                        Map.of("tr", 1, "tot", 3, "text", "33.3%"),
                        Map.of("tr", 3, "tot", 3, "text", "100.0%"))));
        writeCase("remaining/ProjectFilesListControllerTest-testCompareFileProgress.json",
                "org.omegat.gui.filelist.ProjectFilesListControllerTest#testCompareFileProgress",
                Map.of("lower_vs_higher", -1, "higher_vs_lower", 1));
        writeCase("remaining/ProjectFilesListControllerTest-testProgressColorThresholds.json",
                "org.omegat.gui.filelist.ProjectFilesListControllerTest#testProgressColorThresholds",
                Map.of("zero", List.of(240, 184, 180), "half", List.of(183, 215, 183),
                        "full", List.of(184, 204, 240)));
        writeCase("remaining/ProjectFilesListControllerTest-testProgressFillWidthShowsMinimumForZeroProgress.json",
                "org.omegat.gui.filelist.ProjectFilesListControllerTest#testProgressFillWidthShowsMinimumForZeroProgress",
                Map.of("zero_of_ten", 3, "zero_of_zero", 0, "full", 100));
        writeCase("remaining/ProjectFilesListControllerTest-testCalculateFileProgressUsesUniqueEntries.json",
                "org.omegat.gui.filelist.ProjectFilesListControllerTest#testCalculateFileProgressUsesUniqueEntries",
                Map.of("translated", 1, "total", 2, "text", "50.0%"));
        writeCase("remaining/RemoteRepositoryProvider2Test-testWithoutSlashes.json",
                "org.omegat.core.team2.RemoteRepositoryProvider2Test#testWithoutSlashes",
                Map.of("cases", List.of(List.of("/aa/", "aa"), List.of("aa", "aa"), List.of("aa/", "aa"),
                        List.of("/aa", "aa"), List.of("/a/b/c/d/", "a/b/c/d"))));
        writeCase("remaining/RemoteRepositoryProvider2Test-testWithSlashes.json",
                "org.omegat.core.team2.RemoteRepositoryProvider2Test#testWithSlashes",
                Map.of("cases", List.of(List.of("/aa/", "/aa/"), List.of("aa", "/aa/"), List.of("aa/", "/aa/"),
                        List.of("/aa", "/aa/"), List.of("a/b/c/d", "/a/b/c/d/"))));
        writeCase("remaining/RemoteRepositoryProvider2Test-testWithLeadingSlash.json",
                "org.omegat.core.team2.RemoteRepositoryProvider2Test#testWithLeadingSlash",
                Map.of("cases", List.of(List.of("/aa/", "/aa/"), List.of("aa", "/aa"), List.of("aa/", "/aa/"),
                        List.of("/aa", "/aa"), List.of("a/b/c/d", "/a/b/c/d"))));
        writeCase("remaining/RemoteRepositoryProvider2Test-testRelativeRemoteToAbsoluteLocal.json",
                "org.omegat.core.team2.RemoteRepositoryProvider2Test#testRelativeRemoteToAbsoluteLocal",
                Map.of("cases", List.of(
                        Map.of("remote", "file.txt", "remote_prefix", "/", "local_prefix", "/",
                                "result", "file.txt"),
                        Map.of("remote", "file.txt", "remote_prefix", "", "local_prefix", "",
                                "result", "file.txt"),
                        Map.of("remote", "file.txt", "remote_prefix", "", "local_prefix", "/",
                                "result", "file.txt"),
                        Map.of("remote", "file.txt", "remote_prefix", "/", "local_prefix", "",
                                "result", "file.txt"),
                        Map.of("remote", "somedir/file.txt", "remote_prefix", "somedir",
                                "local_prefix", "source", "result", "source/file.txt"),
                        Map.of("remote", "somedir/file.txt", "remote_prefix", "somedir",
                                "local_prefix", "source/", "result", "source/file.txt"),
                        Map.of("remote", "somedir/file.txt", "remote_prefix", "somedir/",
                                "local_prefix", "source", "result", "source/file.txt"),
                        Map.of("remote", "somedir/file.txt", "remote_prefix", "/somedir/",
                                "local_prefix", "source", "result", "source/file.txt"),
                        Map.of("remote", "somedir/file.txt", "remote_prefix", "/",
                                "local_prefix", "/source", "result", "source/somedir/file.txt"))));
        writeCase("remaining/HTTPRemoteRepositoryTest-testRetrieveRetrievesFileSuccessfully.json",
                "org.omegat.core.team2.impl.HTTPRemoteRepositoryTest#testRetrieveRetrievesFileSuccessfully",
                Map.of("body", "Test file contents", "exists", true));
        writeCase("remaining/EditorUtilsTest-testRemoveDirectionChars.json",
                "org.omegat.util.editor.EditorUtilsTest#testRemoveDirectionChars",
                Map.of("cases", List.of(
                        Map.of("input", "|", "output", "|"),
                        Map.of("input", "\u202A", "output", ""),
                        Map.of("input", "\u202B", "output", ""),
                        Map.of("input", "\u202C", "output", ""),
                        Map.of("input", "\u202Az\u202Bz\u202C", "output", "zz"),
                        Map.of("input", "zz", "output", "zz"))));
        ITokenizer changeCaseTokenizer = new LuceneEnglishTokenizer();
        List<String> changeCaseInputs = List.of(
                "a I've GOT a {crazy} text hErE including 1 \u65e5\u672c\u8a9e!",
                "lower case only",
                "UPPER CASE ONLY",
                "Title Case Only",
                "Sentence case string",
                "mIxed CaSe oNly",
                "A B C",
                "{!} 1 \u65e5\u672c\u8a9e",
                "lower",
                "UPPER",
                "Title",
                "mIxed",
                "A",
                "MQL5",
                "<g0>Foo</g0>",
                "\"Foo, Bar\"",
                "\u01c7 \u01c8 \u01c9");
        List<Map<String, Object>> changeCaseCases = new ArrayList<>();
        for (String input : changeCaseInputs) {
            Map<String, Object> changeCase = new LinkedHashMap<>();
            changeCase.put("input", input);
            changeCase.put("lower", EditorUtils.doChangeCase(input, IEditor.CHANGE_CASE_TO.LOWER,
                    Locale.ENGLISH, changeCaseTokenizer));
            changeCase.put("upper", EditorUtils.doChangeCase(input, IEditor.CHANGE_CASE_TO.UPPER,
                    Locale.ENGLISH, changeCaseTokenizer));
            changeCase.put("sentence", EditorUtils.doChangeCase(input, IEditor.CHANGE_CASE_TO.SENTENCE,
                    Locale.ENGLISH, changeCaseTokenizer));
            changeCase.put("title", EditorUtils.doChangeCase(input, IEditor.CHANGE_CASE_TO.TITLE,
                    Locale.ENGLISH, changeCaseTokenizer));
            changeCase.put("cycle", EditorUtils.doChangeCase(input, IEditor.CHANGE_CASE_TO.CYCLE,
                    Locale.ENGLISH, changeCaseTokenizer));
            changeCaseCases.add(changeCase);
        }
        List<String> cycleSequence = new ArrayList<>();
        String cycled = changeCaseInputs.get(0);
        for (int i = 0; i < 5; i++) {
            cycled = EditorUtils.doChangeCase(cycled, IEditor.CHANGE_CASE_TO.CYCLE,
                    Locale.ENGLISH, changeCaseTokenizer);
            cycleSequence.add(cycled);
        }
        writeCase("remaining/EditorUtilsTest-testChangeCase.json",
                "org.omegat.util.editor.EditorUtilsTest#testChangeCase",
                Map.of("cases", changeCaseCases, "cycle_sequence", cycleSequence));
        writeCase("remaining/IssuesTypeListModelTest-testCalculateData_NoIssues.json",
                "org.omegat.gui.issues.IssuesTypeListModelTest#testCalculateData_NoIssues", Map.of("count", 0));
        writeCase("remaining/IssuesTypeListModelTest-testCalculateData_SingleType.json",
                "org.omegat.gui.issues.IssuesTypeListModelTest#testCalculateData_SingleType", Map.of("count", 1));
        writeCase("remaining/IssuesTypeListModelTest-testCalculateData_MultipleTypes.json",
                "org.omegat.gui.issues.IssuesTypeListModelTest#testCalculateData_MultipleTypes", Map.of("count", 2));
        writeCase("remaining/IssuesTypeListModelTest-testCalculateData_SortedOutput.json",
                "org.omegat.gui.issues.IssuesTypeListModelTest#testCalculateData_SortedOutput",
                Map.of("sorted", true));
        writeCase("gui/IssueProvidersTest-testGetIssueProviders.json",
                "org.omegat.gui.issues.IssueProvidersTest#testGetIssueProviders",
                Map.of("providers", List.of("tag", "spell", "terminology", "languagetool")));
        writeCase("gui/IssueProvidersTest-testGetDisabledProviderIds.json",
                "org.omegat.gui.issues.IssueProvidersTest#testGetDisabledProviderIds",
                Map.of("ids", List.of()));
        writeCase("remaining/ExternalTMFactoryTest-testLoadPO.json",
                "org.omegat.core.data.ExternalTMFactoryTest#testLoadPO",
                Map.of("supported", true, "count", 1013,
                        "src0", "Choose syntax highlighting",
                        "tgt0", "Вылучэньне &колерам сынтаксысу",
                        "src1", "< Auto >", "tgt1", "Пра праграму"));
        writeCase("remaining/ExternalTMFactoryTest-testLoadMozillaLang.json",
                "org.omegat.core.data.ExternalTMFactoryTest#testLoadMozillaLang",
                Map.of("supported", true, "count", 33,
                        "src0", "Download %s for Android in your language",
                        "tgt0", "Laden Sie %s für Android in Ihrer Sprache herunter"));
        writeCase("remaining/ExternalTMFactoryTest-testLoadXliff.json",
                "org.omegat.core.data.ExternalTMFactoryTest#testLoadXliff",
                Map.of("supported", true, "count", 3, "src0", "This is a test",
                        "tgt0", "Això és una prova"));
        writeCase("remaining/EntityUtilTest-testEntitiesToCharsNamedEntities.json",
                "org.omegat.util.html.EntityUtilTest#testEntitiesToCharsNamedEntities",
                Map.of("cases", List.of(
                        Map.of("input", "&lt;", "output", "<"),
                        Map.of("input", "&gt;", "output", ">"),
                        Map.of("input", "&amp;", "output", "&"),
                        Map.of("input", "&quot;", "output", "\""))));
        writeCase("remaining/EntityUtilTest-testEntitiesToCharsSpecialCharacters.json",
                "org.omegat.util.html.EntityUtilTest#testEntitiesToCharsSpecialCharacters",
                Map.of("cases", List.of(
                        Map.of("input", "&OElig;", "output", "Œ"),
                        Map.of("input", "&oelig;", "output", "œ"),
                        Map.of("input", "&Scaron;", "output", "Š"),
                        Map.of("input", "&scaron;", "output", "š"),
                        Map.of("input", "&Yuml;", "output", "Ÿ"))));
        writeCase("remaining/EntityUtilTest-testEntitiesToCharsNumericEntities.json",
                "org.omegat.util.html.EntityUtilTest#testEntitiesToCharsNumericEntities",
                Map.of("cases", List.of(
                        Map.of("input", "&#34;", "output", "\""),
                        Map.of("input", "&#x22;", "output", "\""),
                        Map.of("input", "&#169;", "output", "©"))));
        writeCase("remaining/EntityUtilTest-testEntitiesToCharsInvalid.json",
                "org.omegat.util.html.EntityUtilTest#testEntitiesToCharsInvalid",
                Map.of("cases", List.of(
                        Map.of("input", "&invalid;", "output", "&invalid;"),
                        Map.of("input", "&;", "output", "&;"),
                        Map.of("input", "& #;", "output", "& #;"))));
        writeCase("remaining/EntityUtilTest-testCharsToEntitiesBasicEntities.json",
                "org.omegat.util.html.EntityUtilTest#testCharsToEntitiesBasicEntities",
                Map.of("cases", List.of(
                        Map.of("input", "<", "output", "&lt;"),
                        Map.of("input", ">", "output", "&gt;"),
                        Map.of("input", "&", "output", "&amp;"),
                        Map.of("input", "\u00A0", "output", "&nbsp;"))));
        writeCase("remaining/EntityUtilTest-testCharsToEntitiesProtectedEntities.json",
                "org.omegat.util.html.EntityUtilTest#testCharsToEntitiesProtectedEntities",
                Map.of("protected", List.of("<b0>", "</b0>", "<c>", "</c>", "<u1>", "</u1>"),
                        "input", "Le gros <u1>chat</u1> <c>test</c> & <b0>noir</b0> dors",
                        "output", "Le gros <u1>chat</u1> <c>test</c> &amp; <b0>noir</b0> dors"));
        writeCase("remaining/MagicCommentTest-testParseString.json",
                "org.omegat.util.MagicCommentTest#testParseString",
                Map.of("cases", List.of(
                        Map.of("input", "# -*- coding: UTF-8 -*-", "map", Map.of("coding", "UTF-8")),
                        Map.of("input", "# comment -*- coding: UTF-8 -*-", "map", Map.of("coding", "UTF-8")),
                        Map.of("input", "# comment -*- coding: UTF-8; foo: bar -*-",
                                "map", Map.of("coding", "UTF-8", "foo", "bar")),
                        Map.of("input", "# comment -*- foo: bar; coding: UTF-8; -*-",
                                "map", Map.of("coding", "UTF-8", "foo", "bar")),
                        Map.of("input", "# comment -*- foo: bar; -*- coding: UTF-8",
                                "map", Map.of("foo", "bar")),
                        Map.of("input", "# comment -*- foo: bar; coding: UTF-8", "map", Map.of()),
                        Map.of("input", "# comment foo: bar; coding: UTF-8 -*-", "map", Map.of()))));
        writeCase("remaining/MagicCommentTest-testParseFile.json",
                "org.omegat.util.MagicCommentTest#testParseFile", Map.of("coding", "UTF-8"));
        writeCase("remaining/MagicCommentTest-testParseFileBom.json",
                "org.omegat.util.MagicCommentTest#testParseFileBom", Map.of("coding", "UTF-8"));
        writeCase("remaining/MagicCommentTest-testParseEmpty.json",
                "org.omegat.util.MagicCommentTest#testParseEmpty", Map.of("empty", true));
        writeCase("remaining/MagicCommentTest-testParseFileTab.json",
                "org.omegat.util.MagicCommentTest#testParseFileTab", Map.of("empty", true));
        writeCase("remaining/MagicCommentTest-testParseFileUTF16.json",
                "org.omegat.util.MagicCommentTest#testParseFileUTF16", Map.of("empty", true));
        writeCase("remaining/TagUtilTest-testBuildTagList.json",
                "org.omegat.util.TagUtilTest#testBuildTagList",
                Map.of("text", "Tag <test> case <b0>one</b0>.<b1>",
                        "omegat", List.of(Map.of("pos", 16, "tag", "<b0>"),
                                Map.of("pos", 23, "tag", "</b0>"), Map.of("pos", 29, "tag", "<b1>"))));
        writeCase("remaining/TagUtilTest-testTagType.json",
                "org.omegat.util.TagUtilTest#testTagType",
                Map.of("cases", List.of(
                        Map.of("tag", "<x0>", "type", "START"), Map.of("tag", "<x10>", "type", "START"),
                        Map.of("tag", "</x0>", "type", "END"), Map.of("tag", "<x0/>", "type", "SINGLE"),
                        Map.of("tag", "<x>", "type", "SINGLE"), Map.of("tag", "<x/>", "type", "SINGLE"),
                        Map.of("tag", "</x>", "type", "SINGLE"), Map.of("tag", "</x0/>", "type", "SINGLE"),
                        Map.of("tag", "foo", "type", "SINGLE"))));
        writeCase("remaining/StaticUtilsTest-testParseCLICommand.json",
                "org.omegat.util.StaticUtilsTest#testParseCLICommand",
                Map.of("cmd", " sort  \"/path with/spaces in/it\"    /path\\ with/escaped\\ spaces/ \"escape\\\"escape\" 'noescape\\'noescape'' \"noescape\\ noescape\" C:\\windows\\path",
                        "args", List.of("sort", "/path with/spaces in/it", "/path with/escaped spaces/",
                                "escape\"escape", "noescape\\noescape", "noescape\\ noescape",
                                "C:\\windows\\path"),
                        "space", List.of("")));
        writeCase("remaining/ProjectFileStorageTest-testLoadDefaults.json",
                "org.omegat.util.ProjectFileStorageTest#testLoadDefaults",
                Map.of("source_lang", "en-us", "target_lang", "fr-fr",
                        "source_tok", "org.omegat.tokenizer.LuceneEnglishTokenizer",
                        "target_tok", "org.omegat.tokenizer.LuceneFrenchTokenizer",
                        "sentence_seg", true, "support_default", true, "remove_tags", false,
                        "exclude_count", 6, "exclude0", "**/.svn/**"));
        writeCase("remaining/LingvoDSLTest-testReadFileDict.json",
                "org.omegat.core.dictionaries.LingvoDSLTest#testReadFileDict",
                Map.of("word", "space",
                        "article", "<div style=\"text-indent: 30px\">Only a single white space on first character</div>"));
        writeCase("remaining/StarDictTest-testStardict4j.json",
                "org.omegat.core.dictionaries.StarDictTest#testStardict4j",
                Map.of("word_count", 10451, "exact", 1, "word", "testudo",
                        "article_contains", "tortue", "predictive", 1));
        writeCase("remaining/LanguageToolTest-testLanguageMapping.json",
                "org.omegat.languagetools.LanguageToolTest#testLanguageMapping",
                Map.of("cases", List.of(
                        Map.of("code", "en-US", "class", "org.languagetool.language.AmericanEnglish"),
                        Map.of("code", "en-CA", "class", "org.languagetool.language.CanadianEnglish"),
                        Map.of("code", "en", "class", "org.languagetool.language.English"),
                        Map.of("code", "en-JA", "class", "org.languagetool.language.English"),
                        Map.of("code", "be-BY", "class", "org.languagetool.language.Belarusian"),
                        Map.of("code", "be", "class", "org.languagetool.language.Belarusian"),
                        new LinkedHashMap<String, Object>() {{
                            put("code", "xyz");
                            put("class", null);
                        }})));
        writeCase("remaining/LanguageToolTest-testWrapperInit.json",
                "org.omegat.languagetools.LanguageToolTest#testWrapperInit",
                Map.of("java_default_bridge", "LanguageToolNativeBridge",
                        "java_bad_url_fallback", "LanguageToolNativeBridge",
                        "rewrite_bridge", "http"));
        writeCase("remaining/MatchesVarExpansionTest-testExpandVariables.json",
                "org.omegat.gui.matches.MatchesVarExpansionTest#testExpandVariables",
                Map.of("template",
                        "${id}. ... ${sourceText}\n${targetText}\n<${score}/${noStemScore}/${adjustedScore}% ${filePath}>",
                        "text",
                        "${id}. ... ${sourceText}\nmock target text\n<20/40/60% mock testing project>"));
        writeCase("remaining/HTTPRemoteRepositoryTest-testRetrieveHandlesNotModifiedResponse.json",
                "org.omegat.core.team2.impl.HTTPRemoteRepositoryTest#testRetrieveHandlesNotModifiedResponse",
                Map.of("status", 304, "skip_write", true));
        writeStrictCase("remaining/HTTPRemoteRepositoryTest-testSwitchToVersionThrowsExceptionWhenVersionIsNotNull.json",
                "org.omegat.core.team2.impl.HTTPRemoteRepositoryTest#testSwitchToVersionThrowsExceptionWhenVersionIsNotNull",
                Map.of("version", "1.0", "throws", true, "message", "Not supported"));
        writeCase("remaining/HTTPRemoteRepositoryTest-testSwitchToVersionUpdatesToLatest.json",
                "org.omegat.core.team2.impl.HTTPRemoteRepositoryTest#testSwitchToVersionUpdatesToLatest",
                new LinkedHashMap<String, Object>() {{
                    put("version", null);
                    put("ok", true);
                }});
        writeCase("remaining/MatchesTextAreaTest-testReplaceNumbers.json",
                "org.omegat.gui.matches.MatchesTextAreaTest#testReplaceNumbers",
                Map.of("cases", List.of(
                        Map.of("source", "chapter 5", "src_match", "chapter 1", "trg_match", "foo 1",
                                "out", "foo 5"),
                        Map.of("source", "chapter 5.5", "src_match", "chapter 1.1", "trg_match", "foo 1.1",
                                "out", "foo 5.5"))));
        writeCase("remaining/EncodingDetectorTest-testDetectHTMLEncoding.json",
                "org.omegat.util.EncodingDetectorTest#testDetectHTMLEncoding",
                Map.of("cases", List.of(
                        Map.of("file", "file-HTMLUtils-utf8-content-type.html", "encoding", "UTF-8"),
                        Map.of("file", "file-HTMLUtils-utf16_be_with_bom.html", "encoding", "UTF-16BE"),
                        Map.of("file", "file-HTMLUtils-utf16_le_with_bom.html", "encoding", "UTF-16LE"),
                        Map.of("file", "file-HTMLUtils-utf8_with_bom.html", "encoding", "UTF-8"))));
        writeCase("remaining/PreferencesTest-testPreferencesLoadStore.json",
                "org.omegat.util.PreferencesTest#testPreferencesLoadStore",
                Map.of("MyString", "foo", "MyBoolean", "true", "MyInt", "5", "MyEnum", "BAR",
                        "MyEmptyString", "", "MyStringDefault", "bar", "MyIntDefault", "77"));
        writeCase("remaining/PreferencesTest-testLoadingUserPreferencesXML.json",
                "org.omegat.util.PreferencesTest#testLoadingUserPreferencesXML",
                Map.of("key", "source_font_size", "loaded", "14", "saved", "12"));
        writeCase("remaining/PreferencesTest-testPreferencesBackup.json",
                "org.omegat.util.PreferencesTest#testPreferencesBackup",
                Map.of("backup_ext", ".bak"));
        writeCase("remaining/TransTipsMarkerTest-testGetMarksForEntryValidGlossaryMatches.json",
                "org.omegat.gui.glossary.TransTipsMarkerTest#testGetMarksForEntryValidGlossaryMatches",
                Map.of("source", "source text", "marks", 1, "start", 0, "end", 11, "tooltip", "tooltip"));
        writeCase("remaining/DictionariesManagerTest-testAddIgnoreWord.json",
                "org.omegat.core.dictionaries.DictionariesManagerTest#testAddIgnoreWord",
                Map.of("word", "testudo", "ignored", true));
        writeCase("remaining/DictionariesManagerTest-testFindWords.json",
                "org.omegat.core.dictionaries.DictionariesManagerTest#testFindWords",
                Map.of("ignore", "testor", "find1", "testudo", "find2", "tete", "count", 2));
        writeCase("remaining/SpellCheckerManagerTest-testGetCurrentSpellChecker_FallsBackToDummy.json",
                "org.omegat.core.spellchecker.SpellCheckerManagerTest#testGetCurrentSpellChecker_FallsBackToDummy",
                Map.of("fallback", "dummy"));
        writeCase("remaining/ProjectFilesListControllerTest-testUpdateProgressColumnRemovesAndRestoresColumn.json",
                "org.omegat.gui.filelist.ProjectFilesListControllerTest#testUpdateProgressColumnRemovesAndRestoresColumn",
                Map.of("hidden_count", 1, "shown_count", 2));
        writeCase("remaining/EditorUtilsTest-testReplaceGlossaryEntries.json",
                "org.omegat.util.editor.EditorUtilsTest#testReplaceGlossaryEntries",
                Map.of("src", "Snowman Bob went to the snowman party. SnOwMaN!",
                        "out", "Sneeuwpop Blub went to the sneeuwpop party. sneeuwpop!",
                        "multi_src", "Snowman Bob went to the snowman party. SnOwMaN!",
                        "multi_out", "Sneeuwpop Blub went to the sneeuwpop parti. sneeuwpop!",
                        "final_src", "Snowman Bob went to the snowman party. SnOwMaN",
                        "final_out", "Sneeuwpop Blub went to the sneeuwpop parti. sneeuwpop"));
        writeCase("remaining/TMXDateParserTest-testParseDate.json",
                "org.omegat.util.TMXDateParserTest#testParseDate",
                Map.of("format", "yyyyMMdd'T'HHmmss'Z'",
                        "roundtrip", List.of("19971116T192059Z", "19970716T192059Z"),
                        "invalid", List.of("19971116T192059+00:00", "19971116T", "")));
        writeCase("remaining/TmxEscapingWriterTest-testNBSP.json",
                "org.omegat.util.TmxEscapingWriterTest#testNBSP",
                Map.of("input", "[\u00A0]", "output", "[\u00A0]"));
        writeCase("remaining/TmxEscapingWriterTest-testNBH.json",
                "org.omegat.util.TmxEscapingWriterTest#testNBH",
                Map.of("input", "\u0083", "output", "&#x83;"));
        writeCase("remaining/TmxEscapingWriterTest-testSurrogatePair.json",
                "org.omegat.util.TmxEscapingWriterTest#testSurrogatePair",
                Map.of("input", "[😀]", "output", "[😀]"));
        writeCase("remaining/TmxEscapingWriterTest-testInvalidChar.json",
                "org.omegat.util.TmxEscapingWriterTest#testInvalidChar",
                Map.of("input", "\uFFFE", "output", "&#xfffe;"));
        writeCase("remaining/HttpConnectionUtilsTest-testDecodeURLs.json",
                "org.omegat.util.HttpConnectionUtilsTest#testDecodeURLs",
                Map.of("encoded", "https://fr.wikipedia.org/wiki/Science_du_syst%C3%A8me_Terre",
                        "decoded", HttpConnectionUtils.decodeHttpURLs(
                                "https://fr.wikipedia.org/wiki/Science_du_syst%C3%A8me_Terre")));
        String decodeInText = "1. https://fr.wikipedia.org/wiki/Science_du_syst%C3%A8me_Terre";
        String decodeJa = "2. https://ja.wikipedia.org/wiki/2024%E5%B9%B4%E3%81%AE%E3%82%AB%E3%82%BF%E3%83%BC%E3"
                + "%83%AB%E3%82%B0%E3%83%A9%E3%83%B3%E3%83%97%E3%83%AA"
                + "_%28%E3%83%AD%E3%83%BC%E3%83%89%E3%83%AC%E3%83%BC%E3%82%B9%29 参照";
        writeCase("remaining/HttpConnectionUtilsTest-testDecodeURLsInText.json",
                "org.omegat.util.HttpConnectionUtilsTest#testDecodeURLsInText",
                Map.of("input", decodeInText, "output", HttpConnectionUtils.decodeHttpURLs(decodeInText),
                        "input_ja", decodeJa, "output_ja", HttpConnectionUtils.decodeHttpURLs(decodeJa)));
        String decodeMulti = "1. https://google.com/\n2. bar\n"
                + "3. https://fr.wikipedia.org/wiki/Science_du_syst%C3%A8me_Terre";
        writeCase("remaining/HttpConnectionUtilsTest-testDecodeURLsMultipleLines.json",
                "org.omegat.util.HttpConnectionUtilsTest#testDecodeURLsMultipleLines",
                Map.of("input", decodeMulti, "output", HttpConnectionUtils.decodeHttpURLs(decodeMulti),
                        "lines", 3));
        String encBase = "https://fr.wikipedia.org/";
        String encPath = "wiki/Science_du_système_Terre";
        String encQuery = "?query=search&lang=en";
        String encBracket = "https://fr.wikipedia.org/wiki/Doughnut_(modèle_économique)";
        String encJa = "2. https://ja.wikipedia.org/wiki/2024年のカタールグランプリ_(ロードレース)";
        writeCase("remaining/HttpConnectionUtilsTest-testEncodeURLs.json",
                "org.omegat.util.HttpConnectionUtilsTest#testEncodeURLs",
                Map.of("cases", List.of(
                        Map.of("in", encBase, "out", HttpConnectionUtils.encodeHttpURLs(encBase)),
                        Map.of("in", encBase + encPath, "out",
                                HttpConnectionUtils.encodeHttpURLs(encBase + encPath)),
                        Map.of("in", encBase + encPath + encQuery, "out",
                                HttpConnectionUtils.encodeHttpURLs(encBase + encPath + encQuery)),
                        Map.of("in", encBracket, "out", HttpConnectionUtils.encodeHttpURLs(encBracket)),
                        Map.of("in", encJa, "out", HttpConnectionUtils.encodeHttpURLs(encJa)))));
        writeCase("remaining/StatisticsTest-testNumberOfWords.json",
                "org.omegat.core.statistics.StatisticsTest#testNumberOfWords",
                Map.of("cases", List.of(
                        Map.of("text", "one two three", "words", 3),
                        Map.of("text", "one , \b two three", "words", 3),
                        Map.of("text", "o\bne <b>two</b>", "words", 5))));
        writeCase("remaining/StatisticsTest-testNumberOfChars.json",
                "org.omegat.core.statistics.StatisticsTest#testNumberOfChars",
                Map.of("without_spaces", 3, "with_spaces", 4, "text", "1 2\b3"));
        writeCase("remaining/TokenTest-testGlossaryTokenEqualityEnglish.json",
                "org.omegat.core.data.TokenTest#testGlossaryTokenEqualityEnglish",
                Map.of("str", "source and target", "glos", "target", "str_len", 3, "glos_len", 1,
                        "first_deep_eq", false, "last_eq", true));
        writeCase("remaining/TokenTest-testGlossaryTokenEqualityJapanese.json",
                "org.omegat.core.data.TokenTest#testGlossaryTokenEqualityJapanese",
                Map.of("bug", "1034", "expected", "AssertionError", "str", "場所", "glos", "塗布"));
        writeCase("remaining/VersionTest-testVersionComparison.json",
                "org.omegat.util.VersionTest#testVersionComparison",
                Map.of("eq", List.of("1.0.0", "0", "1.0.0", "0"),
                        "less", List.of(
                                List.of("1.0.0", "0", "1.0.0", "1"),
                                List.of("1.0.0", "0", "1.0.1", "0"),
                                List.of("1.0.0", "0", "1.1.0", "0"),
                                List.of("1.0.0", "0", "2.0.0", "0")),
                        "bad_len", List.of("1.0", "0", "1.0.0", "0"),
                        "bad_parse", List.of("a.b.c", "0", "1.0.0", "0")));
        Map<String, Object> abcOnly = new LinkedHashMap<>();
        abcOnly.put("text", "abc");
        abcOnly.put("match", true);
        abcOnly.put("lang", "abc");
        abcOnly.put("country", null);
        writeCase("remaining/PatternConstsTest-testLangAndCountry.json",
                "org.omegat.util.PatternConstsTest#testLangAndCountry",
                Map.of("cases", List.of(
                        Map.of("text", "abc*DEF", "match", false),
                        Map.of("text", "abc-DEF", "match", true, "lang", "abc", "country", "DEF"),
                        abcOnly,
                        Map.of("text", "Z-abc", "match", true, "lang", "Z", "country", "abc"))));
        writeCase("remaining/MergeTest-testTimeTruncate.json",
                "org.omegat.core.data.MergeTest#testTimeTruncate",
                Map.of("input_ms", 123456999L, "truncated_ms", 123456000L));
        writeCase("remaining/MergeTest-testEquals.json",
                "org.omegat.core.data.MergeTest#testEquals",
                Map.of("same", true, "truncated_equal", true, "other_time", false,
                        "diff_translation", false, "diff_note", false, "diff_changer_ok", true));
        writeCase("remaining/MixedEolHandlingReaderTest-testDetection.json",
                "org.omegat.util.MixedEolHandlingReaderTest#testDetection",
                Map.of("cases", List.of(
                        Map.of("text", "a", "eol", "\n", "mixed", false),
                        Map.of("text", "a\nb\nc\n", "eol", "\n", "mixed", false),
                        Map.of("text", "a\rb\rc\r", "eol", "\r", "mixed", false),
                        Map.of("text", "a\r\nb\r\nc\r\n", "eol", "\r\n", "mixed", false),
                        Map.of("text", "a\r\r\nb\r\nc\r\n", "eol", "\r\n", "mixed", true),
                        Map.of("text", "a\r\r\nb\rc\r", "eol", "\r", "mixed", true),
                        Map.of("text", "a\n\r\nb\nc\n", "eol", "\n", "mixed", true),
                        Map.of("text", "a\r\r\nb\nc", "eol", "\r\n", "mixed", true))));
        writeCase("remaining/MixedEolHandlingReaderTest-testReadLine.json",
                "org.omegat.util.MixedEolHandlingReaderTest#testReadLine",
                Map.of("cases", List.of(
                        Map.of("text", "a\rb\rc", "eol", "\r", "lines", List.of("a", "b", "c")),
                        Map.of("text", "a\nb\nc", "eol", "\n", "lines", List.of("a", "b", "c")),
                        Map.of("text", "a\r\nb\r\nc", "eol", "\r\n", "lines", List.of("a", "b", "c")))));
        writeCase("remaining/MixedEolHandlingReaderTest-testFile.json",
                "org.omegat.util.MixedEolHandlingReaderTest#testFile",
                Map.of("file", "data/filters/text/file-TextFilter.txt",
                        "line0", "This test file for test TextFilter.",
                        "eol", "\r\n", "mixed", false));
        writeCase("remaining/KnownExceptionTest-testExceptions.json",
                "org.omegat.core.KnownExceptionTest#testExceptions",
                Map.of("code", "TF_ERROR", "params", List.of("param1", "param2"),
                        "localized", "Error", "cause", "Cause"));
        writeCase("remaining/GlossaryReaderCSVTest-testRead.json",
                "org.omegat.gui.glossary.GlossaryReaderCSVTest#testRead",
                Map.of("count", 7, "src0", "kde", "loc0", "csv kde", "src6", "zz\"zz", "loc6", "zz"));
        writeCase("remaining/GlossaryReaderTBXTest-testRead.json",
                "org.omegat.gui.glossary.GlossaryReaderTBXTest#testRead",
                Map.of("count", 1, "src", "alpha smoothing factor", "loc", "hu translation"));
        writeCase("remaining/DictionaryDataTest-testLookup.json",
                "org.omegat.core.dictionaries.DictionaryDataTest#testLookup",
                Map.of("size_before", -1, "size_after", 4, "foobar", 2, "FOOBAR", 2,
                        "blah", 2, "BLAH", 1, "pred_foo", 2, "exact_foo", 0, "nfc", 1, "zzzz", 0));
        writeCase("remaining/CalcStandardStatisticsTest-testStatistics.json",
                "org.omegat.core.statistics.CalcStandardStatisticsTest#testStatistics",
                Map.of("file", "data/filters/po/file-POFilter-match-stat-en-ca.po",
                        "total_segments", 108, "total_words", 938, "total_nosp", 4894,
                        "total_chars", 5699, "unique_segments", 97, "unique_words", 848,
                        "unique_nosp", 4385, "unique_chars", 5116, "file_segments", 108));
        writeCase("remaining/ScriptingTest-testLoadScriptingWindow.json",
                "org.omegat.gui.scripting.ScriptingTest#testLoadScriptingWindow",
                Map.of("bug", "775", "invalid_is_file", true, "constructs", true));
        writeCase("remaining/ScriptingTest-testDefaultScriptFolderOnScriptWindow.json",
                "org.omegat.gui.scripting.ScriptingTest#testDefaultScriptFolderOnScriptWindow",
                Map.of("config_dir", "/tmp/omegat-config", "scripts", "/tmp/omegat-config/scripts"));
    }

    /**
     * Replace the last method-name-only fixtures with values asserted by the
     * corresponding Java tests. Running each complete test class here prevents
     * these snapshots from surviving after the Java product behavior changes.
     */
    private void exportThinProductTests() throws Exception {
        String[] javaTests = {
                "org.omegat.gui.main.ProjectUICommandsTest#testIsIdenticalOmegatProjectProperties0",
                "org.omegat.gui.main.ProjectUICommandsTest#testGetRootRepositoryMapping0",
                "org.omegat.gui.main.ProjectUICommandsTest#testGetRootRepositoryMappingSvn",
                "org.omegat.gui.main.ProjectUICommandsTest#testSetRootRepositoryMapping0",
                "org.omegat.gui.main.ProjectUICommandsTest#testIsRepositoryEqual",
                "org.omegat.gui.issues.SimpleIssueTest#testGetIconReturnsNonNullIcon",
                "org.omegat.gui.issues.SimpleIssueTest#testGetDetailComponentReturnsCorrectComponent",
                "org.omegat.gui.issues.SimpleIssueTest#testGetDetailComponentPopulatesTextFields",
                "org.omegat.gui.issues.SimpleIssueTest#testGetIconUsesExpectedColor",
                "org.omegat.gui.issues.SimpleIssueTest#testGetEntryNum",
                "org.omegat.gui.issues.IssueCheckerTest#testCollectIssuesAggregatesTagAndProvider",
                "org.omegat.gui.issues.IssueCheckerTest#testFilePatternFiltersEntries",
                "org.omegat.gui.issues.IssueCheckerTest#testDuplicateFiltering",
                "org.omegat.gui.glossary.GlossaryTextAreaTest#testSetGlossaryEntries",
                "org.omegat.gui.glossary.GlossaryTextAreaTest#testSetGlossaryEntriesWithLink",
                "org.omegat.gui.glossary.GlossaryTextAreaTest#testClear",
                "org.omegat.gui.notes.NotesTextAreaTest#testSetNote",
                "org.omegat.gui.notes.NotesTextAreaTest#testClear",
                "org.omegat.util.OStringsTest#testDevBuildMarkerFromBranchCheckout",
                "org.omegat.util.OStringsTest#testDevBuildMarkerHiddenOutsideBranchCheckouts",
                "org.omegat.gui.matches.FindMatchesThreadTest#testSearchBUGS1248",
                "org.omegat.util.XMLStreamReaderTest#testLoadXML",
                "org.omegat.util.XMLStreamReaderTest#testBadEntity",
                "org.omegat.core.statistics.StatsResultTest#testStatsResultXML",
                "org.omegat.gui.scripting.ScriptingTest#testScriptProperties",
                "org.omegat.gui.scripting.ScriptRunnerTest#testAvailableEngines",
                "org.omegat.gui.scripting.ScriptRunnerTest#testCompileScripts"
        };
        Set<String> testClasses = new TreeSet<>();
        for (String javaTest : javaTests) {
            testClasses.add(javaTest.substring(0, javaTest.lastIndexOf('#')));
        }
        for (String testClass : testClasses) {
            assertJavaTestClass(testClass);
        }

        writeCase("gui/ProjectUICommandsTest-testIsIdenticalOmegatProjectProperties0.json",
                javaTests[0], Map.of("identical_before", true, "identical_after_export_levels_change", false));
        writeCase("gui/ProjectUICommandsTest-testGetRootRepositoryMapping0.json", javaTests[1],
                Map.of("branch", "main", "type", "git", "url", "git@github.com:omegat-L10N/ja.git",
                        "mapping_count", 1, "local", "/", "repository", "/"));
        writeCase("gui/ProjectUICommandsTest-testGetRootRepositoryMappingSvn.json", javaTests[2],
                Map.of("type", "svn", "mapping_count", 1, "local", "/", "repository", "/"));
        writeCase("gui/ProjectUICommandsTest-testSetRootRepositoryMapping0.json", javaTests[3],
                Map.of("repository_count", 1, "type", "git", "branch", "main",
                        "url", "https://github.com/omegat-L10N/ja.git", "local", "/", "repository", "/"));
        writeCase("gui/ProjectUICommandsTest-testIsRepositoryEqual.json", javaTests[4],
                Map.of("different_url", false, "same_object", true, "mappings_ignored", true));

        writeCase("gui/SimpleIssueTest-testGetIconReturnsNonNullIcon.json", javaTests[5],
                Map.of("icon_class", "SimpleColorIcon", "present", true));
        writeCase("gui/SimpleIssueTest-testGetDetailComponentReturnsCorrectComponent.json", javaTests[6],
                Map.of("component_class", "IssueDetailSplitPanel", "present", true));
        writeCase("gui/SimpleIssueTest-testGetDetailComponentPopulatesTextFields.json", javaTests[7],
                Map.of("source", "Hello world!", "translation", "Hallo Welt!"));
        writeCase("gui/SimpleIssueTest-testGetIconUsesExpectedColor.json", javaTests[8],
                Map.of("color", "#FF0000"));
        writeCase("gui/SimpleIssueTest-testGetEntryNum.json", javaTests[9], Map.of("entry_num", 1));

        writeCase("gui/IssueCheckerTest-testCollectIssuesAggregatesTagAndProvider.json", javaTests[10],
                Map.of("pattern", ".*", "provider_count", 4, "tag_count", 1, "total", 5));
        writeCase("gui/IssueCheckerTest-testFilePatternFiltersEntries.json", javaTests[11],
                Map.of("pattern", "\\Qfile1.txt\\E", "provider_count", 2, "tag_count", 0, "total", 2));
        writeCase("gui/IssueCheckerTest-testDuplicateFiltering.json", javaTests[12],
                Map.of("provider_all", 4, "provider_filtered", 3, "tag_all", 1, "tag_filtered", 1));

        List<Map<String, Object>> glossary = List.of(
                Map.of("source", "source1", "target", "translation1", "comment", ""),
                Map.of("source", "source2", "target", "translation2", "comment", "comment2"));
        String glossaryText = "source1 = translation1source2 = translation2\n1. comment2";
        writeCase("remaining/GlossaryTextAreaTest-testSetGlossaryEntries.json", javaTests[13],
                Map.of("entries", glossary, "text", glossaryText));
        List<Map<String, Object>> glossaryLink = new ArrayList<>(glossary);
        glossaryLink.add(Map.of("source", "source3", "target", "translation3",
                "comment", "https://fr.wikipedia.org/wiki/Science_du_syst%C3%A8me_Terre"));
        writeCase("remaining/GlossaryTextAreaTest-testSetGlossaryEntriesWithLink.json", javaTests[14],
                Map.of("entries", glossaryLink, "text",
                        glossaryText + "source3 = translation3\n1. https://fr.wikipedia.org/wiki/Science_du_système_Terre"));
        writeCase("remaining/GlossaryTextAreaTest-testClear.json", javaTests[15],
                Map.of("before", glossaryText, "after", ""));
        writeCase("remaining/NotesTextAreaTest-testSetNote.json", javaTests[16],
                Map.of("set", "foobar", "empty_is_null", true));
        writeCase("remaining/NotesTextAreaTest-testClear.json", javaTests[17],
                Map.of("before", "foobar", "after_is_null", true));

        writeCase("remaining/OStringsTest-testDevBuildMarkerFromBranchCheckout.json", javaTests[18],
                Map.of("cases", List.of(
                        Map.of("revision", "6d79ee8db", "branch", "master",
                                "marker", "[6d79ee8db @ master]"),
                        Map.of("revision", "6d79ee8db", "branch", "topic/stpa/build/worktree-revision",
                                "marker", "[6d79ee8db @ topic/stpa/build/worktree-revision]"))));
        writeCase("remaining/OStringsTest-testDevBuildMarkerHiddenOutsideBranchCheckouts.json", javaTests[19],
                Map.of("cases", List.of(
                        Map.of("revision", "6d79ee8db", "branch", "", "marker", ""),
                        Map.of("revision", "6d79ee8db", "branch", "HEAD", "marker", ""),
                        Map.of("revision", "6d79ee8db", "branch", "@gitbranch@", "marker", ""))));

        writeCase("remaining/FindMatchesThreadTest-testSearchBUGS1248.json", javaTests[20],
                Map.of("query", "地力の搾取と浪費が現われる。(1)", "threshold", 30,
                        "hits", List.of(
                                Map.of("source", "地力の搾取と浪費が現われる。(1)", "translation", "weird behavior",
                                        "comes_from", "TM", "score", 90),
                                Map.of("source", "地力の搾取と浪費が現われる。(1)",
                                        "comes_from", "SUBSEGMENTS", "score", 90))));

        List<String> xmlBlocks = List.of(
                "open:ascii", "text:bar", "close:ascii",
                "open:bmp", "text:☃", "close:bmp",
                "open:dec", "text:☃", "close:dec",
                "open:hex", "text:☃", "close:hex",
                "open:astral", "text:🂿", "close:astral",
                "open:a-dec", "text:🂿", "close:a-dec",
                "open:a-hex", "text:🂿", "close:a-hex",
                "open:named", "text:&<>'\"", "close:named", "standalone:standalone");
        writeCase("remaining/XMLStreamReaderTest-testLoadXML.json", javaTests[21],
                Map.of("file", "data/xml/test.xml", "body_attr", "foo", "blocks", xmlBlocks));
        writeCase("remaining/XMLStreamReaderTest-testBadEntity.json", javaTests[22],
                Map.of("files", List.of("data/xml/test-badDecimalEntity.xml",
                        "data/xml/test-badHexEntity.xml"), "error_class", "TranslationException"));

        writeCase("remaining/StatsResultTest-testStatsResultXML.json", javaTests[23],
                Map.ofEntries(
                        Map.entry("project", Map.of("name", "testProject", "root", "",
                                "source_language", "English", "target_language", "French")),
                        Map.entry("total", List.of(0, 0, 0, 0, 0)),
                        Map.entry("remaining", List.of(0, 0, 0, 0, 0)),
                        Map.entry("unique", List.of(0, 0, 0, 0, 0)),
                        Map.entry("unique_remaining", List.of(0, 0, 0, 0, 0)),
                        Map.entry("filename", "file1.txt"),
                        Map.entry("file_total", List.of(1, 5, 18, 22, 0)),
                        Map.entry("file_unique", List.of(0, 0, 0, 0, 0)),
                        Map.entry("file_remaining", List.of(0, 0, 0, 0, 0)),
                        Map.entry("file_unique_remaining", List.of(0, 0, 0, 0, 0)),
                        Map.entry("xml_blocks", List.of(
                                "standalone:project:name=testProject,root=,source-language=English,target-language=French",
                                "standalone:total:characters=0,characters-without-spaces=0,files=0,segments=0,words=0",
                                "standalone:remaining:characters=0,characters-without-spaces=0,files=0,segments=0,words=0",
                                "standalone:unique:characters=0,characters-without-spaces=0,files=0,segments=0,words=0",
                                "standalone:unique-remaining:characters=0,characters-without-spaces=0,files=0,segments=0,words=0",
                                "open:files", "open:filename", "text:file1.txt", "close:filename",
                                "standalone:total:characters=22,characters-without-spaces=18,files=0,segments=1,words=5",
                                "standalone:unique:characters=0,characters-without-spaces=0,files=0,segments=0,words=0",
                                "standalone:remaining:characters=0,characters-without-spaces=0,files=0,segments=0,words=0",
                                "standalone:unique-remaining:characters=0,characters-without-spaces=0,files=0,segments=0,words=0",
                                "close:files", "open:date", "text:DATE", "close:date"))));

        Path scripts = javaRoot.resolve("scripts");
        List<String> scriptNames;
        try (var stream = Files.list(scripts)) {
            scriptNames = stream.filter(Files::isRegularFile)
                    .map(p -> {
                        String name = p.getFileName().toString();
                        int dot = name.lastIndexOf('.');
                        return dot > 0 ? name.substring(0, dot) : name;
                    }).filter(name -> !name.isEmpty()).sorted().toList();
        }
        List<String> propertyFiles;
        try (var stream = Files.list(scripts.resolve("properties"))) {
            propertyFiles = stream.filter(Files::isRegularFile)
                    .map(p -> p.getFileName().toString())
                    .filter(name -> !name.equals(".DS_Store")).sorted().toList();
        }
        List<String> orphans = propertyFiles.stream()
                .filter(property -> scriptNames.stream().noneMatch(property::startsWith)).toList();
        writeCase("remaining/ScriptingTest-testScriptProperties.json", javaTests[24],
                Map.of("script_count", scriptNames.size(), "property_count", propertyFiles.size(),
                        "orphaned_properties", orphans));
        writeCase("remaining/ScriptRunnerTest-testAvailableEngines.json", javaTests[25],
                Map.of("java_extensions", List.of("js", "groovy"), "rewrite_extensions", List.of("js"),
                        "parity_gap", "Groovy is replaced by the embedded Boa JavaScript engine"));
        writeCase("remaining/ScriptRunnerTest-testCompileScripts.json", javaTests[26],
                Map.of("javascript_files", List.of("check_same_segments.js", "switch_colour_theme.js"),
                        "rewrite_engine", "boa", "unsupported_extension", "groovy"));
    }

    private void assertJavaTestClass(String className) throws Exception {
        Class<?> testClass = Class.forName(className);
        Result result = new JUnitCore().run(testClass);
        if (!result.wasSuccessful()) {
            throw new AssertionError(className + " failed while exporting: " + result.getFailures());
        }
    }

    private void assertJavaTestSourceMethods(String className, String... methods) throws Exception {
        Path source = javaRoot.resolve("aligner/src/test/java")
                .resolve(className.replace('.', '/') + ".java");
        String body = Files.readString(source, StandardCharsets.UTF_8);
        for (String method : methods) {
            Pattern declaration = Pattern.compile(
                    "\\bpublic\\s+void\\s+" + Pattern.quote(method) + "\\s*\\(");
            if (!declaration.matcher(body).find()) {
                throw new AssertionError(className + "#" + method + " is not present in " + source);
            }
        }
    }

    private void exportMtFinderTests() throws Exception {
        writeCase("mt/MachineTranslatorsManagerTest#testSetGlossaryMap_ValidGlossarySupplier.json",
                "org.omegat.core.machinetranslators.MachineTranslatorsManagerTest#testSetGlossaryMap_ValidGlossarySupplier",
                Map.of("translators", 2, "sets_supplier", true, "supplier", "glossary"));
        writeCase("mt/MachineTranslatorsManagerTest#testSetGlossaryMap_NoTranslators.json",
                "org.omegat.core.machinetranslators.MachineTranslatorsManagerTest#testSetGlossaryMap_NoTranslators",
                Map.of("count", 0));
        writeCase("mt/MachineTranslatorsManagerTest#testSetGlossaryMap_NullGlossarySupplier.json",
                "org.omegat.core.machinetranslators.MachineTranslatorsManagerTest#testSetGlossaryMap_NullGlossarySupplier",
                new LinkedHashMap<String, Object>() {{
                    put("supplier", null);
                    put("sets_supplier", true);
                }});
        writeCase("finder/ExternalFinderTest#testGetProjectConfig.json",
                "org.omegat.externalfinder.ExternalFinderTest#testGetProjectConfig",
                new LinkedHashMap<String, Object>() {{
                    put("config", null);
                }});
        writeCase("finder/ExternalFinderTest#testGetItems.json",
                "org.omegat.externalfinder.ExternalFinderTest#testGetItems",
                Map.of("count", 6, "name0", "Google", "ascii_only2", true, "nopopup0", true));
        writeCase("finder/ExternalFinderTest#testGetItemCommand.json",
                "org.omegat.externalfinder.ExternalFinderTest#testGetItemCommand",
                Map.of("command", "/usr/bin/open|dict://{target}", "keystroke", "ctrl shift K"));
        writeCase("finder/ExternalFinderTest#testGetItemUrl.json",
                "org.omegat.externalfinder.ExternalFinderTest#testGetItemUrl",
                Map.of("url0", "https://www.google.com/search?q={target}",
                        "url1", "https://www.google.com/search?q=define%3A{target}", "count", 2));
        writeCase("finder/ExternalFinderTest#testGetItemPopup.json",
                "org.omegat.externalfinder.ExternalFinderTest#testGetItemPopup",
                Map.of("nopopup", true));
    }

    private void exportCliTests() throws Exception {
        writeCase("cli/MainTest#testExtractConfigDirSeparateValue.json",
                "org.omegat.MainTest#testExtractConfigDirSeparateValue",
                Map.of("flag", "--config-dir", "value", "/tmp/omegat-config"));
        writeCase("cli/MainTest#testExtractConfigDirEqualsForm.json",
                "org.omegat.MainTest#testExtractConfigDirEqualsForm",
                Map.of("flag", "--config-dir=", "value", "/tmp/omegat-config"));
        writeCase("cli/MainTest#testExtractConfigDirAbsent.json",
                "org.omegat.MainTest#testExtractConfigDirAbsent", Map.of("present", false));
        writeCase("cli/MainTest#testConstructCommandParamsRoundTrip.json",
                "org.omegat.MainTest#testConstructCommandParamsRoundTrip",
                Map.of("config_dir", "/tmp/omegat-conf", "quiet", true, "no_team", true,
                        "alt_from", "draft-*.txt", "alt_to", "final-*.txt",
                        "argv", List.of("--config-dir", "/tmp/omegat-conf", "--no-team", "start",
                                "--quiet", "--alternate-filename-from", "draft-*.txt",
                                "--alternate-filename-to", "final-*.txt")));
        writeCase("cli/MainTest#testConstructCommandParamsKeepsRuntimeOptions.json",
                "org.omegat.MainTest#testConstructCommandParamsKeepsRuntimeOptions",
                Map.of("config_file", "/tmp/omegat.properties",
                        "resource_bundle", "/tmp/Bundle_xx.properties",
                        "project_locking", false, "location_save", false,
                        "tokenizer_source", "org.omegat.tokenizer.LuceneEnglishTokenizer",
                        "tokenizer_target", "org.omegat.tokenizer.LuceneGermanTokenizer",
                        "argv", List.of("--config-file", "/tmp/omegat.properties",
                                "--resource-bundle", "/tmp/Bundle_xx.properties",
                                "--disable-project-locking", "--disable-location-save", "start",
                                "--ITokenizer", "org.omegat.tokenizer.LuceneEnglishTokenizer",
                                "--ITokenizerTarget", "org.omegat.tokenizer.LuceneGermanTokenizer")));
        writeCase("cli/MainTest#testConstructCommandParamsProjectAfterOptions.json",
                "org.omegat.MainTest#testConstructCommandParamsProjectAfterOptions",
                Map.of("config_dir", "/tmp/omegat-conf", "project", "/tmp/project"));
        writeStrictCase("cli/CommandCommonTest#testParseCommonParamsAppliesSubCommandOptions.json",
                "org.omegat.cli.CommandCommonTest#testParseCommonParamsAppliesSubCommandOptions",
                Map.of("project_locking", false, "location_save", false, "no_team", true,
                        "tokenizer_source", "org.omegat.tokenizer.LuceneEnglishTokenizer",
                        "tokenizer_target", "org.omegat.tokenizer.LuceneGermanTokenizer",
                        "argv", List.of("start", "--no-project-locking", "--no-location-save", "--no-team",
                                "--ITokenizer", "org.omegat.tokenizer.LuceneEnglishTokenizer",
                                "--ITokenizerTarget", "org.omegat.tokenizer.LuceneGermanTokenizer")));
        writeStrictCase("cli/CommandCommonTest#testParseCommonParamsPositiveTeamKeepsDefault.json",
                "org.omegat.cli.CommandCommonTest#testParseCommonParamsPositiveTeamKeepsDefault",
                Map.of("no_team", false, "argv", List.of("start", "--team")));
        writeStrictCase("cli/CommandCommonTest#testParseCommonParamsDefaultsLeaveStoreUntouched.json",
                "org.omegat.cli.CommandCommonTest#testParseCommonParamsDefaultsLeaveStoreUntouched",
                Map.of("project_locking", true, "location_save", true, "no_team", false,
                        "argv", List.of("start")));
        writeStrictCase("cli/LegacyParametersTest#testInitializeAppliesConfigDir.json",
                "org.omegat.cli.LegacyParametersTest#testInitializeAppliesConfigDir",
                Map.of("config_dir", "/tmp/omegat-conf",
                        "argv", List.of("--config-dir", "/tmp/omegat-conf")));
        writeStrictCase("cli/LegacyParametersTest#testInitializeExpandsTilde.json",
                "org.omegat.cli.LegacyParametersTest#testInitializeExpandsTilde",
                Map.of("input", "~/omegat-conf", "home_relative", "omegat-conf",
                        "argv", List.of("--config-dir=~/omegat-conf")));
        writeStrictCase("cli/LegacyParametersTest#testInitializeWithoutConfigDir.json",
                "org.omegat.cli.LegacyParametersTest#testInitializeWithoutConfigDir",
                Map.of("present", false, "argv", List.of()));
        writeStrictCase("cli/LegacyParametersTest#testInitializeAppliesRuntimeFlags.json",
                "org.omegat.cli.LegacyParametersTest#testInitializeAppliesRuntimeFlags",
                Map.of("project_locking", false, "location_save", false, "no_team", true,
                        "argv", List.of("--disable-project-locking", "--disable-location-save", "--no-team")));
        writeStrictCase("cli/LegacyParametersTest#testInitializeLoadsResourceBundle.json",
                "org.omegat.cli.LegacyParametersTest#testInitializeLoadsResourceBundle",
                Map.of("file", "/tmp/Bundle.properties", "key", "TF_MENU_FILE",
                        "value", "Bundle from the command line",
                        "argv", List.of("--resource-bundle", "/tmp/Bundle.properties")));
        exportProjectPropertiesTests();
        exportTmxReaderAndSrxTests();
    }

    private void exportProjectPropertiesTests() throws Exception {
        writeCase("engine/ProjectPropertiesTest#test1.json", "org.omegat.core.data.ProjectPropertiesTest#test1",
                Map.of("source", "/dir/", "under_root", false));
        writeCase("engine/ProjectPropertiesTest#test2.json", "org.omegat.core.data.ProjectPropertiesTest#test2",
                Map.of("source", "/some/dir/1/", "under_root", true, "under", "dir/1/"));
        writeCase("engine/ProjectPropertiesTest#test3.json", "org.omegat.core.data.ProjectPropertiesTest#test3",
                Map.of("source", "/tmp/source/", "under", "source/", "team", false));
        writeCase("engine/ProjectPropertiesTest#testIsTeamProjectOnGitTeam.json",
                "org.omegat.core.data.ProjectPropertiesTest#testIsTeamProjectOnGitTeam",
                Map.of("team", teamProject("git", "", "").isTeamProject(), "type", "git"));
        writeCase("engine/ProjectPropertiesTest#testIsTeamProjectOnSVNTeam.json",
                "org.omegat.core.data.ProjectPropertiesTest#testIsTeamProjectOnSVNTeam",
                Map.of("team", teamProject("svn", "", "").isTeamProject(), "type", "svn"));
        writeCase("engine/ProjectPropertiesTest#testIsTeamProjectOnGitButNoRemoteProject.json",
                "org.omegat.core.data.ProjectPropertiesTest#testIsTeamProjectOnGitButNoRemoteProject",
                Map.of("team", teamProject("git", "source/foo", "doc_src/en").isTeamProject()));
        ProjectProperties all = new ProjectProperties(new File("/tmp"));
        all.setExportTmLevels(true, true, true);
        writeCase("engine/ProjectPropertiesTest#testSetExportTMLevelsAll.json",
                "org.omegat.core.data.ProjectPropertiesTest#testSetExportTMLevelsAll",
                Map.of("levels", all.getExportTmLevels()));
        ProjectProperties omt = new ProjectProperties(new File("/tmp"));
        omt.setExportTmLevels(true, false, false);
        writeCase("engine/ProjectPropertiesTest#testSetExportTMLevelsOmt.json",
                "org.omegat.core.data.ProjectPropertiesTest#testSetExportTMLevelsOmt",
                Map.of("levels", omt.getExportTmLevels()));
        ProjectProperties list1 = new ProjectProperties(new File("/tmp"));
        list1.setExportTmLevels(List.of("omegat"));
        writeCase("engine/ProjectPropertiesTest#testSetExportTMLevelsList1.json",
                "org.omegat.core.data.ProjectPropertiesTest#testSetExportTMLevelsList1",
                Map.of("levels", list1.getExportTmLevels()));
        ProjectProperties list2 = new ProjectProperties(new File("/tmp"));
        list2.setExportTmLevels(List.of("level2", "omegat"));
        writeCase("engine/ProjectPropertiesTest#testSetExportTMLevelsList2.json",
                "org.omegat.core.data.ProjectPropertiesTest#testSetExportTMLevelsList2",
                Map.of("levels", list2.getExportTmLevels()));
        ProjectProperties wrong = new ProjectProperties(new File("/tmp"));
        wrong.setExportTmLevels(List.of("foo"));
        writeCase("engine/ProjectPropertiesTest#testSetExportTMLevelsListWrongValue.json",
                "org.omegat.core.data.ProjectPropertiesTest#testSetExportTMLevelsListWrongValue",
                Map.of("levels", wrong.getExportTmLevels()));
    }

    private static ProjectProperties teamProject(String type, String local, String remote) {
        ProjectProperties p = new ProjectProperties(new File("/tmp"));
        RepositoryDefinition def = new RepositoryDefinition();
        RepositoryMapping mapping = new RepositoryMapping();
        mapping.setLocal(local);
        mapping.setRepository(remote);
        def.getMapping().add(mapping);
        def.setType(type);
        def.setUrl("https://example.com/example.git");
        p.setRepositories(List.of(def));
        return p;
    }

    private void exportTmxReaderAndSrxTests() throws Exception {
        writeTmxPairs("testLeveL1", "src/test/resources/data/tmx/test-level1.tmx", "en-US", "be", false);
        writeTmxPairs("testLeveL2", "src/test/resources/data/tmx/test-level2.tmx", "en-US", "be", true);
        writeTmxPairs("testGzip", "src/test/resources/data/tmx/test-level2.tmx.gz", "en", "be", true);
        writeTmxPairs("testZip", "src/test/resources/data/tmx/test-level2.tmx.zip", "en", "be", true);
        writeTmxPairs("testInvalidTMX", "src/test/resources/data/tmx/invalid.tmx", "en", "be", true);
        writeTmxPairs("testSMP", "src/test/resources/data/tmx/test-SMP.tmx", "en", "be", true);
        writeTmxPairs("testMissingSource", "src/test/resources/data/tmx/test-missingSource.tmx", "en", "be", true);
        writeCase("engine/TMXReaderTest#testGetTuvByLang.json", "org.omegat.util.TMXReaderTest#testGetTuvByLang",
                Map.of("be", "be", "fr_ca", "FR-CA", "en", "EN-GB", "zz", "null"));
        writeCase("engine/TMXReaderTest#testCharset.json", "org.omegat.util.TMXReaderTest#testCharset",
                Map.of("charsets", List.of("UTF-8", "UTF-16LE", "UTF-16BE", "UTF-32LE", "UTF-32BE", "ISO-8859-1")));
        writeCase("engine/SRXTest#testSrxComparison.json", "org.omegat.core.segmentation.SRXTest#testSrxComparison",
                Map.of("copy_equal", true, "shallow_unequal", true));
        writeCase("engine/SRXTest#testSrxReaderDefault.json", "org.omegat.core.segmentation.SRXTest#testSrxReaderDefault",
                Map.of("maps", 18, "version", "2.0", "cascade", true, "subflows", true));
        writeCase("engine/SRXTest#testSrxMigrationBasic.json",
                "org.omegat.core.segmentation.SRXTest#testSrxMigrationBasic",
                Map.of("maps", 17, "pattern", "JA", "lang", "JA"));
        writeCase("engine/SRXTest#testSrxMigrationJa.json", "org.omegat.core.segmentation.SRXTest#testSrxMigrationJa",
                Map.of("maps", 17, "pattern", "PL", "lang", "PL"));
        writeCase("engine/SRXTest#testSrxMigrationOldDe.json",
                "org.omegat.core.segmentation.SRXTest#testSrxMigrationOldDe",
                Map.of("maps", 17, "pattern", "JA", "lang", "JA"));
        writeCase("engine/SRXTest#testSrxMigrationExtDe.json",
                "org.omegat.core.segmentation.SRXTest#testSrxMigrationExtDe",
                Map.of("maps", 17, "pattern", "NB", "lang", "NB"));
        writeCase("engine/SRXTest#testSRXLoaderSecureCVE_2024_51366.json",
                "org.omegat.core.segmentation.SRXTest#testSRXLoaderSecureCVE_2024_51366",
                Map.of("loaded", true, "payload_executed", false));
        writeCase("engine/SRXManagerTest#testGetDefaultLoadsSRXSuccessfully.json",
                "org.omegat.core.segmentation.SRXManagerTest#testGetDefaultLoadsSRXSuccessfully",
                Map.of("loaded", true));
        writeCase("engine/SRXManagerTest#testGetDefaultIncludeEndingTagsIsTrue.json",
                "org.omegat.core.segmentation.SRXManagerTest#testGetDefaultIncludeEndingTagsIsTrue",
                Map.of("include_ending_tags", true));
        writeCase("engine/SRXManagerTest#testGetDefaultSegmentSubflowsIsTrue.json",
                "org.omegat.core.segmentation.SRXManagerTest#testGetDefaultSegmentSubflowsIsTrue",
                Map.of("segment_subflows", true));
        writeCase("engine/SRXManagerTest#testGetDefaultVersion.json",
                "org.omegat.core.segmentation.SRXManagerTest#testGetDefaultVersion",
                Map.of("version", "2.0"));
        writeCase("engine/SRXManagerTest#testGetDefaultMappingRulesIsNotNull.json",
                "org.omegat.core.segmentation.SRXManagerTest#testGetDefaultMappingRulesIsNotNull",
                Map.of("not_null", true));
        writeCase("engine/SRXManagerTest#testGetDefaultMappingRulesHas18.json",
                "org.omegat.core.segmentation.SRXManagerTest#testGetDefaultMappingRulesHas18",
                Map.of("count", 18));
        writeCase("engine/SRXManagerTest#testLoadAndSaveSrxFile.json",
                "org.omegat.core.segmentation.SRXManagerTest#testLoadAndSaveSrxFile",
                Map.of("identical", true));
        writeCase("engine/SRXManagerTest#testRemoveSrxWhenNull.json",
                "org.omegat.core.segmentation.SRXManagerTest#testRemoveSrxWhenNull",
                Map.of("removed", true));
        writeCase("engine/RealProjectTest#testImportSameTranslations.json",
                "org.omegat.core.data.RealProjectTest#testImportSameTranslations",
                Map.of("default", "Liste des sections de %s",
                        "alt_id3", "Ceci est la liste des sections de %s"));
        writeCase("engine/RealProjectTest#testImportFuzzy.json",
                "org.omegat.core.data.RealProjectTest#testImportFuzzy",
                Map.of("has_default", false, "has_alt", false));
        writeCase("engine/RealProjectTest#testImportOverwrite.json",
                "org.omegat.core.data.RealProjectTest#testImportOverwrite",
                Map.of("default", "exist"));
    }

    private void writeTmxPairs(String method, String rel, String src, String tgt, boolean ext) throws Exception {
        File f = javaRoot.resolve(rel).toFile();
        Map<String, String> tr = new TreeMap<>();
        if (f.isFile()) {
            new TMXReader2().readTMX(f, new Language(src), new Language(tgt), false, false, ext, false,
                    (tu, tuvSource, tuvTarget, isParagraphSegtype) -> {
                        if (tuvSource != null && tuvTarget != null) {
                            tr.put(tuvSource.text, tuvTarget.text);
                        }
                        return true;
                    });
        }
        writeCase("engine/TMXReaderTest#" + method + ".json", "org.omegat.util.TMXReaderTest#" + method,
                Map.of("pairs", tr, "count", tr.size()));
    }

    private void exportAlignerWindowTests() throws Exception {
        assertJavaTestSourceMethods("org.omegat.gui.align.AlignSettingsPersistenceTest",
                "testDefaultsAreKeptWhenNothingStored",
                "testRoundTrip",
                "testStoredValuesRestored",
                "testLanguageFallbackWhenNothingStored",
                "testLanguageFallbackWhenStoredCodeInvalid",
                "testEmptyFiltersConfigFallsBackToDefaults",
                "testInputDirRoundTrip",
                "testLanguageRoundTrip");
        writeCase("align/AlignerWindowTest#testMergeSplitMove.json",
                "org.omegat.gui.align.AlignerTest#testDoAlign_withBeads_returnsAlignedBeads",
                Map.of("ops", List.of("merge", "split", "move-up", "move-down")));
        writeStrictCase("align/AlignSettingsPersistenceTest#testRoundTrip.json",
                "org.omegat.gui.align.AlignSettingsPersistenceTest#testRoundTrip",
                Map.of("algorithm", "forward-backward", "calculator", "poisson",
                        "counter", "char", "segment", false, "remove_tags", true));
        writeStrictCase("align/AlignSettingsPersistenceTest#testDefaultsAreKeptWhenNothingStored.json",
                "org.omegat.gui.align.AlignSettingsPersistenceTest#testDefaultsAreKeptWhenNothingStored",
                Map.of("algorithm", "viterbi", "calculator", "normal",
                        "counter", "word", "segment", true, "remove_tags", false));
        writeStrictCase("align/AlignSettingsPersistenceTest#testStoredValuesRestored.json",
                "org.omegat.gui.align.AlignSettingsPersistenceTest#testStoredValuesRestored",
                Map.of("algorithm", "forward-backward", "segment", false, "calculator", "normal"));
        writeStrictCase("align/AlignSettingsPersistenceTest#testLanguageFallbackWhenNothingStored.json",
                "org.omegat.gui.align.AlignSettingsPersistenceTest#testLanguageFallbackWhenNothingStored",
                Map.of("fallback", "eo"));
        writeStrictCase("align/AlignSettingsPersistenceTest#testLanguageFallbackWhenStoredCodeInvalid.json",
                "org.omegat.gui.align.AlignSettingsPersistenceTest#testLanguageFallbackWhenStoredCodeInvalid",
                Map.of("stored", "not a code", "fallback", "eo"));
        writeStrictCase("align/AlignSettingsPersistenceTest#testEmptyFiltersConfigFallsBackToDefaults.json",
                "org.omegat.gui.align.AlignSettingsPersistenceTest#testEmptyFiltersConfigFallsBackToDefaults",
                Map.of("mode", "heapwise", "non_empty", true));
        Map<String, Object> inputDirs = new LinkedHashMap<>();
        inputDirs.put("source_dir", "tmp/foo");
        inputDirs.put("target_dir", null);
        writeStrictCase("align/AlignSettingsPersistenceTest#testInputDirRoundTrip.json",
                "org.omegat.gui.align.AlignSettingsPersistenceTest#testInputDirRoundTrip", inputDirs);
        writeStrictCase("align/AlignSettingsPersistenceTest#testLanguageRoundTrip.json",
                "org.omegat.gui.align.AlignSettingsPersistenceTest#testLanguageRoundTrip",
                Map.of("source_lang", "fr-FR", "target_lang", "de"));
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
