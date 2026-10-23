import { useCallback, useEffect, useState } from "react";

import { comboFromKeyPress, displayHotkey } from "../lib/hotkey";
import * as ipc from "../lib/ipc";
import type { Settings } from "../lib/types";

const MAC = navigator.userAgent.includes("Mac");

interface Props {
  onError: (e: unknown) => void;
}

/** The global hotkey that opens the popup launcher: record a new one, or turn it off. */
export function HotkeySetting({ onError }: Props) {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [recording, setRecording] = useState(false);

  const reload = useCallback(() => ipc.getSettings().then(setSettings, onError), [onError]);
  useEffect(() => void reload(), [reload]);

  const save = async (hotkey: string | null) => {
    setRecording(false);
    try {
      await ipc.setPopupHotkey(hotkey);
    } catch (e) {
      onError(e);
    }
    await reload();
  };

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (!recording) return;
    e.preventDefault();
    // Escape cancels recording here instead of closing Settings.
    e.stopPropagation();
    if (e.code === "Escape" && !e.ctrlKey && !e.altKey && !e.shiftKey && !e.metaKey) {
      setRecording(false);
      return;
    }
    const combo = comboFromKeyPress(e);
    if (combo !== null) void save(combo);
  };

  if (settings === null) return null;
  const hotkey = settings.popup_hotkey;

  return (
    <fieldset className="fieldset">
      <legend>Popup launcher</legend>
      <p className="muted">A small search window you can open from anywhere to start an app or group.</p>
      {settings.wayland ? (
        <p className="muted">
          Global hotkeys do not work on Wayland. Bind <code>muster --popup</code> to a key in your compositor instead.
        </p>
      ) : (
        <div className="hotkey">
          <span>Hotkey:</span>
          <span className="hotkey__keys" aria-live="polite">
            {recording ? "Press the new hotkey…" : hotkey ? displayHotkey(hotkey, MAC) : "Off"}
          </span>
          <button
            type="button"
            className="button"
            aria-pressed={recording}
            onClick={() => setRecording((r) => !r)}
            onKeyDown={onKeyDown}
            onBlur={() => setRecording(false)}
          >
            {recording ? "Cancel" : "Change…"}
          </button>
          {hotkey && !recording && (
            <button type="button" className="button" onClick={() => void save(null)}>
              Turn off
            </button>
          )}
        </div>
      )}
      {!settings.wayland && hotkey && !settings.popup_hotkey_active && (
        <p className="warning" role="alert">
          Muster could not claim {displayHotkey(hotkey, MAC)}. Another app may already use it; pick another.
        </p>
      )}
      <p className="muted">
        You can also open it with <code>muster --popup</code>.
      </p>
    </fieldset>
  );
}
