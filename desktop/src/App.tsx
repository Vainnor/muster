import { useCallback, useEffect, useState } from "react";

import { AddApp } from "./components/AddApp";
import { ConfirmGroup } from "./components/ConfirmGroup";
import { GroupEditor } from "./components/GroupEditor";
import { Launcher } from "./components/Launcher";
import { Settings } from "./components/Settings";
import * as ipc from "./lib/ipc";
import { applyTokens, resolveThemeId } from "./lib/theme";
import type { AppInfo, AppView, GroupView, LaunchTarget, ThemeInfo } from "./lib/types";

const darkQuery = window.matchMedia("(prefers-color-scheme: dark)");

function useTheme(setting: string | null): void {
  const [prefersDark, setPrefersDark] = useState(darkQuery.matches);

  // An OS event, not a timer: fires only when the user changes their system theme.
  useEffect(() => {
    const onChange = (e: MediaQueryListEvent) => setPrefersDark(e.matches);
    darkQuery.addEventListener("change", onChange);
    return () => darkQuery.removeEventListener("change", onChange);
  }, []);

  useEffect(() => {
    if (setting === null) return;
    ipc
      .themeTokens(resolveThemeId(setting, prefersDark))
      .then((vars) => applyTokens(document.documentElement.style, vars))
      .catch(() => undefined); // Base CSS keeps a readable fallback palette.
  }, [setting, prefersDark]);
}

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
  const [confirm, setConfirm] = useState<{ group: GroupView; entries: AppView[] } | null>(null);
  useTheme(theme);

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

  const launchApp = (app: AppView) =>
    ipc.launchApp(app.id).then(() => say(`Started ${app.name}.`), fail);

  const startGroup = async (group: GroupView) => {
    try {
      const failures = await ipc.runGroup(group.id);
      if (failures.length === 0) say(`Starting ${group.name}.`);
      else setStatus({ text: failures.map((f) => `${appName(f.app)}: ${f.message}`).join(" "), error: true });
    } catch (e) {
      if (ipc.asCommandError(e).code === "needs_confirmation") {
        const entries = await ipc.groupConfirmation(group.id).catch(() => []);
        setConfirm({ group, entries });
      } else fail(e);
    }
  };

  const confirmAndRun = async () => {
    if (!confirm) return;
    const { group } = confirm;
    setConfirm(null);
    try {
      await ipc.confirmGroup(group.id);
      await reloadGroups();
      await startGroup(group);
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
                <span className="row__meta">{g.entries.map((e) => e.app_name ?? "Missing app").join(", ") || "Empty"}</span>
                <button type="button" className="button button--primary" onClick={() => void startGroup(g)}>
                  Run
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
        <GroupEditor apps={apps ?? []} group={view.group} onSave={saveGroup} onDelete={(id) => void removeGroup(id)} onCancel={() => setView({ name: "groups" })} />
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

      {view.name === "add" && <AddApp isMac={info?.os === "macos"} onAdd={addApp} onCancel={() => setView({ name: "apps" })} />}

      {view.name === "settings" && <Settings themes={themes} theme={theme ?? "system"} onTheme={chooseTheme} onClose={home} />}

      {confirm && <ConfirmGroup groupName={confirm.group.name} entries={confirm.entries} onConfirm={() => void confirmAndRun()} onCancel={() => setConfirm(null)} />}

      {info && (
        <footer className="shell__footer">
          Muster {info.version} · {info.os_label}
        </footer>
      )}
    </main>
  );
}
