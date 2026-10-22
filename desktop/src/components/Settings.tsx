import type { ThemeInfo } from "../lib/types";

interface Props {
  themes: ThemeInfo[];
  theme: string;
  onTheme: (id: string) => void;
  onClose: () => void;
}

/** Presets with live preview: choosing one applies it at once. */
export function Settings({ themes, theme, onTheme, onClose }: Props) {
  return (
    <section
      className="panel"
      aria-labelledby="settings-title"
      onKeyDown={(e) => {
        if (e.key === "Escape") onClose();
      }}
    >
      <h2 id="settings-title" className="panel__title">
        Settings
      </h2>
      <fieldset className="fieldset">
        <legend>Theme</legend>
        <div className="themes" role="radiogroup" aria-label="Theme">
          {[{ id: "system", name: "Follow system" }, ...themes].map((t, i) => (
            <label key={t.id} className={t.id === theme ? "theme theme--current" : "theme"}>
              <input type="radio" name="theme" value={t.id} checked={t.id === theme} autoFocus={i === 0} onChange={() => onTheme(t.id)} />
              {t.name}
            </label>
          ))}
        </div>
      </fieldset>
      <div className="actions">
        <button type="button" className="button" onClick={onClose}>
          Done
        </button>
      </div>
    </section>
  );
}
