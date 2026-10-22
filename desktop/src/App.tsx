import { useEffect, useState } from "react";

import { appInfo, themeTokens } from "./lib/ipc";
import { SYSTEM_THEME, applyTokens, resolveThemeId } from "./lib/theme";
import type { AppInfo } from "./lib/types";

const darkQuery = window.matchMedia("(prefers-color-scheme: dark)");

function useTheme(setting: string): void {
  const [prefersDark, setPrefersDark] = useState(darkQuery.matches);

  // An OS event, not a timer: fires only when the user changes their system theme.
  useEffect(() => {
    const onChange = (e: MediaQueryListEvent) => setPrefersDark(e.matches);
    darkQuery.addEventListener("change", onChange);
    return () => darkQuery.removeEventListener("change", onChange);
  }, []);

  useEffect(() => {
    themeTokens(resolveThemeId(setting, prefersDark))
      .then((vars) => applyTokens(document.documentElement.style, vars))
      .catch(() => undefined); // Base CSS keeps a readable fallback palette.
  }, [setting, prefersDark]);
}

export function App() {
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [error, setError] = useState(false);
  useTheme(SYSTEM_THEME);

  useEffect(() => {
    appInfo().then(setInfo, () => setError(true));
  }, []);

  return (
    <main className="shell">
      <header className="shell__header">
        <h1 className="shell__title">Muster</h1>
        {info && (
          <span className="shell__meta">
            v{info.version} · {info.os_label} ({info.arch})
          </span>
        )}
      </header>
      <input
        className="shell__search"
        type="search"
        placeholder="Search apps and groups"
        aria-label="Search apps and groups"
        autoFocus
        disabled
      />
      <section className="shell__empty" aria-live="polite">
        {error ? (
          <p>Could not reach the app core.</p>
        ) : (
          <p>App detection and groups are coming. This window proves the core → shell → UI path.</p>
        )}
      </section>
    </main>
  );
}
