import { describe, expect, it } from "vitest";

import { baseName } from "./path";

describe("baseName", () => {
  it("drops the folder and the extension", () => {
    expect(baseName("/Applications/Editor.app")).toBe("Editor");
    expect(baseName("C:\\Apps\\tool.exe")).toBe("tool");
  });

  it("keeps a name with no extension or a leading dot", () => {
    expect(baseName("/usr/bin/htop")).toBe("htop");
    expect(baseName("/opt/tools/.hidden")).toBe(".hidden");
  });

  it("ignores a trailing separator", () => {
    expect(baseName("/Applications/Editor.app/")).toBe("Editor");
  });

  it("stays within the name limit", () => {
    expect(baseName(`/opt/${"a".repeat(200)}`)).toHaveLength(128);
  });
});
