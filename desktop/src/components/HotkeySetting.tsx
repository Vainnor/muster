import { useCallback, useEffect, useState } from "react";

import * as ipc from "../lib/ipc";
import type { Settings } from "../lib/types";
import { HotkeyRecorder } from "./HotkeyRecorder";

interface Props {
  onError: (e: unknown) => void;
}

/** The global hotkey that opens the popup launcher: record a new one, or turn it off. */
export function HotkeySetting({ onError }: Props) {
  const [settings, setSettings] = useState<Settings | null>(null);

  const reload = useCallback(() => ipc.getSettings().then(setSettings, onError), [onError]);
  useEffect(() => void reload(), [reload]);

  const save = useCallback(
    async (hotkey: string | null) => {
      try {
        await ipc.setPopupHotkey(hotkey);
      } catch (e) {
        onError(e);
      }
      await reload();
    },
    [onError, reload],
  );

  if (settings === null) return null;

  return (
    <fieldset className="fieldset">
      <legend>Popup launcher</legend>
      <p className="muted">A small search window you can open from anywhere to start an app or group.</p>
      {settings.wayland ? (
        <p className="muted">
          Global hotkeys do not work on Wayland. Bind <code>muster --popup</code> to a key in your compositor instead.
        </p>
      ) : (
        <HotkeyRecorder hotkey={settings.popup_hotkey} active={settings.popup_hotkey_active} onSave={save} removeLabel="Turn off" />
      )}
      <p className="muted">
        You can also open it with <code>muster --popup</code>.
      </p>
    </fieldset>
  );
}
