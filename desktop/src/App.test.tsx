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
  Element.prototype.scrollIntoView ??= () => undefined;
});

afterEach(async () => {
  cleanup();
  // Let the unmounted App's async unlisten finish before the mock is removed.
  await new Promise((resolve) => setTimeout(resolve, 0));
  clearMocks();
});

const script: AppView = { id: "m:backup", name: "Backup", source: "manual", kind: "program", detail: '"/opt/backup.sh" "--all"' };
const group: GroupView = {
  id: "g:night",
  name: "Night",
  entries: [{ app: "m:backup", app_name: "Backup", args: null, cwd: null, delay_ms: 0 }],
  needs_confirmation: true,
};

function backend(runGroup: () => unknown) {
  const calls: string[] = [];
  mockIPC(
    (cmd) => {
      calls.push(cmd);
      switch (cmd) {
        case "app_info":
          return { name: "Muster", version: "0.1.0", os: "linux", arch: "x86_64", os_label: "Linux" };
        case "get_settings":
          return { theme: "system" };
        case "list_themes":
        case "theme_tokens":
          return [];
        case "list_apps":
          return [script];
        case "list_groups":
          return [group];
        case "run_group":
          return runGroup();
        case "launch_app":
          throw { code: "invalid_target", message: "That file or app does not exist." };
        default:
          return null;
      }
    },
    { shouldMockEvents: true },
  );
  return calls;
}

describe("App", () => {
  it("runs a group through one command; approval happens in Rust", async () => {
    const calls = backend(() => []);
    const { App } = await import("./App");
    render(<App />);
    const search = await screen.findByRole("searchbox");
    await screen.findByText("Night");
    fireEvent.keyDown(search, { key: "Enter" });
    await waitFor(() => expect(screen.getByRole("status").textContent).toBe("Starting Night."));
    expect(calls.filter((c) => c.includes("confirm"))).toEqual([]);
  });

  it("treats a cancelled approval as a choice, not an error", async () => {
    backend(() => {
      throw { code: "cancelled", message: "Nothing was started." };
    });
    const { App } = await import("./App");
    render(<App />);
    const search = await screen.findByRole("searchbox");
    await screen.findByText("Night");
    fireEvent.keyDown(search, { key: "Enter" });
    await waitFor(() => expect(screen.getByRole("status").textContent).toBe("Nothing was started."));
    expect(screen.getByRole("status").className).not.toContain("error");
  });

  it("shows the core's message when a launch fails", async () => {
    backend(() => []);
    const { App } = await import("./App");
    render(<App />);
    const search = await screen.findByRole("searchbox");
    fireEvent.change(search, { target: { value: "backup" } });
    await screen.findByText("Backup");
    fireEvent.keyDown(search, { key: "Enter" });
    await waitFor(() => expect(screen.getByRole("status").textContent).toBe("That file or app does not exist."));
  });
});
