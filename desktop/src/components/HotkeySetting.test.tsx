// @vitest-environment jsdom
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { Settings } from "../lib/types";
import { HotkeySetting } from "./HotkeySetting";

afterEach(() => {
  cleanup();
  clearMocks();
});

function setup(initial: Partial<Settings> = {}, onOuterKeyDown?: () => void) {
  let settings: Settings = { theme: "system", popup_hotkey: "Alt+Space", popup_hotkey_active: true, wayland: false, start_on_login: false, updates: { built_in: false, check_on_start: false }, ...initial };
  const saved: (string | null)[] = [];
  mockIPC((cmd, args) => {
    if (cmd === "get_settings") return settings;
    if (cmd === "set_popup_hotkey") {
      const hotkey = (args as { hotkey: string | null }).hotkey;
      saved.push(hotkey);
      settings = { ...settings, popup_hotkey: hotkey };
      return hotkey;
    }
    return null;
  });
  const onError = vi.fn();
  // Settings closes on Escape through a React onKeyDown around this section.
  render(
    <div onKeyDown={onOuterKeyDown}>
      <HotkeySetting onError={onError} />
    </div>,
  );
  return { saved, onError };
}

describe("HotkeySetting", () => {
  it("records a new hotkey from the next key press", async () => {
    const { saved } = setup();
    const change = await screen.findByRole("button", { name: "Change…" });
    fireEvent.click(change);
    expect(screen.getByText("Press the new hotkey…")).toBeTruthy();
    fireEvent.keyDown(change, { code: "ControlLeft", ctrlKey: true });
    expect(saved).toEqual([]);
    fireEvent.keyDown(change, { code: "KeyK", ctrlKey: true, shiftKey: true });
    await waitFor(() => expect(saved).toEqual(["Ctrl+Shift+K"]));
    await screen.findByText("Ctrl+Shift+K");
  });

  it("hears the key press when focus is not on the button, as after a click in WebKit", async () => {
    const { saved } = setup({ popup_hotkey: null, popup_hotkey_active: false });
    fireEvent.click(await screen.findByRole("button", { name: "Change…" }));
    fireEvent.keyDown(document.body, { code: "Space", altKey: true });
    await waitFor(() => expect(saved).toEqual(["Alt+Space"]));
  });

  it("Escape cancels recording without saving or closing settings", async () => {
    const outer = vi.fn();
    const { saved } = setup({}, outer);
    const change = await screen.findByRole("button", { name: "Change…" });
    fireEvent.click(change);
    fireEvent.keyDown(change, { code: "Escape" });
    expect(saved).toEqual([]);
    expect(outer).not.toHaveBeenCalled();
    expect(screen.getByText("Alt+Space")).toBeTruthy();
  });

  it("turns the hotkey off", async () => {
    const { saved } = setup();
    fireEvent.click(await screen.findByRole("button", { name: "Turn off" }));
    await waitFor(() => expect(saved).toEqual([null]));
    await screen.findByText("Off");
  });

  it("warns when the OS refused the saved hotkey", async () => {
    setup({ popup_hotkey_active: false });
    expect((await screen.findByRole("alert")).textContent).toContain("could not claim Alt+Space");
  });

  it("on Wayland, points to the compositor instead of recording", async () => {
    setup({ wayland: true });
    await screen.findByText(/Global hotkeys do not work on Wayland/);
    expect(screen.queryByRole("button", { name: "Change…" })).toBeNull();
  });
});
