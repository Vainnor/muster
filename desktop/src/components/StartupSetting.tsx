import { useCallback, useEffect, useState } from "react";

import * as ipc from "../lib/ipc";

interface Props {
  onError: (e: unknown) => void;
}

/** Start Muster when the user logs in. The OS is asked each time; nothing is cached here. */
export function StartupSetting({ onError }: Props) {
  // undefined while loading; null when the OS state could not be read.
  const [enabled, setEnabled] = useState<boolean | null | undefined>(undefined);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    ipc.getSettings().then((s) => setEnabled(s.start_on_login), onError);
  }, [onError]);

  const toggle = useCallback(
    async (on: boolean) => {
      setBusy(true);
      try {
        setEnabled(await ipc.setStartOnLogin(on));
      } catch (e) {
        onError(e);
      } finally {
        setBusy(false);
      }
    },
    [onError],
  );

  if (enabled === undefined) return null;

  return (
    <fieldset className="fieldset">
      <legend>Startup</legend>
      {enabled === null ? (
        <p className="muted">Muster could not check whether it starts when you log in.</p>
      ) : (
        <label className="check">
          <input type="checkbox" checked={enabled} disabled={busy} onChange={(e) => void toggle(e.target.checked)} />
          Start Muster when I log in
        </label>
      )}
      <p className="muted">It starts in the tray, so the popup hotkey works right away.</p>
    </fieldset>
  );
}
