import type { CssVar } from "./types";

export const SYSTEM_THEME = "system";

/** Resolves the "follow system" setting to a preset id. */
export function resolveThemeId(setting: string, prefersDark: boolean): string {
  if (setting !== SYSTEM_THEME) return setting;
  return prefersDark ? "dark" : "light";
}

/** Only custom properties in our namespace are applied; anything else is ignored. */
export function isTokenName(name: string): boolean {
  return /^--mu-[a-z0-9-]+$/.test(name);
}

/**
 * Applies tokens through the CSSOM. `setProperty` is allowed by the CSP, unlike inline
 * `style` attributes or injected `<style>` elements.
 */
export function applyTokens(style: Pick<CSSStyleDeclaration, "setProperty">, vars: CssVar[]): number {
  let applied = 0;
  for (const v of vars) {
    if (!isTokenName(v.name)) continue;
    style.setProperty(v.name, v.value);
    applied += 1;
  }
  return applied;
}
