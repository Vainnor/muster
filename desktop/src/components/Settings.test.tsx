// @vitest-environment jsdom
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { ThemePreview } from "../lib/types";
import { Settings } from "./Settings";

const preview: ThemePreview = {
  theme: { id: "mine", name: "Mine", mode: "dark", custom: true },
  tokens: [{ name: "--mu-bg", value: "#000000" }],
  warnings: [{ foreground: "muted_text", background: "surface", ratio: 3.1, required: 4.5 }],
};

afterEach(() => {
  cleanup();
  clearMocks();
});

function setup() {
  const calls: string[] = [];
  mockIPC((cmd) => {
    // The hotkey section loads its own settings; these tests are about theme commands.
    if (cmd === "get_settings") return { theme: "system", popup_hotkey: null, popup_hotkey_active: false, wayland: false, start_on_login: false };
    calls.push(cmd);
    if (cmd === "preview_theme" || cmd === "import_theme") return preview;
    return null;
  });
  const props = {
    onTheme: vi.fn(),
    onPreview: vi.fn(),
    onThemesChanged: vi.fn().mockResolvedValue(undefined),
    onError: vi.fn(),
    onClose: vi.fn(),
  };
  const { container } = render(<Settings themes={[]} theme="system" {...props} />);
  const input = container.querySelector<HTMLInputElement>('input[type="file"]')!;
  return { calls, props, input };
}

const file = (text: string) => new File([text], "mine.json", { type: "application/json" });

describe("Settings theme import", () => {
  it("previews with warnings, then imports and selects the theme", async () => {
    const { calls, props, input } = setup();
    fireEvent.change(input, { target: { files: [file("{}")] } });

    expect(await screen.findByText("Secondary text on panels: 3.1:1, needs 4.5:1")).toBeTruthy();
    expect(props.onPreview).toHaveBeenCalledWith(preview.tokens);
    expect(calls).toEqual(["preview_theme"]);

    fireEvent.click(screen.getByText("Import and use"));
    await waitFor(() => expect(props.onTheme).toHaveBeenCalledWith("mine"));
    expect(calls).toEqual(["preview_theme", "import_theme"]);
    expect(props.onPreview).toHaveBeenLastCalledWith(null);
  });

  it("cancel restores the chosen theme and saves nothing", async () => {
    const { calls, props, input } = setup();
    fireEvent.change(input, { target: { files: [file("{}")] } });
    fireEvent.click(await screen.findByText("Cancel"));
    expect(props.onPreview).toHaveBeenLastCalledWith(null);
    expect(calls).toEqual(["preview_theme"]);
  });

  it("refuses files over 16 KiB without reading them", async () => {
    const { calls, props, input } = setup();
    fireEvent.change(input, { target: { files: [file("x".repeat(16 * 1024 + 1))] } });
    await waitFor(() => expect(props.onError).toHaveBeenCalled());
    expect(calls).toEqual([]);
  });
});
