import type { ContrastWarning } from "./types";

const LABELS: Record<string, string> = {
  text: "Text",
  muted_text: "Secondary text",
  accent_text: "Button text",
  accent: "Accent",
  background: "the background",
  surface: "panels",
};

/** "Secondary text on panels: 3.1:1, needs 4.5:1" */
export function describeWarning(w: ContrastWarning): string {
  const fg = LABELS[w.foreground] ?? w.foreground;
  const bg = w.background === "accent" ? "the accent color" : (LABELS[w.background] ?? w.background);
  return `${fg} on ${bg}: ${w.ratio.toFixed(1)}:1, needs ${w.required.toFixed(1)}:1`;
}

/** Theme files are at most 16 KiB; checked here too so a huge file is never read. */
export const MAX_THEME_BYTES = 16 * 1024;
