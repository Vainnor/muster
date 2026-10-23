// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { AppView } from "../lib/types";
import { GroupEditor } from "./GroupEditor";

const apps: AppView[] = [
  { id: "d:editor", name: "Editor", source: "detected", kind: "program", detail: "" },
  { id: "d:browser", name: "Browser", source: "detected", kind: "program", detail: "" },
];

afterEach(cleanup);

describe("GroupEditor", () => {
  it("submits entries in order with parsed arguments and delays", () => {
    const onSave = vi.fn().mockResolvedValue(true);
    render(<GroupEditor apps={apps} group={null} onSave={onSave} onDelete={vi.fn()} onCancel={vi.fn()} onHotkeyChanged={vi.fn()} onError={vi.fn()} />);
    fireEvent.change(screen.getByLabelText("Name"), { target: { value: "Work" } });
    fireEvent.click(screen.getByText("+ Editor"));
    fireEvent.click(screen.getByText("+ Browser"));
    fireEvent.click(screen.getAllByLabelText("Move up")[1]!);
    fireEvent.change(screen.getAllByLabelText(/Wait before/)[1]!, { target: { value: "1.5" } });
    fireEvent.change(screen.getAllByLabelText(/Arguments/)[0]!, { target: { value: "--private\n\nhttps://a.example" } });
    fireEvent.click(screen.getByText("Save group"));

    expect(onSave).toHaveBeenCalledWith({
      id: null,
      name: "Work",
      entries: [
        { app: "d:browser", args: ["--private", "https://a.example"], cwd: null, delay_ms: 0 },
        { app: "d:editor", args: null, cwd: null, delay_ms: 1500 },
      ],
    });
  });

  it("clamps delays to 60 seconds", () => {
    const onSave = vi.fn().mockResolvedValue(true);
    render(<GroupEditor apps={apps} group={null} onSave={onSave} onDelete={vi.fn()} onCancel={vi.fn()} onHotkeyChanged={vi.fn()} onError={vi.fn()} />);
    fireEvent.change(screen.getByLabelText("Name"), { target: { value: "W" } });
    fireEvent.click(screen.getByText("+ Editor"));
    fireEvent.change(screen.getByLabelText(/Wait before/), { target: { value: "999" } });
    fireEvent.click(screen.getByText("Save group"));
    expect(onSave.mock.calls[0]![0].entries[0].delay_ms).toBe(60000);
  });
});
