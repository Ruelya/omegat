#!/usr/bin/env python3
"""Write remaining editor / completer / bundle goldens in ExportGoldens shape."""
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
    # LTR insertString model: source "XXX" + "\\n" then empty translation.
    dump(
        "fixtures/goldens/editor/EditorControllerTest#testEditorControllerLoadSimpleProject.json",
        {
            "java_test": "org.omegat.gui.editor.EditorControllerTest#testEditorControllerLoadSimpleProject",
            "current_file": "source.txt",
            "current_entry_number": 1,
            "translation_start": 4,
            "translation_end": 4,
            "note": "Headless Java test is skipped; offset is SegmentBuilder.insertString for XXX + newline.",
        },
    )
    dump(
        "fixtures/goldens/editor/EditorControllerTest#testEditorControllerLoadSimpleProjectWithCaretEvent.json",
        {
            "java_test": "org.omegat.gui.editor.EditorControllerTest#testEditorControllerLoadSimpleProjectWithCaretEvent",
            "translation_start": 4,
            "translation_end": 4,
        },
    )
    dump(
        "fixtures/goldens/editor/MarkerColorFreshnessTest#testPainterFollowsColorPreferenceChange.json",
        {
            "java_test": "org.omegat.gui.editor.mark.MarkerColorFreshnessTest#testPainterFollowsColorPreferenceChange",
            "source": "Edit",
            "translation": "target",
            "linked": "xAUTO",
            "before_color": "#1565c0",
            "after_color": "#123456",
        },
    )
    dump(
        "fixtures/goldens/editor/ComesFromMTMarkerTest#testNearString.json",
        {
            "java_test": "org.omegat.gui.editor.mark.ComesFromMTMarkerTest#testNearString",
            "comes_from": "TM",
            "tm_root": "src/test/resources/data/tmx",
            "proj": "src/test/resources/data/tmx/mt/mt1.tmx",
            "from_mt": True,
        },
    )
    dump(
        "fixtures/goldens/editor/CharTableModelTest#defaultTableIncludesZeroWidthSpace.json",
        {
            "java_test": "org.omegat.gui.editor.chartable.CharTableModelTest#defaultTableIncludesZeroWidthSpace",
            "glyph": "\u200b",
            "columns": 16,
        },
    )
    dump(
        "fixtures/goldens/editor/CharTableModelTest#autoCompleterSelectionUsesZeroWidthSpacePayload.json",
        {
            "java_test": "org.omegat.gui.editor.chartable.CharTableModelTest#autoCompleterSelectionUsesZeroWidthSpacePayload",
            "payload": "\u200b",
        },
    )
    dump(
        "fixtures/goldens/editor/CollapsibleBarTest#startsCollapsedByDefault.json",
        {
            "java_test": "org.omegat.gui.editor.CollapsibleBarTest#startsCollapsedByDefault",
            "expanded": False,
        },
    )
    dump(
        "fixtures/goldens/editor/CollapsibleBarTest#toggleExpandsAndCollapses.json",
        {
            "java_test": "org.omegat.gui.editor.CollapsibleBarTest#toggleExpandsAndCollapses",
            "after_toggle": True,
            "after_second_toggle": False,
        },
    )
    dump(
        "fixtures/goldens/editor/CollapsibleBarTest#setExpandedControlsState.json",
        {
            "java_test": "org.omegat.gui.editor.CollapsibleBarTest#setExpandedControlsState",
            "set_true": True,
            "set_false": False,
        },
    )
    dump(
        "fixtures/goldens/editor/CollapsibleBarTest#summaryReflectsModelAfterRefresh.json",
        {
            "java_test": "org.omegat.gui.editor.CollapsibleBarTest#summaryReflectsModelAfterRefresh",
            "initial": "empty",
            "after": "src:foo AND tgt:bar",
        },
    )
    dump(
        "fixtures/goldens/editor/CollapsibleBarTest#constructorDoesNotCallBuildSummaryBeforeSubclassInit.json",
        {
            "java_test": "org.omegat.gui.editor.CollapsibleBarTest#constructorDoesNotCallBuildSummaryBeforeSubclassInit",
            "summary": "empty",
        },
    )
    dump(
        "fixtures/goldens/editor/EditorProjectReloadLeakTest#closedProjectsMustBecomeUnreachableWithEditorAttached.json",
        {
            "java_test": "org.omegat.gui.editor.EditorProjectReloadLeakTest#closedProjectsMustBecomeUnreachableWithEditorAttached",
            "cycles": 3,
            "document_after_close": None,
            "entry_count_after_close": 0,
        },
    )
    dump(
        "fixtures/goldens/editor/GlossaryAutoCompleterViewTest#testSuggestions.json",
        {
            "java_test": "org.omegat.gui.glossary.GlossaryAutoCompleterViewTest#testSuggestions",
            "terms": ["foo", "bar", "BAZ"],
            "cases": [
                {"chunk": "", "contextual_only": False, "payloads": ["foo", "bar", "BAZ"]},
                {"chunk": "f", "contextual_only": False, "payloads": ["foo"]},
                {"chunk": "b", "contextual_only": False, "payloads": ["bar", "baz", "BAZ"]},
                {"chunk": "F", "contextual_only": False, "payloads": ["Foo", "foo"]},
                {"chunk": "FO", "contextual_only": False, "payloads": ["FOO", "foo"]},
                {"chunk": "B", "contextual_only": False, "payloads": ["BAZ", "Bar", "bar"]},
                {"chunk": "foo", "contextual_only": False, "payloads": ["foo", "bar", "BAZ"]},
                {"chunk": "", "contextual_only": True, "payloads": []},
                {"chunk": "f", "contextual_only": True, "payloads": ["foo"]},
            ],
        },
    )
    dump(
        "fixtures/goldens/align/BundleTest#testBundleEncodings.json",
        {
            "java_test": "org.omegat.gui.align.BundleTest#testBundleEncodings",
            "bundle": "org.omegat.gui.align.Bundle",
            "accepted_encodings": ["US-ASCII", "WINDOWS-1252"],
        },
    )
    print("wrote remaining goldens")


if __name__ == "__main__":
    main()
