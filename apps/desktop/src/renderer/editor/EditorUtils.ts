/** Java `org.omegat.gui.editor.EditorUtils`. */
export function removeDirectionChars(s: string): string {
  return s.replace(/[\u200e\u200f\u202a-\u202e\u2066-\u2069]/g, "");
}

export function changeCase(s: string, mode: "upper" | "lower" | "title" | "sentence"): string {
  if (mode === "upper") return s.toUpperCase();
  if (mode === "lower") return s.toLowerCase();
  if (mode === "title") return s.replace(/\S+/g, (w) => w.charAt(0).toUpperCase() + w.slice(1).toLowerCase());
  return s.charAt(0).toUpperCase() + s.slice(1);
}
