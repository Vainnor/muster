import { useState } from "react";

import * as ipc from "../lib/ipc";
import { baseName } from "../lib/path";
import { parseArgs } from "../lib/search";
import type { LaunchTarget, PickKind } from "../lib/types";

interface Props {
  isMac: boolean;
  onAdd: (name: string, target: LaunchTarget) => Promise<boolean>;
  onError: (e: unknown) => void;
  onCancel: () => void;
}

type Kind = "program" | "app_bundle" | "url";

/** Add a program, script, macOS app bundle or link by hand. Core validates it. */
export function AddApp({ isMac, onAdd, onError, onCancel }: Props) {
  const [kind, setKind] = useState<Kind>("program");
  const [name, setName] = useState("");
  const [location, setLocation] = useState("");
  const [args, setArgs] = useState("");
  const [cwd, setCwd] = useState("");

  // The native picker runs in Rust; it returns the chosen path, or null if the user cancels.
  const browse = async (pick: PickKind) => {
    try {
      const picked = await ipc.pickPath(pick);
      if (!picked) return;
      if (picked.kind === "folder") {
        setCwd(picked.path);
        return;
      }
      setKind(picked.kind);
      setLocation(picked.path);
      if (name.trim() === "") setName(baseName(picked.path));
    } catch (e) {
      onError(e);
    }
  };

  const target = (): LaunchTarget => {
    if (kind === "url") return { kind, url: location.trim() };
    if (kind === "app_bundle") return { kind, path: location.trim() };
    return { kind, path: location.trim(), args: parseArgs(args) ?? [], cwd: cwd.trim() === "" ? null : cwd.trim() };
  };

  return (
    <form
      className="panel"
      onSubmit={(e) => {
        e.preventDefault();
        void onAdd(name, target());
      }}
      onKeyDown={(e) => {
        if (e.key === "Escape") onCancel();
      }}
    >
      <h2 className="panel__title">Add an app</h2>
      <p className="muted">
        For programs Muster did not find. The first time one runs, Muster shows exactly what it will start and asks you first.
      </p>
      <label className="label">
        Kind
        <select className="field" value={kind} onChange={(e) => setKind(e.target.value as Kind)}>
          <option value="program">Program or script</option>
          {isMac && <option value="app_bundle">macOS app (.app)</option>}
          <option value="url">Web or mail link</option>
        </select>
      </label>
      <label className="label">
        Name
        <input className="field" value={name} maxLength={128} required autoFocus onChange={(e) => setName(e.target.value)} />
      </label>
      <div className="field-row">
        <label className="label">
          {kind === "url" ? "Link (http, https or mailto)" : "Full path"}
          <input
            className="field"
            value={location}
            required
            placeholder={kind === "url" ? "https://example.com" : kind === "app_bundle" ? "/Applications/App.app" : "/usr/bin/app"}
            onChange={(e) => setLocation(e.target.value)}
          />
        </label>
        {kind !== "url" && (
          <button
            type="button"
            className="button"
            aria-label={kind === "app_bundle" ? "Browse for an app" : "Browse for a program"}
            onClick={() => void browse(kind)}
          >
            Browse…
          </button>
        )}
      </div>
      {kind === "program" && (
        <>
          <label className="label">
            Arguments (one per line)
            <textarea className="field" rows={3} value={args} onChange={(e) => setArgs(e.target.value)} />
          </label>
          <div className="field-row">
            <label className="label">
              Working folder (optional)
              <input className="field" value={cwd} onChange={(e) => setCwd(e.target.value)} />
            </label>
            <button type="button" className="button" aria-label="Browse for a working folder" onClick={() => void browse("folder")}>
              Browse…
            </button>
          </div>
        </>
      )}
      <div className="actions">
        <button type="submit" className="button button--primary">
          Add
        </button>
        <button type="button" className="button" onClick={onCancel}>
          Cancel
        </button>
      </div>
    </form>
  );
}
