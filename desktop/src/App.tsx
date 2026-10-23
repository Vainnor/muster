import { useCallback, useEffect, useState } from "react";

import { AddApp } from "./components/AddApp";
import { GroupEditor } from "./components/GroupEditor";
import { Launcher } from "./components/Launcher";
import { Settings } from "./components/Settings";
import { closeSummary } from "./lib/close";
import { MAC, displayHotkey } from "./lib/hotkey";
import * as ipc from "./lib/ipc";
import { useTheme } from "./lib/useTheme";
import type { AppInfo, AppView, CssVar, GroupView, LaunchTarget, ThemeInfo } from "./lib/types";

type View = { name: "launcher" } | { name: "groups" } | { name: "edit"; group: GroupView | null } | { name: "add" } | { name: "apps" } | { name: "settings" };

interface Status {
  text: string;
  error: boolean;
}

const NOTICES: Record<string, string> = {
  config_recovered: "Your settings file was damaged. Muster kept a copy beside it and started fresh.",
  config_unreadable: "Your settings file could not be read. Changes this session will not be saved.",
};

export function App() {
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [apps, setApps] = useState<AppView[] | null>(null);
  const [groups, setGroups] = useState<GroupView[]>([]);
  const [themes, setThemes] = useState<ThemeInfo[]>([]);
  const [theme, setThemeSetting] = useState<string | null>(null);
  const [view, setView] = useState<View>({ name: "launcher" });
  const [status, setStatus] = useState<Status | null>(null);
  const [preview, setPreview] = useState<CssVar[] | null>(null);
  useTheme(theme, preview);

  const fail = useCallback((e: unknown) => setStatus({ text: ipc.asCommandError(e).message, error: true }), []);
  const say = (text: string) => setStatus({ text, error: false });
  const appName = useCallback((id: string) => apps?.find((a) => a.id === id)?.name ?? "An app", [apps]);

  const reloadGroups = useCallback(() => ipc.listGroups().then(setGroups, fail), [fail]);

  useEffect(() => {
    ipc.appInfo().then(setInfo, fail);
    ipc.getSettings().then((s) => setThemeSetting(s.theme), () => setThemeSetting("system"));
    ipc.listThemes().then(setThemes, fail);
    ipc.startupNotice().then((n) => n && setStatus({ text: NOTICES[n] ?? "", error: true }), fail);
    ipc.listApps().then(setApps, (e) => {
      setApps([]);
      fail(e);
    });
    void reloadGroups();
  }, [fail, reloadGroups]);

  useEffect(() => {
    const off = ipc.onLaunchFailed((f) => setStatus({ text: `${appName(f.app)}: ${f.message}`, error: true }));
    return () => void off.then((unlisten) => unlisten());
  }, [appName]);

  // A cancelled approval is the user's choice, not an error.
  const failOrCancel = useCallback(
    (e: unknown) => {
      const err = ipc.asCommandError(e);
      if (err.code === "cancelled") setStatus({ text: err.message, error: false });
      else fail(e);
    },
    [fail],
  );

  // Rust shows the approval dialog itself when an app or group runs something not yet approved.
  const launchApp = (app: AppView) =>
    ipc.launchApp(app.id).then(() => say(`Started ${app.name}.`), failOrCancel);

  const startGroup = async (group: GroupView) => {
    try {
      const failures = await ipc.runGroup(group.id);
      if (failures.length === 0) say(`Starting ${group.name}.`);
      else setStatus({ text: failures.map((f) => `${appName(f.app)}: ${f.message}`).join(" "), error: true });
      if (group.needs_confirmation) void reloadGroups();
    } catch (e) {
      failOrCancel(e);
    }
  };

  const closeGroup = async (group: GroupView) => {
    say(`Closing ${group.name}…`);
    try {
      setStatus(closeSummary(group.name, await ipc.closeGroup(group.id), appName));
    } catch (e) {
      fail(e);
    }
  };

  const saveGroup = async (draft: Parameters<typeof ipc.saveGroup>[0]) => {
    try {
      await ipc.saveGroup(draft);
      await reloadGroups();
      say(`Saved ${draft.name.trim()}.`);
      setView({ name: "groups" });
      return true;
    } catch (e) {
      fail(e);
      return false;
    }
  };

  const removeGroup = async (id: string) => {
    try {
      await ipc.deleteGroup(id);
      await reloadGroups();
      say("Group deleted.");
      setView({ name: "groups" });
    } catch (e) {
      fail(e);
    }
  };

  const addApp = async (name: string, target: LaunchTarget) => {
    try {
      await ipc.addApp(name, target);
      setApps(await ipc.listApps());
      say(`Added ${name.trim()}.`);
      setView({ name: "launcher" });
      return true;
    } catch (e) {
      fail(e);
      return false;
    }
  };

  const removeApp = async (app: AppView) => {
    try {
      await ipc.removeApp(app.id);
      setApps(await ipc.listApps());
      await reloadGroups();
      say(`Removed ${app.name}.`);
    } catch (e) {
      fail(e);
    }
  };

  const chooseTheme = (id: string) => {
    setThemeSetting(id);
    ipc.setTheme(id).catch(fail);
  };

  const refresh = () => {
    setApps(null);
    ipc.refreshApps().then(setApps, (e) => {
      setApps([]);
      fail(e);
    });
  };

  const home = () => setView({ name: "launcher" });
  const nav: Array<[View["name"], string]> = [
    ["launcher", "Launch"],
    ["groups", "Groups"],
    ["apps", "Apps"],
    ["settings", "Settings"],
  ];

  return (
    <main className="shell">
      <header className="shell__header">
        <h1 className="shell__title">Muster</h1>
        <nav className="nav" aria-label="Sections">
          {nav.map(([name, label]) => (
            <button
              key={name}
              type="button"
              className={view.name === name ? "nav__item nav__item--current" : "nav__item"}
              aria-current={view.name === name ? "page" : undefined}
              onClick={() => setView({ name } as View)}
            >
              {label}
            </button>
          ))}
        </nav>
      </header>

      <p className={status?.error ? "status status--error" : "status"} role="status" aria-live="polite">
        {status?.text}
      </p>

      {view.name === "launcher" && (
        <Launcher apps={apps} groups={groups} onLaunchApp={launchApp} onRunGroup={(g) => void startGroup(g)} onEscapeEmpty={() => void ipc.closeWindow().catch(fail)} />
      )}

      {view.name === "groups" && (
        <section className="panel" aria-labelledby="groups-title" onKeyDown={(e) => e.key === "Escape" && home()}>
          <h2 id="groups-title" className="panel__title">
            Groups
          </h2>
          {groups.length === 0 && <p className="muted">A group starts several apps with one action, in the order you choose.</p>}
          <ul className="rows">
            {groups.map((g) => (
              <li key={g.id} className="row">
                <span className="row__name">{g.name}</span>
                <span className="row__meta">
                  {g.entries.map((e) => e.app_name ?? "Missing app").join(", ") || "Empty"}
                  {g.hotkey && ` · ${displayHotkey(g.hotkey, MAC)}`}
                </span>
                <button type="button" className="button button--primary" onClick={() => void startGroup(g)}>
                  Run
                </button>
                <button type="button" className="button" onClick={() => void closeGroup(g)}>
                  Close
                </button>
                <button type="button" className="button" onClick={() => setView({ name: "edit", group: g })}>
                  Edit
                </button>
              </li>
            ))}
          </ul>
          <div className="actions">
            <button type="button" className="button" autoFocus onClick={() => setView({ name: "edit", group: null })}>
              New group
            </button>
          </div>
        </section>
      )}

      {view.name === "edit" && (
        <GroupEditor
          apps={apps ?? []}
          group={view.group}
          onSave={saveGroup}
          onDelete={(id) => void removeGroup(id)}
          onCancel={() => setView({ name: "groups" })}
          onHotkeyChanged={reloadGroups}
          onError={fail}
        />
      )}

      {view.name === "apps" && (
        <section className="panel" aria-labelledby="apps-title" onKeyDown={(e) => e.key === "Escape" && home()}>
          <h2 id="apps-title" className="panel__title">
            Apps
          </h2>
          <div className="actions">
            <button type="button" className="button" autoFocus onClick={() => setView({ name: "add" })}>
              Add an app
            </button>
            <button type="button" className="button" onClick={refresh}>
              Find installed apps again
            </button>
          </div>
          {apps === null ? (
            <p className="muted">Looking for installed apps…</p>
          ) : (
            <ul className="rows">
              {apps.map((a) => (
                <li key={a.id} className="row">
                  <span className="row__name">{a.name}</span>
                  <span className="row__meta" title={a.detail}>
                    {a.detail}
                  </span>
                  {a.source === "manual" && (
                    <button type="button" className="button button--quiet" onClick={() => void removeApp(a)}>
                      Remove
                    </button>
                  )}
                </li>
              ))}
            </ul>
          )}
        </section>
      )}

      {view.name === "add" && <AddApp isMac={info?.os === "macos"} onAdd={addApp} onError={fail} onCancel={() => setView({ name: "apps" })} />}

      {view.name === "settings" && (
        <Settings
          themes={themes}
          theme={theme ?? "system"}
          onTheme={chooseTheme}
          onPreview={setPreview}
          onThemesChanged={() => ipc.listThemes().then(setThemes, fail)}
          onError={fail}
          onClose={() => {
            setPreview(null);
            home();
          }}
        />
      )}


      {info && (
        <footer className="shell__footer">
          Muster {info.version} · {info.os_label}
        </footer>
      )}
    </main>
  );
}
