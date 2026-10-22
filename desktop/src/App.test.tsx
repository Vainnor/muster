// @vitest-environment jsdom
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeAll, describe, expect, it } from "vitest";

import type { AppView, GroupView } from "./lib/types";

beforeAll(() => {
  window.matchMedia = ((query: string) => ({
    matches: false,
    media: query,
    addEventListener: () => undefined,
    removeEventListener: () => undefined,
  })) as unknown as typeof window.matchMedia;
  // jsdom may lack the dialog methods; the component only needs them to exist.
  HTMLDialogElement.prototype.showModal ??= function (this: HTMLDialogElement) {
    this.open = true;
  };
  Element.prototype.scrollIntoView ??= () => undefined;
  HTMLDialogElement.prototype.close ??= function (this: HTMLDialogElement) {
    this.open = false;
  };
});

afterEach(async () => {
  cleanup();
  // Let the unmounted App's async unlisten finish before the mock is removed.
  await new Promise((resolve) => setTimeout(resolve, 0));
  clearMocks();
});

const script: AppView = { id: "m:backup", name: "Backup", source: "manual", kind: "program", detail: "/opt/backup.sh --all" };
const group: GroupView = {
  id: "g:night",
  name: "Night",
  entries: [{ app: "m:backup", app_name: "Backup", args: null, cwd: null, delay_ms: 0 }],
  needs_confirmation: true,
};

function backend() {
  const calls: Array<[string, unknown]> = [];
  let confirmed = false;
  mockIPC((cmd, args) => {
    calls.push([cmd, args]);
    switch (cmd) {
      case "app_info":
        return { name: "Muster", version: "0.1.0", os: "linux", arch: "x86_64", os_label: "Linux" };
      case "get_settings":
        return { theme: "system" };
      case "list_themes":
      case "theme_tokens":
        return [];
      case "startup_notice":
        return null;
      case "list_apps":
        return [script];
      case "list_groups":
        return [{ ...group, needs_confirmation: !confirmed }];
      case "group_confirmation":
        return [script];
      case "confirm_group":
        confirmed = true;
        return null;
      case "run_group":
        if (!confirmed) throw { code: "needs_confirmation", message: "Confirm first." };
        return [];
      default:
        return null;
    }
  }, { shouldMockEvents: true });
  return calls;
}

describe("App", () => {
  it("asks before the first run of a group with a manual entry, then runs it", async () => {
    const calls = backend();
    const { App } = await import("./App");
    render(<App />);
    const search = await screen.findByRole("searchbox");
    await screen.findByText("Night");
    fireEvent.keyDown(search, { key: "Enter" });

    expect(await screen.findByText("/opt/backup.sh --all")).toBeTruthy();
    fireEvent.click(screen.getByText("Run group"));

    await waitFor(() => expect(screen.getByRole("status").textContent).toBe("Starting Night."));
    const order = calls.map(([c]) => c).filter((c) => ["run_group", "group_confirmation", "confirm_group"].includes(c));
    expect(order).toEqual(["run_group", "group_confirmation", "confirm_group", "run_group"]);
  });

  it("shows the core's message when a launch fails", async () => {
    backend();
    mockIPC((cmd) => {
      if (cmd === "launch_app") throw { code: "invalid_target", message: "That file or app does not exist." };
      if (cmd === "list_apps") return [script];
      if (cmd === "list_groups" || cmd === "list_themes" || cmd === "theme_tokens") return [];
      if (cmd === "get_settings") return { theme: "system" };
      return null;
    }, { shouldMockEvents: true });
    const { App } = await import("./App");
    render(<App />);
    const search = await screen.findByRole("searchbox");
    await screen.findByText("Backup");
    fireEvent.keyDown(search, { key: "Enter" });
    await waitFor(() => expect(screen.getByRole("status").textContent).toBe("That file or app does not exist."));
  });
});
