import { useCallback, useEffect, useState } from "react";

import { Launcher } from "./components/Launcher";
import * as ipc from "./lib/ipc";
import type { AppView, GroupView } from "./lib/types";
import { useTheme } from "./lib/useTheme";

interface Status {
  text: string;
  error: boolean;
}

/** The popup launcher: only the search list. It closes itself once something starts. */
export function Popup() {
  const [apps, setApps] = useState<AppView[] | null>(null);
  const [groups, setGroups] = useState<GroupView[]>([]);
  const [theme, setTheme] = useState<string | null>(null);
  const [status, setStatus] = useState<Status | null>(null);
  useTheme(theme, null);

  const fail = useCallback((e: unknown) => {
    const err = ipc.asCommandError(e);
    // A cancelled approval is the user's choice: say so and stay open to pick something else.
    setStatus({ text: err.message, error: err.code !== "cancelled" });
  }, []);
  const close = useCallback(() => void ipc.closeWindow().catch(fail), [fail]);

  useEffect(() => {
    ipc.getSettings().then(
      (s) => setTheme(s.theme),
      () => setTheme("system"),
    );
    ipc.listApps().then(setApps, (e) => {
      setApps([]);
      fail(e);
    });
    ipc.listGroups().then(setGroups, fail);
  }, [fail]);

  const launchApp = (app: AppView) => ipc.launchApp(app.id).then(close, fail);

  const runGroup = async (group: GroupView) => {
    try {
      const failures = await ipc.runGroup(group.id);
      if (failures.length === 0) close();
      else setStatus({ text: failures.map((f) => f.message).join(" "), error: true });
    } catch (e) {
      fail(e);
    }
  };

  return (
    <main className="popup">
      <Launcher
        apps={apps}
        groups={groups}
        onLaunchApp={(a) => void launchApp(a)}
        onRunGroup={(g) => void runGroup(g)}
        onEscapeEmpty={close}
      />
      {status && (
        <p className={status.error ? "status status--error" : "status"} role="status">
          {status.text}
        </p>
      )}
    </main>
  );
}
