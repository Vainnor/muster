import { useMemo, useState } from "react";

import { move, parseArgs, rank } from "../lib/search";
import type { AppView, GroupDraft, GroupEntry, GroupView } from "../lib/types";
import { GroupHotkey } from "./GroupHotkey";

interface Props {
  apps: AppView[];
  group: GroupView | null;
  onSave: (draft: GroupDraft) => Promise<boolean>;
  onDelete: (id: string) => void;
  onCancel: () => void;
  /** Reload the group list after the hotkey changes. */
  onHotkeyChanged: () => Promise<void>;
  onError: (e: unknown) => void;
}

interface Row extends GroupEntry {
  key: number;
  argsText: string;
}

let nextKey = 0;
const toRow = (e: GroupEntry): Row => ({ ...e, key: nextKey++, argsText: (e.args ?? []).join("\n") });

/** Create or edit a group: name, apps in launch order, and per-entry delay and arguments. */
export function GroupEditor({ apps, group, onSave, onDelete, onCancel, onHotkeyChanged, onError }: Props) {
  const [name, setName] = useState(group?.name ?? "");
  const [rows, setRows] = useState<Row[]>(() => (group?.entries ?? []).map(toRow));
  const [filter, setFilter] = useState("");
  const names = useMemo(() => new Map(apps.map((a) => [a.id, a.name])), [apps]);
  const candidates = rank(apps, filter).slice(0, 8);

  const update = (key: number, change: Partial<Row>) =>
    setRows((rs) => rs.map((r) => (r.key === key ? { ...r, ...change } : r)));

  const save = () =>
    void onSave({
      id: group?.id ?? null,
      name,
      entries: rows.map(({ app, argsText, cwd, delay_ms }) => ({
        app,
        args: parseArgs(argsText),
        cwd: cwd && cwd.trim() !== "" ? cwd.trim() : null,
        delay_ms,
      })),
    });

  return (
    <form
      className="panel"
      onSubmit={(e) => {
        e.preventDefault();
        save();
      }}
      onKeyDown={(e) => {
        if (e.key === "Escape") onCancel();
      }}
    >
      <h2 className="panel__title">{group ? "Edit group" : "New group"}</h2>
      <label className="label">
        Name
        <input className="field" value={name} maxLength={64} autoFocus required onChange={(e) => setName(e.target.value)} />
      </label>

      <fieldset className="fieldset">
        <legend>Apps, in launch order</legend>
        {rows.length === 0 && <p className="muted">Add apps below.</p>}
        <ol className="entries">
          {rows.map((r, i) => (
            <li key={r.key} className="entry">
              <div className="entry__head">
                <span className="entry__name">{names.get(r.app) ?? "Missing app"}</span>
                <button type="button" className="button button--quiet" aria-label="Move up" disabled={i === 0} onClick={() => setRows((rs) => move(rs, i, -1))}>
                  ↑
                </button>
                <button type="button" className="button button--quiet" aria-label="Move down" disabled={i === rows.length - 1} onClick={() => setRows((rs) => move(rs, i, 1))}>
                  ↓
                </button>
                <button type="button" className="button button--quiet" onClick={() => setRows((rs) => rs.filter((x) => x.key !== r.key))}>
                  Remove
                </button>
              </div>
              <div className="entry__fields">
                <label className="label label--inline">
                  Wait before (seconds)
                  <input
                    className="field field--short"
                    type="number"
                    min={0}
                    max={60}
                    step={0.5}
                    value={r.delay_ms / 1000}
                    onChange={(e) => update(r.key, { delay_ms: Math.round(Math.max(0, Math.min(60, Number(e.target.value) || 0)) * 1000) })}
                  />
                </label>
                <label className="label">
                  Arguments (one per line, empty uses the app's own)
                  <textarea className="field" rows={2} value={r.argsText} onChange={(e) => update(r.key, { argsText: e.target.value })} />
                </label>
                <label className="label">
                  Working folder (optional)
                  <input className="field" value={r.cwd ?? ""} onChange={(e) => update(r.key, { cwd: e.target.value })} />
                </label>
              </div>
            </li>
          ))}
        </ol>
        <label className="label">
          Add an app
          <input className="field" type="search" placeholder="Search apps" value={filter} onChange={(e) => setFilter(e.target.value)} />
        </label>
        <ul className="picker">
          {candidates.map((a) => (
            <li key={a.id}>
              <button
                type="button"
                className="button button--quiet picker__item"
                disabled={rows.length >= 32}
                onClick={() => setRows((rs) => [...rs, toRow({ app: a.id, args: null, cwd: null, delay_ms: 0 })])}
              >
                + {a.name}
              </button>
            </li>
          ))}
        </ul>
      </fieldset>

      <fieldset className="fieldset">
        <legend>Hotkey</legend>
        {group ? (
          <GroupHotkey group={group} onChanged={onHotkeyChanged} onError={onError} />
        ) : (
          <p className="muted">Save the group first, then give it a hotkey.</p>
        )}
      </fieldset>

      <div className="actions">
        <button type="submit" className="button button--primary">
          Save group
        </button>
        <button type="button" className="button" onClick={onCancel}>
          Cancel
        </button>
        {group && (
          <button type="button" className="button button--danger" onClick={() => onDelete(group.id)}>
            Delete group
          </button>
        )}
      </div>
    </form>
  );
}
