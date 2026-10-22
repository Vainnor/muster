import { describe, expect, it } from "vitest";

import { describeWarning } from "./contrast";

describe("describeWarning", () => {
  it("names the pair and both ratios", () => {
    expect(describeWarning({ foreground: "muted_text", background: "surface", ratio: 3.14, required: 4.5 })).toBe(
      "Secondary text on panels: 3.1:1, needs 4.5:1",
    );
    expect(describeWarning({ foreground: "accent_text", background: "accent", ratio: 2, required: 4.5 })).toBe(
      "Button text on the accent color: 2.0:1, needs 4.5:1",
    );
  });
});
