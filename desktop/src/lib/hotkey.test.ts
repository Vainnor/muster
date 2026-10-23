import { describe, expect, it } from "vitest";

import { comboFromKeyPress, displayHotkey } from "./hotkey";

const press = (code: string, mods: Partial<Record<"ctrlKey" | "altKey" | "shiftKey" | "metaKey", boolean>> = {}) => ({
  code,
  ctrlKey: false,
  altKey: false,
  shiftKey: false,
  metaKey: false,
  ...mods,
});

describe("comboFromKeyPress", () => {
  it("names modifiers in canonical order and the key by its position", () => {
    expect(comboFromKeyPress(press("Space", { altKey: true }))).toBe("Alt+Space");
    expect(comboFromKeyPress(press("KeyK", { metaKey: true, shiftKey: true, ctrlKey: true }))).toBe("Ctrl+Shift+Super+K");
    expect(comboFromKeyPress(press("Digit1", { ctrlKey: true }))).toBe("Ctrl+1");
    expect(comboFromKeyPress(press("ArrowUp", { altKey: true }))).toBe("Alt+Up");
    expect(comboFromKeyPress(press("F12"))).toBe("F12");
  });

  it("waits while only modifiers are held", () => {
    for (const code of ["AltLeft", "ControlRight", "ShiftLeft", "MetaLeft", "OSRight"]) {
      expect(comboFromKeyPress(press(code, { altKey: true }))).toBeNull();
    }
  });

  it("passes unknown keys through for Rust to reject", () => {
    expect(comboFromKeyPress(press("IntlBackslash", { altKey: true }))).toBe("Alt+IntlBackslash");
  });
});

describe("displayHotkey", () => {
  it("uses macOS key names on macOS only", () => {
    expect(displayHotkey("Alt+Super+K", true)).toBe("Option+Cmd+K");
    expect(displayHotkey("Alt+Super+K", false)).toBe("Alt+Super+K");
  });
});
