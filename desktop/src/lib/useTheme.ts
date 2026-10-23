import { useEffect, useState } from "react";

import * as ipc from "./ipc";
import { applyTokens, resolveThemeId } from "./theme";
import type { CssVar } from "./types";

const darkQuery = window.matchMedia("(prefers-color-scheme: dark)");

/** Applies the chosen theme (or a preview) to the page, following the OS light/dark setting. */
export function useTheme(setting: string | null, preview: CssVar[] | null): void {
  const [prefersDark, setPrefersDark] = useState(darkQuery.matches);

  // An OS event, not a timer: fires only when the user changes their system theme.
  useEffect(() => {
    const onChange = (e: MediaQueryListEvent) => setPrefersDark(e.matches);
    darkQuery.addEventListener("change", onChange);
    return () => darkQuery.removeEventListener("change", onChange);
  }, []);

  useEffect(() => {
    if (preview !== null) {
      applyTokens(document.documentElement.style, preview);
      return;
    }
    if (setting === null) return;
    ipc
      .themeTokens(resolveThemeId(setting, prefersDark))
      .then((vars) => applyTokens(document.documentElement.style, vars))
      .catch(() => undefined); // Base CSS keeps a readable fallback palette.
  }, [setting, prefersDark, preview]);
}
