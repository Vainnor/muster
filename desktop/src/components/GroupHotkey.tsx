import { useCallback, useEffect, useState } from "react";

import * as ipc from "../lib/ipc";
import type { GroupView } from "../lib/types";
import { HotkeyRecorder } from "./HotkeyRecorder";

interface Props {
  group: GroupView;
  onChanged: () => Promise<void>;
  onError: (e: unknown) => void;
}

/** A group's own hotkey, saved as soon as it is recorded: the OS has to accept it first. */
export function GroupHotkey({ group, onChanged, onError }: Props) {
  const [hotkey, setHotkey] = useState(group.hotkey);
  const [active, setActive] = useState(group.hotkey_active);
  const [wayland, setWayland] = useState<boolean | null>(null);

  useEffect(() => {
    ipc.getSettings().then((s) => setWayland(s.wayland), onError);
  }, [onError]);

  const save = useCallback(
    async (next: string | null) => {
      try {
        setHotkey(await ipc.setGroupHotkey(group.id, next));
        // Saved means the OS accepted it.
        setActive(true);
        await onChanged();
      } catch (e) {
        onError(e);
      }
    },
    [group.id, onChanged, onError],
  );

  if (wayland === null) return null;
  if (wayland) {
    return (
      <p className="muted">
        Global hotkeys do not work on Wayland. Bind <code>muster --run-group {group.id}</code> to a key in your compositor instead.
      </p>
    );
  }
  return (
    <>
      <p className="muted">Runs this group from anywhere, even with Muster's window closed.</p>
      <HotkeyRecorder hotkey={hotkey} active={active} onSave={save} removeLabel="Remove" />
    </>
  );
}
