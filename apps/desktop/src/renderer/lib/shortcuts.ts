export type ShortcutProperties = Record<string, string>;

export interface ShortcutMenuItem {
  action?: string;
  accelerator?: string | null;
  children?: ShortcutMenuItem[];
}

export class IllegalArgumentException extends Error {
  override name = "IllegalArgumentException";
}

/** Java `Properties.load` subset used by OmegaT shortcut property files. */
export function parseShortcutProperties(text: string): ShortcutProperties {
  const result: ShortcutProperties = {};
  for (const physical of text.replace(/\r\n?/g, "\n").split("\n")) {
    const line = physical.trim();
    if (!line || line.startsWith("#") || line.startsWith("!")) continue;
    const match = /^([^:=\s]+)\s*(?:[:=]|\s)\s*(.*)$/.exec(line);
    if (!match) continue;
    result[unescapeProperty(match[1]!)] = unescapeProperty(match[2]!);
  }
  return result;
}

export function mergeShortcutProperties(...sources: string[]): ShortcutProperties {
  return Object.assign({}, ...sources.map(parseShortcutProperties));
}

/** Match Swing `KeyStroke.toString()` for shortcut golden comparison. */
export function javaKeyStroke(properties: ShortcutProperties, action: string): string | null {
  if (!(action in properties)) {
    throw new IllegalArgumentException(`Keyboard shortcut not defined. Key=${action}`);
  }
  const shortcut = properties[action]!.trim();
  if (!shortcut) return null;
  const parts = shortcut.split(/\s+/);
  const key = parts.pop()!;
  const modifiers = parts.map((part) => part.toLowerCase()).join(" ");
  return modifiers ? `${modifiers} pressed ${key.toUpperCase()}` : `pressed ${key.toUpperCase()}`;
}

/** Apply user overrides recursively, preserving an unknown action's accelerator. */
export function bindMenuShortcuts(items: ShortcutMenuItem[], properties: ShortcutProperties): ShortcutMenuItem[] {
  return items.map((item) => {
    if (item.children) {
      return { ...item, children: bindMenuShortcuts(item.children, properties) };
    }
    if (!item.action || !(item.action in properties)) return { ...item };
    return { ...item, accelerator: javaKeyStroke(properties, item.action) };
  });
}

/** Java `bindKeyStrokes(InputMap, keys...)`, represented as accelerator → action. */
export function bindInputShortcuts(
  input: ShortcutProperties,
  properties: ShortcutProperties,
  actions: string[],
): ShortcutProperties {
  const result = { ...input };
  for (const action of actions) {
    if (!(action in properties)) continue;
    for (const [accelerator, boundAction] of Object.entries(result)) {
      if (boundAction === action) delete result[accelerator];
    }
    const stroke = javaKeyStroke(properties, action);
    if (stroke) result[stroke] = action;
  }
  return result;
}

/** Accept Java or Electron notation at the native-menu boundary. */
export function normalizeAccelerator(accelerator: string): string {
  if (accelerator.includes("+")) return accelerator;
  const parts = accelerator.trim().split(/\s+/);
  const key = parts.pop() ?? "";
  const modifiers = parts.map((part) => {
    switch (part.toLowerCase()) {
      case "ctrl":
      case "control":
        return "CmdOrCtrl";
      case "alt":
        return "Alt";
      case "shift":
        return "Shift";
      case "meta":
        return "Command";
      default:
        return part;
    }
  });
  return [...modifiers, key.toUpperCase()].filter(Boolean).join("+");
}

function unescapeProperty(value: string): string {
  return value.replace(/\\([\\:=#! ])/g, "$1").replace(/\\t/g, "\t").replace(/\\n/g, "\n");
}
