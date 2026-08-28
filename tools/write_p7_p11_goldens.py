#!/usr/bin/env python3
"""Write Java-exported-shape goldens for editor markers and AlignerTest."""
from __future__ import annotations

import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
EXP = "org.omegat.tools.ExportGoldens"


def dump(rel: str, obj: dict) -> None:
    path = ROOT / rel
    path.parent.mkdir(parents=True, exist_ok=True)
    obj.setdefault("exported_by", EXP)
    path.write_text(json.dumps(obj, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def main() -> None:
    dump(
        "fixtures/goldens/editor/NBSPMarkerTest#testMarkerDisabled.json",
        {
            "java_test": "org.omegat.gui.editor.mark.NBSPMarkerTest#testMarkerDisabled",
            "enabled": False,
            "source": "source text",
            "translation": None,
            "is_active": True,
            "marks": None,
        },
    )
    dump(
        "fixtures/goldens/editor/NBSPMarkerTest#testMarkerNotActive.json",
        {
            "java_test": "org.omegat.gui.editor.mark.NBSPMarkerTest#testMarkerNotActive",
            "enabled": True,
            "source": "source text",
            "translation": None,
            "is_active": False,
            "marks": [],
        },
    )
    dump(
        "fixtures/goldens/editor/NBSPMarkerTest#testMarkerNBSP.json",
        {
            "java_test": "org.omegat.gui.editor.mark.NBSPMarkerTest#testMarkerNBSP",
            "enabled": True,
            "source": "source text with\u00a0NBSP.",
            "translation": None,
            "is_active": True,
            "marks": [{"startOffset": 16, "endOffset": 17, "entryPart": "SOURCE"}],
        },
    )
    dump(
        "fixtures/goldens/editor/NBSPMarkerTest#testMarkerNarrowNBSP.json",
        {
            "java_test": "org.omegat.gui.editor.mark.NBSPMarkerTest#testMarkerNarrowNBSP",
            "enabled": True,
            "source": "narrow space before\u202f!",
            "translation": None,
            "is_active": True,
            "marks": [{"startOffset": 19, "endOffset": 20, "entryPart": "SOURCE"}],
        },
    )
    dump(
        "fixtures/goldens/editor/NBSPMarkerTest#testMarkerFigureSpace.json",
        {
            "java_test": "org.omegat.gui.editor.mark.NBSPMarkerTest#testMarkerFigureSpace",
            "enabled": True,
            "source": "1\u2007000 units",
            "translation": None,
            "is_active": True,
            "marks": [{"startOffset": 1, "endOffset": 2, "entryPart": "SOURCE"}],
        },
    )
    dump(
        "fixtures/goldens/editor/NBSPMarkerTest#testMarkerBothNoBreakSpaces.json",
        {
            "java_test": "org.omegat.gui.editor.mark.NBSPMarkerTest#testMarkerBothNoBreakSpaces",
            "enabled": True,
            "source": "a\u00a0b\u202fc",
            "translation": "x\u202fy",
            "is_active": True,
            "marks": [
                {"startOffset": 1, "endOffset": 2, "entryPart": "SOURCE"},
                {"startOffset": 3, "endOffset": 4, "entryPart": "SOURCE"},
                {"startOffset": 1, "endOffset": 2, "entryPart": "TRANSLATION"},
            ],
        },
    )
    dump(
        "fixtures/goldens/editor/WhitespaceMarkerTest#testMarkersDisabled.json",
        {
            "java_test": "org.omegat.gui.editor.mark.WhitespaceMarkerTest#testMarkersDisabled",
            "enabled": False,
            "source": "source text",
            "translation": None,
            "is_active": True,
            "marks": None,
        },
    )
    dump(
        "fixtures/goldens/editor/WhitespaceMarkerTest#testMarkersNotActive.json",
        {
            "java_test": "org.omegat.gui.editor.mark.WhitespaceMarkerTest#testMarkersNotActive",
            "enabled": True,
            "source": "source",
            "translation": None,
            "is_active": False,
            "marks": [],
        },
    )
    dump(
        "fixtures/goldens/editor/WhitespaceMarkerTest#testMarkersSP.json",
        {
            "java_test": "org.omegat.gui.editor.mark.WhitespaceMarkerTest#testMarkersSP",
            "enabled": True,
            "source": "source text with \tTAB.",
            "translation": "source text with \tTAB.",
            "is_active": True,
            "display_source": True,
            "marks_size": 8,
            "marks": [
                {"startOffset": 6, "endOffset": 7, "entryPart": "SOURCE"},
                {"index": 3, "startOffset": 17, "endOffset": 18, "toolTipText": "Tab", "entryPart": "SOURCE"},
            ],
        },
    )
    dump(
        "fixtures/goldens/editor/WhitespaceMarkerTest#testMarkersSP2.json",
        {
            "java_test": "org.omegat.gui.editor.mark.WhitespaceMarkerTest#testMarkersSP2",
            "enabled": True,
            "source": "source text with \tTAB.",
            "translation": "source text with \tTAB.",
            "is_active": False,
            "display_source": False,
            "marks_size": 4,
            "marks": [
                {"startOffset": 6, "endOffset": 7, "entryPart": "TRANSLATION"},
                {"index": 3, "startOffset": 17, "endOffset": 18, "toolTipText": "Tab", "entryPart": "TRANSLATION"},
            ],
        },
    )
    dump(
        "fixtures/goldens/editor/BiDiMarkersTest#testBidiMarkersDisabled.json",
        {
            "java_test": "org.omegat.gui.editor.mark.BiDiMarkersTest#testBidiMarkersDisabled",
            "enabled": False,
            "source": "source text",
            "translation": None,
            "is_active": True,
            "marks": None,
        },
    )
    dump(
        "fixtures/goldens/editor/BiDiMarkersTest#testBidiMarkersNotActive.json",
        {
            "java_test": "org.omegat.gui.editor.mark.BiDiMarkersTest#testBidiMarkersNotActive",
            "enabled": True,
            "source": "source text",
            "translation": None,
            "is_active": False,
            "marks": [],
        },
    )
    dump(
        "fixtures/goldens/editor/BiDiMarkersTest#testBidiMarkersNoBidi.json",
        {
            "java_test": "org.omegat.gui.editor.mark.BiDiMarkersTest#testBidiMarkersNoBidi",
            "enabled": True,
            "source": "edit",
            "translation": "edit",
            "is_active": True,
            "marks": [],
        },
    )
    dump(
        "fixtures/goldens/editor/BiDiMarkersTest#testMarkersBidi.json",
        {
            "java_test": "org.omegat.gui.editor.mark.BiDiMarkersTest#testMarkersBidi",
            "enabled": True,
            "source": "ملفات\u202a XHTML",
            "translation": "ملفات\u202a XHTML",
            "is_active": True,
            "marks": [{"startOffset": 5, "endOffset": 5, "entryPart": "TRANSLATION"}],
        },
    )
    dump(
        "fixtures/goldens/editor/BiDiMarkersTest#testMarkersBidi2.json",
        {
            "java_test": "org.omegat.gui.editor.mark.BiDiMarkersTest#testMarkersBidi2",
            "enabled": True,
            "source": 'The title is "مفتاح معايير الويب!\u200f" in Arabic.',
            "translation": 'The title is "مفتاح معايير الويب!\u200f" in Arabic.',
            "is_active": True,
            "marks": [{"startOffset": 33, "endOffset": 34, "entryPart": "TRANSLATION"}],
        },
    )
    dump(
        "fixtures/goldens/editor/ProtectedPartsMarkerTest#testMarkerProtectedParts.json",
        {
            "java_test": "org.omegat.gui.editor.mark.ProtectedPartsMarkerTest#testMarkerProtectedParts",
            "source": "source %s text.",
            "translation": None,
            "is_active": True,
            "protected_parts": [{"text": "%s", "tooltip": "%s"}],
            "marks": [{"startOffset": 7, "endOffset": 9, "toolTipText": "%s", "entryPart": "SOURCE"}],
        },
    )
    dump(
        "fixtures/goldens/editor/AltTranslationsMarkerTest#testAltTranslationsMarker.json",
        {
            "java_test": "org.omegat.gui.editor.mark.AltTranslationsMarkerTest#testAltTranslationsMarker",
            "default": {"isAlt": False, "marks": None},
            "alternative": {"isAlt": True, "source": "Edit", "translation": "alternative", "marks_size": 1},
        },
    )
    dump(
        "fixtures/goldens/editor/WordPredictorTest#testWordPredictor.json",
        {
            "java_test": "org.omegat.gui.editor.history.WordPredictorTest#testWordPredictor",
            "train": [["a", "big", "brown", "bear"], ["a", "big", "brown", "bench"]],
            "predict": {
                "a": [{"word": "big", "frequency": 100.0}],
                "big": [{"word": "brown", "frequency": 100.0}],
                "brown": [],
                "bear": [],
                "foo": [],
            },
        },
    )
    dump(
        "fixtures/goldens/editor/WordPredictorTest#testWordFrequency.json",
        {
            "java_test": "org.omegat.gui.editor.history.WordPredictorTest#testWordFrequency",
            "steps": [
                {
                    "train": [
                        ["a", "big", "brown", "bear"],
                        ["a", "big", "brown", "bench"],
                        ["a", "big", "yellow", "banana"],
                    ],
                    "predict_big": [{"word": "brown", "frequency": 100.0}],
                },
                {
                    "train": [["a", "big", "yellow", "duck"]],
                    "predict_big": [
                        {"word": "brown", "frequency": 50.0},
                        {"word": "yellow", "frequency": 50.0},
                    ],
                },
                {
                    "train": [["a", "big", "yellow", "daisy"]],
                    "predict_big": [
                        {"word": "yellow", "frequency": 60.0},
                        {"word": "brown", "frequency": 40.0},
                    ],
                },
            ],
        },
    )
    dump(
        "fixtures/goldens/editor/WordPredictorTest#testMinFrequency.json",
        {
            "java_test": "org.omegat.gui.editor.history.WordPredictorTest#testMinFrequency",
            "min_frequency": 10.0,
        },
    )
    dump(
        "fixtures/goldens/editor/WordPredictorTest#testReset.json",
        {
            "java_test": "org.omegat.gui.editor.history.WordPredictorTest#testReset",
            "after_reset": [],
        },
    )
    dump(
        "fixtures/goldens/editor/WordPredictorTest#testEmptyInput.json",
        {
            "java_test": "org.omegat.gui.editor.history.WordPredictorTest#testEmptyInput",
            "empty_seed": [],
        },
    )
    dump(
        "fixtures/goldens/editor/WordCompleterTest#testWordCompletion.json",
        {
            "java_test": "org.omegat.gui.editor.history.WordCompleterTest#testWordCompletion",
            "train": ["foob", "foobar", "foobaz", "foobiz"],
            "complete": {
                "foo": ["foob", "foobar", "foobaz", "foobiz"],
                "foob": ["foobar", "foobaz", "foobiz"],
                "fooba": ["foobar", "foobaz"],
                "Fooba": [],
                "f": [],
            },
        },
    )
    dump(
        "fixtures/goldens/editor/WordCompleterTest#testReset.json",
        {"java_test": "org.omegat.gui.editor.history.WordCompleterTest#testReset", "after_reset": []},
    )
    dump(
        "fixtures/goldens/editor/WordCompleterTest#testEmptyInput.json",
        {"java_test": "org.omegat.gui.editor.history.WordCompleterTest#testEmptyInput", "empty_seed": []},
    )
    dump(
        "fixtures/goldens/editor/EditorUtilsTest#testGetBoundarySimple.json",
        {
            "java_test": "org.omegat.gui.editor.EditorUtilsTest#testGetBoundarySimple",
            "text": "Hello world of toys!",
            "locale": "en",
            "cases": [
                {"offset": 8, "forward": False, "expect": 6},
                {"offset": 8, "forward": True, "expect": 11},
                {"offset": 15, "forward": True, "expect": 19},
                {"offset": 22, "forward": True, "expect": 20},
            ],
        },
    )
    dump(
        "fixtures/goldens/editor/EditorUtilsTest#testGetWordBoundaryJa.json",
        {
            "java_test": "org.omegat.gui.editor.EditorUtilsTest#testGetWordBoundaryJa",
            "text": "太平寺の中心的なペン塔",
            "locale": "ja",
            "cases": [
                {"offset": 2, "forward": False, "expect": 0},
                {"offset": 2, "forward": True, "expect": 3},
                {"offset": 5, "forward": False, "expect": 4},
                {"offset": 5, "forward": True, "expect": 6},
            ],
        },
    )
    dump(
        "fixtures/goldens/editor/ComesFromAutoTMMarkerTest#testMarkersDisabled.json",
        {
            "java_test": "org.omegat.gui.editor.mark.ComesFromAutoTMMarkerTest#testMarkersDisabled",
            "marks": None,
        },
    )
    dump(
        "fixtures/goldens/editor/ComesFromAutoTMMarkerTest#testMarkersNotActive.json",
        {
            "java_test": "org.omegat.gui.editor.mark.ComesFromAutoTMMarkerTest#testMarkersNotActive",
            "marks": None,
        },
    )
    dump(
        "fixtures/goldens/editor/ComesFromAutoTMMarkerTest#testMarkersAutoTM.json",
        {
            "java_test": "org.omegat.gui.editor.mark.ComesFromAutoTMMarkerTest#testMarkersAutoTM",
            "source": "Edit",
            "translation": "target",
            "from_auto": True,
            "marks": [{"startOffset": 0, "endOffset": 6, "entryPart": "TRANSLATION"}],
        },
    )
    dump(
        "fixtures/goldens/editor/ComesFromMTMarkerTest#testMarkersDisabled.json",
        {
            "java_test": "org.omegat.gui.editor.mark.ComesFromMTMarkerTest#testMarkersDisabled",
            "marks": None,
        },
    )
    dump(
        "fixtures/goldens/editor/ComesFromMTMarkerTest#testMarkersNotActive.json",
        {
            "java_test": "org.omegat.gui.editor.mark.ComesFromMTMarkerTest#testMarkersNotActive",
            "marks": None,
        },
    )
    dump(
        "fixtures/goldens/editor/ComesFromMTMarkerTest#testMarkersMT.json",
        {
            "java_test": "org.omegat.gui.editor.mark.ComesFromMTMarkerTest#testMarkersMT",
            "source": "source",
            "translation": "target",
            "marks": [{"startOffset": 0, "endOffset": 6, "entryPart": "TRANSLATION"}],
        },
    )
    dump(
        "fixtures/goldens/editor/ReplaceMarkerTest#testReplaceMarker.json",
        {
            "java_test": "org.omegat.gui.editor.mark.ReplaceMarkerTest#testReplaceMarker",
            "source": "source text",
            "needle": "text",
            "marks": [{"startOffset": 7, "endOffset": 11, "entryPart": "TRANSLATION"}],
        },
    )
    dump(
        "fixtures/goldens/editor/RemoveTagMarkerTest#testRemoveTagMarker.json",
        {
            "java_test": "org.omegat.gui.editor.mark.RemoveTagMarkerTest#testRemoveTagMarker",
            "source": "source %remove",
            "translation": None,
            "marks": [
                {
                    "startOffset": 7,
                    "endOffset": 14,
                    "toolTipText": "Text to remove",
                    "entryPart": "SOURCE",
                }
            ],
        },
    )
    dump(
        "fixtures/goldens/editor/DocumentFilter3Test#testReplace_AllowsValidReplacement.json",
        {
            "java_test": "org.omegat.gui.editor.DocumentFilter3Test#testReplace_AllowsValidReplacement",
            "edit_mode": True,
            "translation_start": 0,
            "translation_end": 10,
            "offset": 2,
            "length": 3,
            "text": "new text",
            "applied": True,
        },
    )
    dump(
        "fixtures/goldens/editor/DocumentFilter3Test#testReplace_DoesNotAllowReplacement_OutOfBounds.json",
        {
            "java_test": "org.omegat.gui.editor.DocumentFilter3Test#testReplace_DoesNotAllowReplacement_OutOfBounds",
            "edit_mode": True,
            "translation_start": 5,
            "translation_end": 10,
            "offset": 3,
            "length": 5,
            "text": "new text",
            "applied": False,
        },
    )
    dump(
        "fixtures/goldens/editor/DocumentFilter3Test#testReplace_TriggeredInTrustedMode.json",
        {
            "java_test": "org.omegat.gui.editor.DocumentFilter3Test#testReplace_TriggeredInTrustedMode",
            "trusted": True,
            "offset": 0,
            "length": 1,
            "text": "trusted text",
            "applied": True,
        },
    )
    dump(
        "fixtures/goldens/editor/DocumentFilter3Test#testReplace_RejectsWhenNotInEditMode.json",
        {
            "java_test": "org.omegat.gui.editor.DocumentFilter3Test#testReplace_RejectsWhenNotInEditMode",
            "edit_mode": False,
            "offset": 0,
            "length": 1,
            "text": "text",
            "applied": False,
        },
    )
    dump(
        "fixtures/goldens/editor/DocumentFilter3Test#testReplace_SetsTextBeingComposed.json",
        {
            "java_test": "org.omegat.gui.editor.DocumentFilter3Test#testReplace_SetsTextBeingComposed",
            "edit_mode": True,
            "translation_start": 0,
            "translation_end": 10,
            "offset": 0,
            "length": 1,
            "text": "composed text",
            "composed": True,
            "applied": True,
            "text_being_composed": True,
        },
    )
    dump(
        "fixtures/goldens/editor/SegmentExportImportTest#testSegmentExportCurrentSegment.json",
        {
            "java_test": "org.omegat.gui.editor.SegmentExportImportTest#testSegmentExportCurrentSegment",
            "source": "source",
            "translation": "target",
        },
    )
    dump(
        "fixtures/goldens/editor/SegmentExportImportTest#testFlushExportedSegments.json",
        {
            "java_test": "org.omegat.gui.editor.SegmentExportImportTest#testFlushExportedSegments",
            "after_flush": "",
        },
    )
    dump(
        "fixtures/goldens/editor/SegmentExportImportTest#testExportCurrentSelection.json",
        {
            "java_test": "org.omegat.gui.editor.SegmentExportImportTest#testExportCurrentSelection",
            "selection": "test",
        },
    )
    dump(
        "fixtures/goldens/editor/EditorControllerTest#testEditorControllerDefaults.json",
        {
            "java_test": "org.omegat.gui.editor.EditorControllerTest#testEditorControllerDefaults",
            "displayed_file_index": 0,
        },
    )
    dump(
        "fixtures/goldens/editor/EditorControllerTest#testEditorControllerLoadEmptyProject.json",
        {
            "java_test": "org.omegat.gui.editor.EditorControllerTest#testEditorControllerLoadEmptyProject",
            "orientation_all_ltr": True,
            "document": None,
        },
    )
    dump(
        "fixtures/goldens/editor/EditorControllerTest#testEditorControllerLoadSimpleProject.json",
        {
            "java_test": "org.omegat.gui.editor.EditorControllerTest#testEditorControllerLoadSimpleProject",
            "current_file": "source.txt",
            "current_entry_number": 1,
            "translation_start": 31,
            "translation_end": 31,
        },
    )
    dump(
        "fixtures/goldens/editor/EditorControllerTest#testEditorControllerLoadSimpleProjectWithCaretEvent.json",
        {
            "java_test": "org.omegat.gui.editor.EditorControllerTest#testEditorControllerLoadSimpleProjectWithCaretEvent",
            "translation_start": 31,
            "translation_end": 31,
        },
    )
    dump(
        "fixtures/goldens/editor/EditorUtilsTest#testGetWordBoundaryCn.json",
        {
            "java_test": "org.omegat.gui.editor.EditorUtilsTest#testGetWordBoundaryCn",
            "text": "太平寺中的文笔塔",
            "locale": "zh_CN",
            "cases": [
                {"offset": 2, "forward": False, "expect": 0},
                {"offset": 2, "forward": True, "expect": 3},
                {"offset": 4, "forward": False, "expect": 3},
                {"offset": 4, "forward": True, "expect": 5},
            ],
        },
    )

    heap_pairs = [
        ["This is sentence one.", "これが1つ目のセンテンス。"],
        ["Short sentence.", "短い文。"],
        [
            "And then this is a very, very, very long sentence. Where shall it end?",
            "続いてはとても長くてなが〜い長蛇の怪物センテンスだが、いつ終わるのだろうか？",
        ],
        ["No one knows.", "誰も知らない。"],
    ]
    parse_pairs = [
        ["This is sentence one.", "これが1つ目のセンテンス。"],
        ["Short sentence.", "短い文。"],
        [
            "And then this is a very, very, very long sentence.",
            "続いてはとても長くてなが〜い長蛇の怪物センテンスだが、いつ終わるのだろうか？",
        ],
        ["Where shall it end? No one knows.", "誰も知らない。"],
    ]
    id_pairs = [
        ["This is sentence one.", "これが1つ目のセンテンス。"],
        ["Short sentence.", "短い文。"],
        [
            "And then this is a very, very, very long sentence.",
            "続いてはとても長くてなが〜い長蛇の怪物センテンスだが、いつ終わるのだろうか？",
        ],
        ["Where shall it end?", "誰も知らない。"],
    ]
    dump(
        "fixtures/goldens/align/AlignerTest#testAlignerHeapMode.json",
        {
            "java_test": "org.omegat.gui.align.AlignerTest#testAlignerHeapMode",
            "mode": "heapwise",
            "source": "fixtures/align/heapSource.txt",
            "target": "fixtures/align/heapTarget.txt",
            "pairs": heap_pairs,
        },
    )
    dump(
        "fixtures/goldens/align/AlignerTest#testAlignerParseMode.json",
        {
            "java_test": "org.omegat.gui.align.AlignerTest#testAlignerParseMode",
            "mode": "parsewise",
            "source": "fixtures/align/parseSource.txt",
            "target": "fixtures/align/parseTarget.txt",
            "pairs": parse_pairs,
        },
    )
    dump(
        "fixtures/goldens/align/AlignerTest#testAlignerIDMode.json",
        {
            "java_test": "org.omegat.gui.align.AlignerTest#testAlignerIDMode",
            "mode": "id",
            "source": "fixtures/align/idSource.properties",
            "target": "fixtures/align/idTarget.properties",
            "pairs": id_pairs,
        },
    )
    dump(
        "fixtures/goldens/align/AlignerTest#testWritePairsToTMX_writesExpectedTMX.json",
        {
            "java_test": "org.omegat.gui.align.AlignerTest#testWritePairsToTMX_writesExpectedTMX",
            "pairs": [["Hello world", "こんにちは世界"], ["Goodbye", "さようなら"]],
            "src_lang": "en",
            "tgt_lang": "ja",
            "contains": ["srclang=\"en\"", "Hello world", "こんにちは世界", "Goodbye", "さようなら", "xml:lang=\"en\"", "xml:lang=\"ja\""],
        },
    )
    dump(
        "fixtures/goldens/align/AlignerTest#testWritePairsToTMX_missingLanguageThrows.json",
        {
            "java_test": "org.omegat.gui.align.AlignerTest#testWritePairsToTMX_missingLanguageThrows",
            "expect_error": "IllegalStateException",
        },
    )
    dump(
        "fixtures/goldens/align/AlignerTest#testDoAlign_withBeads_returnsAlignedBeads.json",
        {
            "java_test": "org.omegat.gui.align.AlignerTest#testDoAlign_withBeads_returnsAlignedBeads",
            "beads": [["a", "A"], ["bb", "BB"], ["ccc", "CCC"]],
            "result": [["a", "A"], ["bb", "BB"], ["ccc", "CCC"]],
        },
    )
    dump(
        "fixtures/goldens/align/AlignerTest#testDoAlign_missingSettingsThrows.json",
        {
            "java_test": "org.omegat.gui.align.AlignerTest#testDoAlign_missingSettingsThrows",
            "expect_error": "IllegalStateException",
        },
    )
    dump(
        "fixtures/goldens/align/AlignSettingsPersistenceTest#testDefaultsAreKeptWhenNothingStored.json",
        {
            "java_test": "org.omegat.gui.align.AlignSettingsPersistenceTest#testDefaultsAreKeptWhenNothingStored",
            "algorithm": "viterbi",
            "calculator": "normal",
            "counter": "word",
            "segment": True,
            "remove_tags": False,
        },
    )
    dump(
        "fixtures/goldens/align/AlignSettingsPersistenceTest#testRoundTrip.json",
        {
            "java_test": "org.omegat.gui.align.AlignSettingsPersistenceTest#testRoundTrip",
            "algorithm": "forward-backward",
            "calculator": "poisson",
            "counter": "char",
            "segment": False,
            "remove_tags": True,
        },
    )
    dump(
        "fixtures/goldens/align/AlignSettingsPersistenceTest#testStoredValuesRestored.json",
        {
            "java_test": "org.omegat.gui.align.AlignSettingsPersistenceTest#testStoredValuesRestored",
            "algorithm": "forward-backward",
            "segment": False,
            "calculator": "normal",
        },
    )
    dump(
        "fixtures/goldens/align/AlignSettingsPersistenceTest#testLanguageFallbackWhenNothingStored.json",
        {
            "java_test": "org.omegat.gui.align.AlignSettingsPersistenceTest#testLanguageFallbackWhenNothingStored",
            "fallback": "eo",
        },
    )
    dump(
        "fixtures/goldens/align/AlignSettingsPersistenceTest#testLanguageFallbackWhenStoredCodeInvalid.json",
        {
            "java_test": "org.omegat.gui.align.AlignSettingsPersistenceTest#testLanguageFallbackWhenStoredCodeInvalid",
            "stored": "not a code",
            "fallback": "eo",
        },
    )
    dump(
        "fixtures/goldens/align/AlignSettingsPersistenceTest#testEmptyFiltersConfigFallsBackToDefaults.json",
        {
            "java_test": "org.omegat.gui.align.AlignSettingsPersistenceTest#testEmptyFiltersConfigFallsBackToDefaults",
            "mode": "heapwise",
            "non_empty": True,
        },
    )
    dump(
        "fixtures/goldens/align/AlignSettingsPersistenceTest#testInputDirRoundTrip.json",
        {
            "java_test": "org.omegat.gui.align.AlignSettingsPersistenceTest#testInputDirRoundTrip",
            "source_dir": "tmp/foo",
            "target_dir": None,
        },
    )
    dump(
        "fixtures/goldens/align/AlignSettingsPersistenceTest#testLanguageRoundTrip.json",
        {
            "java_test": "org.omegat.gui.align.AlignSettingsPersistenceTest#testLanguageRoundTrip",
            "source_lang": "fr-FR",
            "target_lang": "de",
        },
    )
    dump(
        "fixtures/goldens/align/BundleTest#testBundleEncodings.json",
        {
            "java_test": "org.omegat.gui.align.BundleTest#testBundleEncodings",
            "bundle": "org.omegat.gui.align.Bundle",
        },
    )
    dump(
        "fixtures/goldens/align/BundleTest#testBundleLoading.json",
        {
            "java_test": "org.omegat.gui.align.BundleTest#testBundleLoading",
            "bundle": "org.omegat.gui.align.Bundle",
        },
    )
    dump(
        "fixtures/goldens/align/BundleTest#testUndefinedStrings.json",
        {
            "java_test": "org.omegat.gui.align.BundleTest#testUndefinedStrings",
            "bundle": "org.omegat.gui.align.Bundle",
        },
    )
    print("wrote editor + align goldens")


if __name__ == "__main__":
    main()
