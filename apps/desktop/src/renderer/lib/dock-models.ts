export type GlossaryDisplayEntry = {
  source: string;
  target: string;
  comment?: string;
};

export function decodeGlossaryComment(comment = ""): string {
  try {
    return decodeURI(comment);
  } catch {
    return comment;
  }
}

/** Plain-text rendering used by the glossary dock and accessibility labels. */
export function renderGlossaryText(entries: GlossaryDisplayEntry[]): string {
  return entries
    .map((entry) => {
      const comment = decodeGlossaryComment(entry.comment);
      return `${entry.source} = ${entry.target}${comment ? `\n1. ${comment}` : ""}`;
    })
    .join("");
}

/** Java `NotesTextArea.getNoteText`: empty text represents no note. */
export function noteText(value: string): string | null {
  return value === "" ? null : value;
}

export class NotesDocument {
  private value: string | null = null;

  set(text: string): void {
    this.value = noteText(text);
  }

  clear(): void {
    this.value = null;
  }

  get(): string | null {
    return this.value;
  }
}
