import { useEffect, useRef } from "react";

import type { AppView } from "../lib/types";

interface Props {
  groupName: string;
  entries: AppView[];
  onConfirm: () => void;
  onCancel: () => void;
}

/** The one confirmation in Muster: before the first run of a group with apps added by hand. */
export function ConfirmGroup({ groupName, entries, onConfirm, onCancel }: Props) {
  const ref = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    const dialog = ref.current;
    if (dialog && !dialog.open) dialog.showModal();
    return () => dialog?.close();
  }, []);

  return (
    <dialog
      ref={ref}
      className="dialog"
      aria-labelledby="confirm-title"
      onCancel={(e) => {
        e.preventDefault();
        onCancel();
      }}
    >
      <h2 id="confirm-title" className="panel__title">
        Run “{groupName}”?
      </h2>
      <p>This group starts programs you added by hand. Check that you trust each one:</p>
      <ul className="confirm-list">
        {entries.map((a) => (
          <li key={a.id}>
            <strong>{a.name}</strong>
            <code className="confirm-list__detail">{a.detail}</code>
          </li>
        ))}
      </ul>
      <p className="muted">Muster asks again if you add another program by hand to this group.</p>
      <div className="actions">
        <button type="button" className="button button--primary" autoFocus onClick={onConfirm}>
          Run group
        </button>
        <button type="button" className="button" onClick={onCancel}>
          Cancel
        </button>
      </div>
    </dialog>
  );
}
