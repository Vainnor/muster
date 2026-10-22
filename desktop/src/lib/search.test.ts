import { describe, expect, it } from "vitest";

import { move, parseArgs, rank } from "./search";

const items = [{ name: "Visual Studio Code" }, { name: "Code Notes" }, { name: "Xcode" }, { name: "Mail" }];

describe("rank", () => {
  it("orders prefix, then word start, then substring", () => {
    expect(rank(items, "code").map((i) => i.name)).toEqual(["Code Notes", "Visual Studio Code", "Xcode"]);
  });

  it("is case-insensitive and trims", () => {
    expect(rank(items, "  MAIL ").map((i) => i.name)).toEqual(["Mail"]);
  });

  it("keeps everything for an empty query", () => {
    expect(rank(items, "")).toHaveLength(4);
  });
});

describe("parseArgs", () => {
  it("splits lines and drops blanks", () => {
    expect(parseArgs("--a\n\n  two words \n")).toEqual(["--a", "two words"]);
    expect(parseArgs("  \n")).toBeNull();
  });
});

describe("move", () => {
  it("moves within bounds", () => {
    expect(move(["a", "b", "c"], 0, 1)).toEqual(["b", "a", "c"]);
    expect(move(["a", "b", "c"], 0, -1)).toEqual(["a", "b", "c"]);
    expect(move(["a", "b", "c"], 2, 5)).toEqual(["a", "b", "c"]);
  });
});
