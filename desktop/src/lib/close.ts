import type { ClosedEntry } from "./types";

/** The status line after closing `group`. `appName` turns an app id into its name. */
export function closeSummary(group: string, closed: ClosedEntry[], appName: (id: string) => string): { text: string; error: boolean } {
  const names = (outcome: ClosedEntry["outcome"]) => [...new Set(closed.filter((c) => c.outcome === outcome).map((c) => appName(c.app)))].join(", ");
  const parts: string[] = [];
  const stillRunning = names("still_running");
  const closedNames = names("closed");
  if (closedNames) parts.push(`Closed ${closedNames}.`);
  if (stillRunning) parts.push(`Still running: ${stillRunning}.`);
  if (names("too_broad")) parts.push(`Not closed, since it would close other programs too: ${names("too_broad")}.`);
  if (parts.length === 0) parts.push(`Nothing from ${group} was running.`);
  return { text: parts.join(" "), error: stillRunning !== "" };
}
