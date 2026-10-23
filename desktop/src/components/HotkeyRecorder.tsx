import { useCallback, useEffect, useState } from "react";

import { MAC, comboFromKeyPress, displayHotkey } from "../lib/hotkey";

interface Props {
  hotkey: string | null;
  /** False with a hotkey: the OS refused it, usually because another app holds it. */
  active: boolean;
  /** Saves a new hotkey, or `null` to remove it. Rust validates and registers it. */
  onSave: (hotkey: string | null) => Promise<void>;
  /** What "remove" is called here: "Turn off" for the popup, "Remove" for a group. */
  removeLabel: string;
}

/** Shows a hotkey and records a new one from the next key press. */
export function HotkeyRecorder({ hotkey, active, onSave, removeLabel }: Props) {
  const [recording, setRecording] = useState(false);

  const save = useCallback(
    async (next: string | null) => {
      setRecording(false);
      await onSave(next);
    },
    [onSave],
  );

  // While recording, listen on the whole window: WebKit does not focus a button on click, so
  // the keys may never reach the button itself. Capture runs before React's handlers, so
  // Escape here cancels recording instead of closing the screen around it.
  useEffect(() => {
    if (!recording) return;
    const onKeyDown = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();
      if (e.code === "Escape" && !e.ctrlKey && !e.altKey && !e.shiftKey && !e.metaKey) {
        setRecording(false);
        return;
      }
      const combo = comboFromKeyPress(e);
      if (combo !== null) void save(combo);
    };
    window.addEventListener("keydown", onKeyDown, true);
    return () => window.removeEventListener("keydown", onKeyDown, true);
  }, [recording, save]);

  return (
    <>
      <div className="hotkey">
        <span>Hotkey:</span>
        <span className="hotkey__keys" aria-live="polite">
          {recording ? "Press the new hotkey…" : hotkey ? displayHotkey(hotkey, MAC) : "Off"}
        </span>
        <button type="button" className="button" aria-pressed={recording} onClick={() => setRecording((r) => !r)}>
          {recording ? "Cancel" : "Change…"}
        </button>
        {hotkey && !recording && (
          <button type="button" className="button" onClick={() => void save(null)}>
            {removeLabel}
          </button>
        )}
      </div>
      {hotkey && !active && (
        <p className="warning" role="alert">
          Muster could not claim {displayHotkey(hotkey, MAC)}. Another app may already use it; pick another.
        </p>
      )}
    </>
  );
}
