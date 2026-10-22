// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";

import type { AppView, GroupView } from "../lib/types";
import { Launcher } from "./Launcher";

const app = (id: string, name: string): AppView => ({ id, name, source: "detected", kind: "program", detail: "" });
const apps = [app("d:mail", "Mail"), app("d:code", "Code"), app("d:notes", "Code Notes")];
const groups: GroupView[] = [{ id: "g:work", name: "Work", entries: [], needs_confirmation: false }];

function setup() {
  const props = { onLaunchApp: vi.fn(), onRunGroup: vi.fn(), onEscapeEmpty: vi.fn() };
  render(<Launcher apps={apps} groups={groups} {...props} />);
  return { ...props, search: screen.getByRole("searchbox") };
}

beforeAll(() => {
  Element.prototype.scrollIntoView ??= () => undefined; // jsdom has no layout
});

afterEach(cleanup);

describe("Launcher", () => {
  it("focuses search and launches the first match on Enter", () => {
    const { search, onLaunchApp } = setup();
    expect(document.activeElement).toBe(search);
    fireEvent.change(search, { target: { value: "co" } });
    fireEvent.keyDown(search, { key: "Enter" });
    expect(onLaunchApp).toHaveBeenCalledWith(apps[1]);
  });

  it("moves the selection with arrow keys", () => {
    const { search, onLaunchApp } = setup();
    fireEvent.change(search, { target: { value: "code" } });
    fireEvent.keyDown(search, { key: "ArrowDown" });
    fireEvent.keyDown(search, { key: "Enter" });
    expect(onLaunchApp).toHaveBeenCalledWith(apps[2]);
  });

  it("lists groups before apps and runs them", () => {
    const { search, onRunGroup } = setup();
    fireEvent.keyDown(search, { key: "Enter" });
    expect(onRunGroup).toHaveBeenCalledWith(groups[0]);
  });

  it("clears on the first Escape and closes on the second", () => {
    const { search, onEscapeEmpty } = setup();
    fireEvent.change(search, { target: { value: "mail" } });
    fireEvent.keyDown(search, { key: "Escape" });
    expect((search as HTMLInputElement).value).toBe("");
    expect(onEscapeEmpty).not.toHaveBeenCalled();
    fireEvent.keyDown(search, { key: "Escape" });
    expect(onEscapeEmpty).toHaveBeenCalledOnce();
  });

  it("ignores mousemove without movement", () => {
    const { search, onLaunchApp } = setup();
    fireEvent.mouseMove(screen.getByText("Code Notes"), { movementX: 0, movementY: 0 });
    fireEvent.keyDown(search, { key: "Enter" });
    expect(onLaunchApp).not.toHaveBeenCalled(); // the group is still first
  });
});
