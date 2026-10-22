import { useRef, useState } from "react";

import { MAX_THEME_BYTES, describeWarning } from "../lib/contrast";
import * as ipc from "../lib/ipc";
import type { CssVar, ThemeInfo, ThemePreview } from "../lib/types";

interface Props {
  themes: ThemeInfo[];
  theme: string;
  onTheme: (id: string) => void;
  /** Applies tokens for a live preview; `null` returns to the chosen theme. */
  onPreview: (tokens: CssVar[] | null) => void;
  /** Reload the theme list after an import or delete. */
  onThemesChanged: () => Promise<void>;
  onError: (e: unknown) => void;
  onClose: () => void;
}

/** Presets and imported themes with live preview; import, export and delete. */
export function Settings({ themes, theme, onTheme, onPreview, onThemesChanged, onError, onClose }: Props) {
  const [pending, setPending] = useState<{ json: string; preview: ThemePreview } | null>(null);
  const [exported, setExported] = useState<{ name: string; json: string } | null>(null);
  const fileRef = useRef<HTMLInputElement>(null);

  const cancelImport = () => {
    setPending(null);
    onPreview(null);
  };

  const pickFile = async (file: File | undefined) => {
    if (fileRef.current) fileRef.current.value = "";
    if (!file) return;
    if (file.size > MAX_THEME_BYTES) {
      onError({ code: "invalid_theme", message: "That theme file is larger than 16 KiB." });
      return;
    }
    try {
      const json = await file.text();
      const preview = await ipc.previewTheme(json);
      setPending({ json, preview });
      onPreview(preview.tokens);
    } catch (e) {
      onError(e);
    }
  };

  const confirmImport = async () => {
    if (!pending) return;
    try {
      const saved = await ipc.importTheme(pending.json);
      setPending(null);
      onPreview(null);
      await onThemesChanged();
      onTheme(saved.theme.id);
    } catch (e) {
      onError(e);
    }
  };

  const showExport = async (t: { id: string; name: string }) => {
    try {
      setExported({ name: t.name, json: await ipc.exportTheme(t.id) });
    } catch (e) {
      onError(e);
    }
  };

  const remove = async (t: ThemeInfo) => {
    try {
      await ipc.deleteTheme(t.id);
      await onThemesChanged();
      if (theme === t.id) onTheme("system");
    } catch (e) {
      onError(e);
    }
  };

  const current = themes.find((t) => t.id === theme);

  return (
    <section
      className="panel"
      aria-labelledby="settings-title"
      onKeyDown={(e) => {
        if (e.key !== "Escape") return;
        if (pending) cancelImport();
        else onClose();
      }}
    >
      <h2 id="settings-title" className="panel__title">
        Settings
      </h2>

      {pending ? (
        <fieldset className="fieldset" aria-live="polite">
          <legend>Import “{pending.preview.theme.name}”</legend>
          <p className="muted">The window shows the theme now. Nothing is saved until you import it.</p>
          {pending.preview.warnings.length > 0 ? (
            <div className="warning" role="alert">
              <p>Some text in this theme may be hard to read:</p>
              <ul>
                {pending.preview.warnings.map((w) => (
                  <li key={`${w.foreground}-${w.background}`}>{describeWarning(w)}</li>
                ))}
              </ul>
              <p>You can still use it.</p>
            </div>
          ) : (
            <p className="muted">All text meets the WCAG AA contrast minimum.</p>
          )}
          <div className="actions">
            <button type="button" className="button button--primary" autoFocus onClick={() => void confirmImport()}>
              Import and use
            </button>
            <button type="button" className="button" onClick={cancelImport}>
              Cancel
            </button>
          </div>
        </fieldset>
      ) : (
        <fieldset className="fieldset">
          <legend>Theme</legend>
          <div className="themes" role="radiogroup" aria-label="Theme">
            {[{ id: "system", name: "Follow system", custom: false }, ...themes].map((t, i) => (
              <label key={t.id} className={t.id === theme ? "theme theme--current" : "theme"}>
                <input type="radio" name="theme" value={t.id} checked={t.id === theme} autoFocus={i === 0} onChange={() => onTheme(t.id)} />
                {t.name}
                {t.custom && <span className="theme__tag">imported</span>}
              </label>
            ))}
          </div>
          <div className="actions">
            <input
              ref={fileRef}
              className="visually-hidden"
              id="theme-file"
              type="file"
              accept=".json,application/json"
              onChange={(e) => void pickFile(e.target.files?.[0])}
            />
            <button type="button" className="button" onClick={() => fileRef.current?.click()}>
              Import a theme file…
            </button>
            {current && (
              <button type="button" className="button" onClick={() => void showExport(current)}>
                Export “{current.name}”
              </button>
            )}
            {current?.custom && (
              <button type="button" className="button button--danger" onClick={() => void remove(current)}>
                Delete “{current.name}”
              </button>
            )}
          </div>
        </fieldset>
      )}

      {exported && (
        <fieldset className="fieldset">
          <legend>“{exported.name}” as a theme file</legend>
          <p className="muted">Save this text as a .json file to share it or edit it, then import it again.</p>
          <textarea className="field" rows={10} readOnly value={exported.json} aria-label="Theme file" />
          <div className="actions">
            <button
              type="button"
              className="button"
              onClick={() => void navigator.clipboard.writeText(exported.json).catch(() => onError({ code: "internal", message: "Could not copy." }))}
            >
              Copy
            </button>
            <button type="button" className="button" onClick={() => setExported(null)}>
              Close
            </button>
          </div>
        </fieldset>
      )}

      <div className="actions">
        <button type="button" className="button" onClick={onClose}>
          Done
        </button>
      </div>
    </section>
  );
}
