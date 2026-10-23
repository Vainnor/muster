// @vitest-environment jsdom
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { UpdateView } from "../lib/types";
import { UpdateSetting } from "./UpdateSetting";

afterEach(() => {
  cleanup();
  clearMocks();
});

function setup(builtIn: boolean, update: UpdateView | null, install: () => unknown = () => null) {
  const calls: Array<[string, unknown]> = [];
  mockIPC((cmd, args) => {
    calls.push([cmd, args]);
    switch (cmd) {
      case "get_settings":
        return { theme: "system", popup_hotkey: null, popup_hotkey_active: false, wayland: false, start_on_login: false, updates: { built_in: builtIn, check_on_start: false } };
      case "check_for_update":
        return update;
      case "install_update":
        return install();
      default:
        return null;
    }
  });
  const onError = vi.fn();
  render(<UpdateSetting onError={onError} />);
  return { calls, onError };
}

const found: UpdateView = { version: "1.1.0", notes: "Fixes.", install: "in_app" };

describe("UpdateSetting", () => {
  it("does not check until asked", async () => {
    const { calls } = setup(true, null);
    await screen.findByRole("button", { name: "Check for updates" });
    expect(calls.map(([c]) => c)).toEqual(["get_settings"]);
  });

  it("says when Muster is up to date", async () => {
    setup(true, null);
    fireEvent.click(await screen.findByRole("button", { name: "Check for updates" }));
    await screen.findByText("Muster is up to date.");
  });

  it("offers to install an update this copy can install, and treats no as a choice", async () => {
    const { calls, onError } = setup(true, found, () => {
      throw { code: "cancelled", message: "Nothing was started." };
    });
    fireEvent.click(await screen.findByRole("button", { name: "Check for updates" }));
    await screen.findByText("Muster 1.1.0 is available.");
    fireEvent.click(screen.getByRole("button", { name: "Install and restart" }));
    await waitFor(() => expect(calls.map(([c]) => c)).toContain("install_update"));
    expect(onError).not.toHaveBeenCalled();
  });

  it("points package installs at the package manager, with no install button", async () => {
    setup(true, { ...found, install: "package_manager" });
    fireEvent.click(await screen.findByRole("button", { name: "Check for updates" }));
    await screen.findByText("Update Muster with your package manager.");
    expect(screen.queryByRole("button", { name: "Install and restart" })).toBeNull();
    expect(screen.getByRole("button", { name: "Open the release page" })).toBeTruthy();
  });

  it("saves the check at startup", async () => {
    const { calls } = setup(true, null);
    fireEvent.click(await screen.findByRole("checkbox", { name: "Check for updates when Muster starts" }));
    await waitFor(() => expect(calls).toContainEqual(["set_check_updates_on_start", { on: true }]));
  });

  it("says so in builds without the check", async () => {
    setup(false, null);
    await screen.findByText("This copy of Muster was built without the update check.");
    expect(screen.queryByRole("button", { name: "Check for updates" })).toBeNull();
  });
});
