/** Java `org.omegat.gui.editor.UnderlineFactory`. */
export type Underline = { style: "solid" | "wavy" | "dotted"; color: string };

export function underlineFor(kind: string): Underline {
  if (kind === "spell") return { style: "wavy", color: "#c00" };
  if (kind === "lt") return { style: "wavy", color: "#06c" };
  if (kind === "glossary") return { style: "dotted", color: "#080" };
  return { style: "solid", color: "#888" };
}
