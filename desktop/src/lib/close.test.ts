import { describe, expect, it } from "vitest";

import { closeSummary } from "./close";
import type { ClosedEntry } from "./types";

const name = (id: string) => ({ "m:editor": "Editor", "m:sync": "Sync", "m:shell": "Shell", "m:site": "Site" })[id] ?? id;

describe("closeSummary", () => {
  it("names what closed", () => {
    const closed: ClosedEntry[] = [
      { app: "m:editor", outcome: "closed" },
      { app: "m:sync", outcome: "closed" },
      { app: "m:site", outcome: "link" },
    ];
    expect(closeSummary("Work", closed, name)).toEqual({ text: "Closed Editor, Sync.", error: false });
  });

  it("reports what is still running as an error", () => {
    const closed: ClosedEntry[] = [
      { app: "m:editor", outcome: "closed" },
      { app: "m:sync", outcome: "still_running" },
    ];
    expect(closeSummary("Work", closed, name)).toEqual({ text: "Closed Editor. Still running: Sync.", error: true });
  });

  it("says why a shell entry was left alone", () => {
    const closed: ClosedEntry[] = [{ app: "m:shell", outcome: "too_broad" }];
    expect(closeSummary("Work", closed, name).text).toBe("Not closed, since it would close other programs too: Shell.");
  });

  it("says when nothing was running", () => {
    const closed: ClosedEntry[] = [
      { app: "m:editor", outcome: "not_running" },
      { app: "m:site", outcome: "link" },
    ];
    expect(closeSummary("Work", closed, name)).toEqual({ text: "Nothing from Work was running.", error: false });
  });

  it("names an app listed twice once", () => {
    const closed: ClosedEntry[] = [
      { app: "m:editor", outcome: "closed" },
      { app: "m:editor", outcome: "not_running" },
      { app: "m:editor", outcome: "closed" },
    ];
    expect(closeSummary("Work", closed, name).text).toBe("Closed Editor.");
  });
});
