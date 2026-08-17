/** Java `org.omegat.gui.editor.MarkerController`. */
import { AltTranslationsMarker } from "./mark/AltTranslationsMarker";
import { BidiMarkers } from "./mark/BidiMarkers";
import { calcMarkers } from "./mark/CalcMarkersThread";
import { ComesFromAutoTMMarker } from "./mark/ComesFromAutoTMMarker";
import { ComesFromMTMarker } from "./mark/ComesFromMTMarker";
import { FontFallbackMarker } from "./mark/FontFallbackMarker";
import type { IMarker, MarkerInput } from "./mark/IMarker";
import type { Mark } from "./mark/Mark";
import { NBSPMarker } from "./mark/NBSPMarker";
import { ProtectedPartsMarker } from "./mark/ProtectedPartsMarker";
import { RemoveTagMarker } from "./mark/RemoveTagMarker";
import { ReplaceMarker } from "./mark/ReplaceMarker";
import { WhitespaceMarker } from "./mark/WhitespaceMarker";

export class MarkerController {
  markers: IMarker[] = [
    new WhitespaceMarker(),
    new NBSPMarker(),
    new BidiMarkers(),
    new ProtectedPartsMarker(),
    new AltTranslationsMarker(),
    new ComesFromAutoTMMarker(),
    new ComesFromMTMarker(),
    new FontFallbackMarker(),
    new RemoveTagMarker(),
    new ReplaceMarker(),
  ];

  process(input: MarkerInput): Mark[] {
    return calcMarkers(this.markers, input);
  }
}
