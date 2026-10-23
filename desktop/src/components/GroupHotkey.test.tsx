// @vitest-environment jsdom
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { GroupView } from "../lib/types";
import { GroupHotkey } from "./GroupHotkey";

afterEach(() => {
  cleanup();
  clearMocks();
});

const group: GroupView = { id: "g:work", name: "Work", entries: [], needs_confirmation: false, hotkey: null, hotkey_active: false };

function setup(wayland = false, answer: (hotkey: string | null) => unknown = (h) => h) {
  const saved: { id: string; hotkey: string | null }[] = [];
  mockIPC((cmd, args) => {
    if (cmd === "get_settings") return { theme: "system", popup_hotkey: "Alt+Space", popup_hotkey_active: true, wayland, start_on_login: false };
    if (cmd === "set_group_hotkey") {
      const a = args as { id: string; hotkey: string | null };
      saved.push(a);
      return answer(a.hotkey);
    }
    return null;
  });
  const onChanged = vi.fn().mockResolvedValue(undefined);
  const onError = vi.fn();
  render(<GroupHotkey group={group} onChanged={onChanged} onError={onError} />);
  return { saved, onChanged, onError };
}

describe("GroupHotkey", () => {
  it("records a hotkey for this group and reloads the list", async () => {
    const { saved, onChanged } = setup();
    fireEvent.click(await screen.findByRole("button", { name: "Change…" }));
    fireEvent.keyDown(document.body, { code: "Digit1", ctrlKey: true, altKey: true });
    await waitFor(() => expect(saved).toEqual([{ id: "g:work", hotkey: "Ctrl+Alt+1" }]));
    await screen.findByText("Ctrl+Alt+1");
    expect(onChanged).toHaveBeenCalled();
  });

  it("keeps the old hotkey when Rust refuses the new one", async () => {
    const { onError } = setup(false, () => {
      throw { code: "hotkey_in_use", message: "The popup or another group already uses that hotkey. Pick another." };
    });
    fireEvent.click(await screen.findByRole("button", { name: "Change…" }));
    fireEvent.keyDown(document.body, { code: "Space", altKey: true });
    await waitFor(() => expect(onError).toHaveBeenCalled());
    expect(screen.getByText("Off")).toBeTruthy();
  });

  it("on Wayland, gives the command to bind instead", async () => {
    setup(true);
    await screen.findByText("muster --run-group g:work");
    expect(screen.queryByRole("button", { name: "Change…" })).toBeNull();
  });
});
