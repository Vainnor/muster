import { useEffect, useMemo, useRef, useState } from "react";

import { rank } from "../lib/search";
import type { AppView, GroupView } from "../lib/types";

type Result = { type: "group"; group: GroupView; name: string } | { type: "app"; app: AppView; name: string };

interface Props {
  apps: AppView[] | null;
  groups: GroupView[];
  onLaunchApp: (app: AppView) => void;
  onRunGroup: (group: GroupView) => void;
  onEscapeEmpty: () => void;
}

/** Search-first list of groups and apps. Typing filters; arrows move; Enter starts. */
export function Launcher({ apps, groups, onLaunchApp, onRunGroup, onEscapeEmpty }: Props) {
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState(0);
  const listRef = useRef<HTMLUListElement>(null);

  const results = useMemo<Result[]>(
    () => [
      ...rank(groups, query).map((group) => ({ type: "group" as const, group, name: group.name })),
      ...rank(apps ?? [], query).map((app) => ({ type: "app" as const, app, name: app.name })),
    ],
    [apps, groups, query],
  );

  useEffect(() => setSelected(0), [query]);
  useEffect(() => {
    listRef.current?.querySelector<HTMLElement>(`[data-index="${selected}"]`)?.scrollIntoView({ block: "nearest" });
  }, [selected]);

  const start = (r: Result | undefined) => {
    if (r?.type === "group") onRunGroup(r.group);
    else if (r?.type === "app") onLaunchApp(r.app);
  };

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setSelected((s) => Math.min(results.length - 1, s + 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setSelected((s) => Math.max(0, s - 1));
    } else if (e.key === "Enter") {
      e.preventDefault();
      start(results[selected]);
    } else if (e.key === "Escape") {
      e.preventDefault();
      if (query !== "") setQuery("");
      else onEscapeEmpty();
    }
  };

  return (
    <div className="launcher">
      <input
        className="field launcher__search"
        type="search"
        placeholder="Search apps and groups"
        aria-label="Search apps and groups"
        aria-controls="results"
        aria-activedescendant={results.length > 0 ? `result-${selected}` : undefined}
        autoFocus
        value={query}
        onChange={(e) => setQuery(e.target.value)}
        onKeyDown={onKeyDown}
      />
      {apps === null ? (
        <p className="muted">Looking for installed apps…</p>
      ) : results.length === 0 ? (
        <p className="muted">{query ? "Nothing matches." : "No apps found yet. Add one by hand."}</p>
      ) : (
        <ul id="results" className="results" role="listbox" aria-label="Results" ref={listRef}>
          {results.map((r, i) => (
            <li
              key={r.type === "group" ? r.group.id : r.app.id}
              id={`result-${i}`}
              data-index={i}
              role="option"
              aria-selected={i === selected}
              className={i === selected ? "result result--selected" : "result"}
              // Only real pointer movement selects: WebKit also fires mousemove when rows
              // render under a resting pointer, which would steal Enter from the first match.
              onMouseMove={(e) => {
                if (e.movementX !== 0 || e.movementY !== 0) setSelected(i);
              }}
              onClick={() => start(r)}
            >
              <span className="result__name">{r.name}</span>
              <span className="result__meta">
                {r.type === "group"
                  ? `Group · ${r.group.entries.length} ${r.group.entries.length === 1 ? "app" : "apps"}`
                  : r.app.source === "manual"
                    ? "Added by you"
                    : r.app.kind === "url"
                      ? "Link"
                      : "App"}
              </span>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
