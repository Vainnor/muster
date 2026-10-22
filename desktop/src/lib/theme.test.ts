import { describe, expect, it } from "vitest";

import { applyTokens, isTokenName, resolveThemeId } from "./theme";

describe("resolveThemeId", () => {
  it("follows the system setting", () => {
    expect(resolveThemeId("system", true)).toBe("dark");
    expect(resolveThemeId("system", false)).toBe("light");
  });

  it("keeps an explicit preset", () => {
    expect(resolveThemeId("nord", false)).toBe("nord");
  });
});

describe("applyTokens", () => {
  it("applies only --mu-* properties", () => {
    const set: Array<[string, string]> = [];
    const style = { setProperty: (n: string, v: string) => void set.push([n, v]) };
    const applied = applyTokens(style, [
      { name: "--mu-bg", value: "#2e3440" },
      { name: "background", value: "url(https://evil.example)" },
      { name: "--other", value: "x" },
    ]);
    expect(applied).toBe(1);
    expect(set).toEqual([["--mu-bg", "#2e3440"]]);
  });

  it("rejects names outside the namespace", () => {
    expect(isTokenName("--mu-space-4")).toBe(true);
    expect(isTokenName("--mu-BG")).toBe(false);
    expect(isTokenName("--mu-bg;color:red")).toBe(false);
  });
});
