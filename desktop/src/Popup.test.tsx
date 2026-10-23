// @vitest-environment jsdom
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeAll, describe, expect, it } from "vitest";

import type { AppView } from "./lib/types";

// useTheme reads prefers-color-scheme when its module loads, so Popup is imported after this.
beforeAll(() => {
  window.matchMedia = ((query: string) => ({
    matches: false,
    media: query,
    addEventListener: () => undefined,
    removeEventListener: () => undefined,
  })) as unknown as typeof window.matchMedia;
  Element.prototype.scrollIntoView ??= () => undefined;
});

afterEach(() => {
  cleanup();
  clearMocks();
});

const app: AppView = { id: "m:editor", name: "Editor", source: "manual", kind: "program", detail: '"/usr/bin/editor"' };

async function setup(launch: () => unknown, delays = { list: 20, theme: 60 }) {
  const { Popup } = await import("./Popup");
  const calls: string[] = [];
  // The theme and list answer a little late, as they can in the app, and each answer is
  // recorded when it arrives so the test can tell "asked" from "loaded".
  const late = <T,>(name: string, value: T, ms: number) =>
    new Promise<T>((resolve) =>
      setTimeout(() => {
        calls.push(`${name}:done`);
        resolve(value);
      }, ms),
    );
  mockIPC((cmd) => {
    calls.push(cmd);
    if (cmd === "get_settings") return { theme: "system", popup_hotkey: "Alt+Space", popup_hotkey_active: true, wayland: false };
    if (cmd === "list_apps") return late(cmd, [app], delays.list);
    if (cmd === "list_groups") return [];
    if (cmd === "theme_tokens") return late(cmd, [], delays.theme);
    if (cmd === "launch_app") return launch();
    return null;
  });
  render(<Popup />);
  return calls;
}

describe("Popup", () => {
  it.each([
    { list: 20, theme: 60 },
    { list: 60, theme: 20 },
  ])("asks to be shown only after the theme and the list have loaded, and only once ($list ms list, $theme ms theme)", async (delays) => {
    const calls = await setup(() => null, delays);
    await screen.findByText("Editor");
    await waitFor(() => expect(calls).toContain("popup_ready"));
    const ready = calls.indexOf("popup_ready");
    expect(ready).toBeGreaterThan(calls.indexOf("theme_tokens:done"));
    expect(ready).toBeGreaterThan(calls.indexOf("list_apps:done"));
    expect(calls.indexOf("theme_tokens:done")).toBeGreaterThan(-1);
    fireEvent.change(screen.getByRole("searchbox"), { target: { value: "ed" } });
    await new Promise((resolve) => setTimeout(resolve, 20));
    expect(calls.filter((c) => c === "popup_ready")).toHaveLength(1);
  });

  it("closes itself once the app starts", async () => {
    const calls = await setup(() => null);
    fireEvent.click(await screen.findByText("Editor"));
    await waitFor(() => expect(calls).toContain("close_window"));
  });

  it("stays open with a note when the user declines the approval", async () => {
    const calls = await setup(() => {
      throw { code: "cancelled", message: "Nothing was started." };
    });
    fireEvent.click(await screen.findByText("Editor"));
    const note = await screen.findByRole("status");
    expect(note.textContent).toBe("Nothing was started.");
    expect(note.className).toBe("status");
    expect(calls).not.toContain("close_window");
  });

  it("Escape on an empty search closes it", async () => {
    const calls = await setup(() => null);
    await screen.findByText("Editor");
    fireEvent.keyDown(screen.getByRole("searchbox"), { key: "Escape" });
    await waitFor(() => expect(calls).toContain("close_window"));
  });
});
