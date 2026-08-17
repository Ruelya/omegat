/** Java `org.omegat.gui.editor.history.HistoryPredictor`. */
import { AutoCompleterListView } from "../autocompleter/AutoCompleterListView";
import { item, type AutoCompleterItem } from "../autocompleter/AutoCompleterItem";
import { predictNext, trainPredictor, type PredictorModel } from "./WordPredictor";

export class HistoryPredictor extends AutoCompleterListView {
  model: PredictorModel = new Map();
  constructor() {
    super("history-predict");
  }
  train(translations: string[]) {
    this.model = trainPredictor(translations);
  }
  computeListData(wordChunk: string): AutoCompleterItem[] {
    return predictNext(this.model, wordChunk).map((p) => item(p.word, "history-predict", [`${p.pct}%`]));
  }
}
