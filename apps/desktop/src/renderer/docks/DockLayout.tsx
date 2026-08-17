import { useApp } from "../store/app";
import { CommentsDock } from "./CommentsDock";
import { DictionaryDock } from "./DictionaryDock";
import { EditorDock } from "./EditorDock";
import { GlossaryDock } from "./GlossaryDock";
import { MachineTranslationDock } from "./MachineTranslationDock";
import { MatchesDock } from "./MatchesDock";
import { MultipleTranslationsDock } from "./MultipleTranslationsDock";
import { NotesDock } from "./NotesDock";
import { SegmentPropertiesDock } from "./SegmentPropertiesDock";
import { Split } from "./Splitter";

export function WorkspaceDocks() {
  const layout = useApp((s) => s.layout);
  const main = (
    <Split axis="v" ratio={layout.left} field="left">
      <Split axis="h" ratio={layout.notes} field="notes">
        <NotesDock />
        <GlossaryDock />
      </Split>
      <Split axis="v" ratio={layout.editorStack} field="editorStack">
        <Split axis="h" ratio={layout.editorMain} field="editorMain">
          <EditorDock />
          <Split axis="v" ratio={layout.props} field="props">
            <SegmentPropertiesDock />
            <CommentsDock />
          </Split>
        </Split>
        <Split axis="h" ratio={layout.matches} field="matches">
          <MatchesDock />
          <MultipleTranslationsDock />
        </Split>
      </Split>
    </Split>
  );
  if (!layout.showDict && !layout.showMt) return <div className="workspace-docks">{main}</div>;
  return (
    <div className="workspace-docks">
      <Split axis="v" ratio={layout.east} field="east">
        {main}
        <Split axis="h" ratio={layout.dictMt} field="dictMt">
          {layout.showDict ? <DictionaryDock /> : <div />}
          {layout.showMt ? <MachineTranslationDock /> : <div />}
        </Split>
      </Split>
    </div>
  );
}
