/** Compose Java `gui.editor.mark.*` intervals. */
import { altTranslationsMarker } from "./AltTranslationsMarker";
import { bidiMarkers } from "./BidiMarkers";
import { comesFromAutoTMMarker } from "./ComesFromAutoTMMarker";
import { comesFromMTMarker } from "./ComesFromMTMarker";
import { fontFallbackMarker } from "./FontFallbackMarker";
import { nbspMarker } from "./NBSPMarker";
import { protectedPartsMarker } from "./ProtectedPartsMarker";
import { removeTagMarker } from "./RemoveTagMarker";
import { replaceMarker } from "./ReplaceMarker";
import { whitespaceMarker } from "./WhitespaceMarker";
import type { Mark } from "./Mark";

export type MarkInterval = { start: number; end: number; kind: string; text: string };

function asInterval(m: Mark, kind: string): MarkInterval {
  return { start: m.startOffset, end: m.endOffset, kind, text: m.toolTipText ?? m.painter };
}

export function allMarkers(input: {
  text: string;
  source?: string;
  isAlt?: boolean;
  fromAuto?: boolean;
  fromMt?: boolean;
}): MarkInterval[] {
  const t = input.text;
  return [
    ...whitespaceMarker(t).map((m) => asInterval(m, "whitespace")),
    ...nbspMarker(t).map((m) => asInterval(m, "nbsp")),
    ...bidiMarkers(t).map((m) => asInterval(m, "bidi")),
    ...protectedPartsMarker(t).map((m) => asInterval(m, "protected")),
    ...altTranslationsMarker(!!input.isAlt, t.length).map((m) => asInterval(m, "alt")),
    ...comesFromAutoTMMarker(!!input.fromAuto, t.length).map((m) => asInterval(m, "auto-tm")),
    ...comesFromMTMarker(!!input.fromMt, t.length).map((m) => asInterval(m, "mt")),
    ...fontFallbackMarker(t).map((m) => asInterval(m, "font-fallback")),
    ...removeTagMarker(input.source ?? "", t).map((m) => asInterval(m, "remove-tag")),
  ];
}

export {
  whitespaceMarker,
  nbspMarker,
  bidiMarkers,
  protectedPartsMarker,
  altTranslationsMarker,
  comesFromAutoTMMarker,
  comesFromMTMarker,
  fontFallbackMarker,
  removeTagMarker,
  replaceMarker,
};
