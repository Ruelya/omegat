/** Java `org.omegat.gui.editor.autotext.Autotext`. */
export type AutotextEntry = { shortcut: string; full: string; comment: string };

export function parseAutotext(raw: string): AutotextEntry[] {
  return raw
    .split(/\n+/)
    .map((ln) => ln.trim())
    .filter(Boolean)
    .map((ln) => {
      const [shortcut = "", full = "", comment = ""] = ln.split("=");
      return { shortcut, full, comment };
    });
}

export function matchAutotext(entries: AutotextEntry[], chunk: string): AutotextEntry[] {
  const p = chunk.toLowerCase();
  return entries.filter((e) => e.shortcut.toLowerCase().startsWith(p) || e.full.toLowerCase().includes(p));
}
