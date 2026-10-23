// @vitest-environment jsdom
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { StartupSetting } from "./StartupSetting";

afterEach(() => {
  cleanup();
  clearMocks();
});

function setup(startOnLogin: boolean | null, answer: (enabled: boolean) => unknown = (enabled) => enabled) {
  const asked: boolean[] = [];
  mockIPC((cmd, args) => {
    if (cmd === "get_settings") return { theme: "system", popup_hotkey: null, popup_hotkey_active: false, wayland: false, start_on_login: startOnLogin };
    if (cmd === "set_start_on_login") {
      const enabled = (args as { enabled: boolean }).enabled;
      asked.push(enabled);
      return answer(enabled);
    }
    return null;
  });
  const onError = vi.fn();
  render(<StartupSetting onError={onError} />);
  return { asked, onError };
}

describe("StartupSetting", () => {
  it("turns start on login on and shows what the OS reports", async () => {
    const { asked } = setup(false);
    const box = await screen.findByRole<HTMLInputElement>("checkbox", { name: "Start Muster when I log in" });
    expect(box.checked).toBe(false);
    fireEvent.click(box);
    await waitFor(() => expect(asked).toEqual([true]));
    await waitFor(() => expect(box.checked).toBe(true));
  });

  it("shows the OS's answer, not the click, when the two differ", async () => {
    setup(false, () => false);
    const box = await screen.findByRole<HTMLInputElement>("checkbox");
    fireEvent.click(box);
    await waitFor(() => expect(box.disabled).toBe(false));
    expect(box.checked).toBe(false);
  });

  it("reports a failure and leaves the box as it was", async () => {
    const { onError } = setup(true, () => {
      throw { code: "login_item", message: "Muster could not change whether it starts when you log in." };
    });
    const box = await screen.findByRole<HTMLInputElement>("checkbox");
    fireEvent.click(box);
    await waitFor(() => expect(onError).toHaveBeenCalled());
    expect(box.checked).toBe(true);
  });

  it("says so when the OS state could not be read", async () => {
    setup(null);
    await screen.findByText("Muster could not check whether it starts when you log in.");
    expect(screen.queryByRole("checkbox")).toBeNull();
  });
});
