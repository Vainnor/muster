// Turns a key press into the text `set_popup_hotkey` parses. Rust decides whether it is a
// hotkey Muster accepts; this only names the keys.

export interface KeyPress {
  code: string;
  ctrlKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
  metaKey: boolean;
}

const NAMES: Record<string, string> = {
  Space: "Space",
  Enter: "Enter",
  Tab: "Tab",
  Backspace: "Backspace",
  Delete: "Delete",
  Home: "Home",
  End: "End",
  PageUp: "PageUp",
  PageDown: "PageDown",
  ArrowUp: "Up",
  ArrowDown: "Down",
  ArrowLeft: "Left",
  ArrowRight: "Right",
  Backquote: "Backquote",
  Minus: "Minus",
  Equal: "Equal",
  BracketLeft: "BracketLeft",
  BracketRight: "BracketRight",
  Backslash: "Backslash",
  Semicolon: "Semicolon",
  Quote: "Quote",
  Comma: "Comma",
  Period: "Period",
  Slash: "Slash",
  Escape: "Escape",
};

const MODIFIER_CODES = /^(Control|Alt|Shift|Meta|OS)(Left|Right)?$/;

/** The key's name, from its physical position (`code`), so Option+Space on macOS still reads as Space. */
function keyName(code: string): string {
  const letter = /^Key([A-Z])$/.exec(code)?.[1];
  if (letter !== undefined) return letter;
  const digit = /^Digit([0-9])$/.exec(code)?.[1];
  if (digit !== undefined) return digit;
  if (/^F([1-9]|1[0-9]|2[0-4])$/.test(code)) return code;
  return NAMES[code] ?? code;
}

/** `null` while only modifiers are held. */
export function comboFromKeyPress(e: KeyPress): string | null {
  if (MODIFIER_CODES.test(e.code)) return null;
  const parts: string[] = [];
  if (e.ctrlKey) parts.push("Ctrl");
  if (e.altKey) parts.push("Alt");
  if (e.shiftKey) parts.push("Shift");
  if (e.metaKey) parts.push("Super");
  parts.push(keyName(e.code));
  return parts.join("+");
}

/** The canonical form with the names macOS prints on its keys. */
export function displayHotkey(hotkey: string, mac: boolean): string {
  if (!mac) return hotkey;
  return hotkey
    .split("+")
    .map((p) => (p === "Alt" ? "Option" : p === "Super" ? "Cmd" : p))
    .join("+");
}
