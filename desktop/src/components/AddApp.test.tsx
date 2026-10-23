// @vitest-environment jsdom
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { Picked } from "../lib/types";
import { AddApp } from "./AddApp";

afterEach(() => {
  cleanup();
  clearMocks();
});

function setup(pick: () => Picked | null, isMac = false) {
  const asked: unknown[] = [];
  mockIPC((cmd, args) => {
    if (cmd !== "pick_path") return null;
    asked.push(args);
    return pick();
  });
  const onAdd = vi.fn(() => Promise.resolve(true));
  const onError = vi.fn();
  render(<AddApp isMac={isMac} onAdd={onAdd} onError={onError} onCancel={() => {}} />);
  return { asked, onAdd, onError };
}

const field = (name: string) => screen.getByRole("textbox", { name }) as HTMLInputElement;

describe("AddApp", () => {
  it("fills the path and a name from the picked program", async () => {
    const { asked, onAdd } = setup(() => ({ kind: "program", path: "/opt/tools/build.sh" }));
    fireEvent.click(screen.getByRole("button", { name: "Browse for a program" }));
    await waitFor(() => expect(field("Full path").value).toBe("/opt/tools/build.sh"));
    expect(asked).toEqual([{ kind: "program" }]);
    expect(field("Name").value).toBe("build");

    fireEvent.click(screen.getByRole("button", { name: "Add" }));
    expect(onAdd).toHaveBeenCalledWith("build", { kind: "program", path: "/opt/tools/build.sh", args: [], cwd: null });
  });

  it("switches to an app bundle when the picked program is an .app", async () => {
    const { onAdd } = setup(() => ({ kind: "app_bundle", path: "/Applications/Editor.app" }), true);
    fireEvent.click(screen.getByRole("button", { name: "Browse for a program" }));
    await waitFor(() => expect(screen.getByRole("combobox", { name: "Kind" })).toHaveProperty("value", "app_bundle"));
    fireEvent.click(screen.getByRole("button", { name: "Add" }));
    expect(onAdd).toHaveBeenCalledWith("Editor", { kind: "app_bundle", path: "/Applications/Editor.app" });
  });

  it("keeps a name the user already typed", async () => {
    setup(() => ({ kind: "program", path: "/opt/tools/build.sh" }));
    fireEvent.change(field("Name"), { target: { value: "Nightly build" } });
    fireEvent.click(screen.getByRole("button", { name: "Browse for a program" }));
    await waitFor(() => expect(field("Full path").value).toBe("/opt/tools/build.sh"));
    expect(field("Name").value).toBe("Nightly build");
  });

  it("fills the working folder from the folder picker", async () => {
    const { asked } = setup(() => ({ kind: "folder", path: "/opt/tools" }));
    fireEvent.click(screen.getByRole("button", { name: "Browse for a working folder" }));
    await waitFor(() => expect(field("Working folder (optional)").value).toBe("/opt/tools"));
    expect(asked).toEqual([{ kind: "folder" }]);
    expect(field("Full path").value).toBe("");
  });

  it("changes nothing when the picker is cancelled", async () => {
    const { asked } = setup(() => null);
    fireEvent.click(screen.getByRole("button", { name: "Browse for a program" }));
    await waitFor(() => expect(asked).toHaveLength(1));
    expect(field("Full path").value).toBe("");
    expect(field("Name").value).toBe("");
  });

  it("reports a picker that is already open", async () => {
    const { onError } = setup(() => {
      throw { code: "busy", message: "Answer the open Muster dialog first." };
    });
    fireEvent.click(screen.getByRole("button", { name: "Browse for a program" }));
    await waitFor(() => expect(onError).toHaveBeenCalledWith({ code: "busy", message: "Answer the open Muster dialog first." }));
  });

  it("has no picker for a link", () => {
    setup(() => null);
    fireEvent.change(screen.getByRole("combobox", { name: "Kind" }), { target: { value: "url" } });
    expect(screen.queryByRole("button", { name: /Browse/ })).toBeNull();
  });
});
