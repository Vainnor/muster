import { useCallback, useEffect, useState } from "react";

import * as ipc from "../lib/ipc";
import type { UpdateSettings, UpdateView } from "../lib/types";

interface Props {
  onError: (e: unknown) => void;
}

type Result = { kind: "none" } | { kind: "checking" } | { kind: "current" } | { kind: "found"; update: UpdateView };

/** The update check. Nothing goes over the network until the button is pressed, or the check at startup is turned on. */
export function UpdateSetting({ onError }: Props) {
  const [settings, setSettings] = useState<UpdateSettings | null>(null);
  const [result, setResult] = useState<Result>({ kind: "none" });

  useEffect(() => {
    ipc.getSettings().then((s) => setSettings(s.updates), onError);
  }, [onError]);

  const check = useCallback(async () => {
    setResult({ kind: "checking" });
    try {
      const update = await ipc.checkForUpdate();
      setResult(update ? { kind: "found", update } : { kind: "current" });
    } catch (e) {
      setResult({ kind: "none" });
      onError(e);
    }
  }, [onError]);

  const install = useCallback(async () => {
    try {
      await ipc.installUpdate();
    } catch (e) {
      // Saying no in the dialog is a choice, not an error.
      if (ipc.asCommandError(e).code !== "cancelled") onError(e);
    }
  }, [onError]);

  const setOnStart = useCallback(
    async (on: boolean) => {
      try {
        await ipc.setCheckUpdatesOnStart(on);
        setSettings((s) => s && { ...s, check_on_start: on });
      } catch (e) {
        onError(e);
      }
    },
    [onError],
  );

  if (!settings) return null;

  return (
    <fieldset className="fieldset">
      <legend>Updates</legend>
      {!settings.built_in ? (
        <p className="muted">This copy of Muster was built without the update check.</p>
      ) : (
        <>
          <div className="actions">
            <button type="button" className="button" disabled={result.kind === "checking"} onClick={() => void check()}>
              {result.kind === "checking" ? "Checking…" : "Check for updates"}
            </button>
          </div>
          {result.kind === "current" && <p className="muted">Muster is up to date.</p>}
          {result.kind === "found" && (
            <div className="update">
              <p>Muster {result.update.version} is available.</p>
              {result.update.notes && <p className="muted update__notes">{result.update.notes}</p>}
              <div className="actions">
                {result.update.install === "in_app" && (
                  <button type="button" className="button button--primary" onClick={() => void install()}>
                    Install and restart
                  </button>
                )}
                {result.update.install === "package_manager" && <p className="muted">Update Muster with your package manager.</p>}
                <button type="button" className="button" onClick={() => ipc.openReleasePage().catch(onError)}>
                  Open the release page
                </button>
              </div>
            </div>
          )}
          <label className="check">
            <input type="checkbox" checked={settings.check_on_start} onChange={(e) => void setOnStart(e.target.checked)} />
            Check for updates when Muster starts
          </label>
          <p className="muted">A check asks GitHub for the latest release and sends nothing else.</p>
        </>
      )}
    </fieldset>
  );
}
