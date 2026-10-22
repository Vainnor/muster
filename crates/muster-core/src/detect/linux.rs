//! Linux: freedesktop `.desktop` entries under `$XDG_DATA_HOME/applications` and
//! `$XDG_DATA_DIRS/applications`.
//!
//! Follows the Desktop Entry Specification 1.5: only the `[Desktop Entry]` group, unlocalized
//! keys, string escapes, then `Exec` quoting and field codes. The `Exec` line becomes an
//! argument array and is never handed to a shell. Field codes: `%%` is `%`; the file and URL
//! codes (`%f %F %u %U`) and `%i %c %k` are dropped because Muster launches apps without
//! files; the deprecated `%d %D %n %N %v %m` are dropped; any other code rejects the entry.
//!
//! Entries are skipped when `Type` is not `Application`, `Hidden` or `NoDisplay` is true,
//! `Terminal` is true (Muster does not open a terminal for them), `TryExec` names a program
//! that is not installed, or a bare `Exec` program is not on the search path.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use super::{DetectedApp, MAX_APPS, Scan, clean_name, find_in, read_small, walk};
use crate::launch::{LaunchTarget, MAX_ARG_LEN, MAX_ARGS};

/// `applications/` subdirectories are rare; vendors nest at most one or two levels.
const MAX_DEPTH: usize = 3;

/// Why one `.desktop` file was rejected.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EntryError {
    #[error("no [Desktop Entry] group")]
    NoEntryGroup,
    #[error("missing required key {0}")]
    Missing(&'static str),
    #[error("unknown escape sequence")]
    BadEscape,
    #[error("unterminated quote in Exec")]
    UnterminatedQuote,
    #[error("unknown field code %{0} in Exec")]
    UnknownFieldCode(char),
    #[error("Exec has too many or too long arguments")]
    TooLong,
    #[error("Exec program is a relative path")]
    RelativeProgram,
    #[error("Name is empty, too long or has control characters")]
    BadName,
}

/// The application directories from the XDG base-directory spec, highest priority first.
#[must_use]
pub fn default_roots() -> Vec<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let data_home = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| home.map(|h| h.join(".local/share")));
    let data_dirs = std::env::var_os("XDG_DATA_DIRS")
        .filter(|v| !v.is_empty())
        .map_or_else(
            || vec!["/usr/local/share".into(), "/usr/share".into()],
            |v| std::env::split_paths(&v).collect::<Vec<_>>(),
        );
    data_home
        .into_iter()
        .chain(data_dirs)
        .filter(|d| d.is_absolute())
        .map(|d| d.join("applications"))
        .collect()
}

/// Scans `roots` (highest priority first). The first file with a given desktop-file id wins,
/// even when it hides the app, so `~/.local/share/applications/x.desktop` with `Hidden=true`
/// removes the system entry. Bare `Exec` programs are resolved against `search_path`.
#[must_use]
pub fn scan(roots: &[PathBuf], search_path: &[PathBuf]) -> Scan {
    let mut seen = HashSet::new();
    let mut scan = Scan::default();
    for root in roots {
        walk(root, MAX_DEPTH, &mut |v| {
            if v.is_dir {
                return true;
            }
            let Some(stem) = v.relative.strip_suffix(".desktop") else {
                return false;
            };
            let id = format!("{}.desktop", stem.replace('/', "-"));
            if !seen.insert(id.clone()) || scan.apps.len() >= MAX_APPS {
                return false;
            }
            let parsed = read_small(v.path)
                .and_then(|bytes| String::from_utf8(bytes).ok())
                .map(|text| parse_desktop_entry(&text));
            match parsed {
                Some(Ok(Some(entry))) => match entry.into_app(id, roots, search_path) {
                    Some(app) => scan.apps.push(app),
                    None => scan.skipped += 1,
                },
                Some(Ok(None)) => {}
                Some(Err(_)) | None => scan.skipped += 1,
            }
            false
        });
    }
    scan
}

/// A parsed `.desktop` file that should be shown. Not yet resolved against the filesystem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopEntry {
    pub name: String,
    /// `Exec` after field-code removal: the program, then its arguments.
    pub exec: Vec<String>,
    pub try_exec: Option<String>,
    pub path: Option<String>,
    pub icon: Option<String>,
}

impl DesktopEntry {
    fn into_app(
        self,
        key: String,
        roots: &[PathBuf],
        search_path: &[PathBuf],
    ) -> Option<DetectedApp> {
        if let Some(try_exec) = &self.try_exec {
            resolve_program(try_exec, search_path)?;
        }
        let mut exec = self.exec.into_iter();
        let program = resolve_program(&exec.next()?, search_path)?;
        let cwd = self.path.map(PathBuf::from).filter(|p| p.is_absolute());
        let icon = self.icon.and_then(|icon| find_icon(&icon, roots));
        Some(DetectedApp {
            key,
            name: self.name,
            target: LaunchTarget::Program {
                path: program,
                args: exec.collect(),
                cwd,
            },
            icon,
        })
    }
}

/// An absolute program is taken as written (`launch::validate` checks it exists before it is
/// saved or run). A bare name is looked up in `search_path` and the entry is skipped if absent,
/// which drops entries left behind by uninstalled packages.
fn resolve_program(program: &str, search_path: &[PathBuf]) -> Option<PathBuf> {
    let path = Path::new(program);
    if path.is_absolute() {
        Some(path.to_owned())
    } else {
        find_in(search_path, program)
    }
}

/// Parses one `.desktop` file. `Ok(None)` means a valid entry that should not be listed.
pub fn parse_desktop_entry(contents: &str) -> Result<Option<DesktopEntry>, EntryError> {
    let mut in_entry = false;
    let mut found_entry = false;
    let mut keys: Vec<(&str, &str)> = Vec::new();
    for line in contents.lines() {
        let line = line.trim_start();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') {
            if found_entry {
                break;
            }
            in_entry = line.trim_end() == "[Desktop Entry]";
            found_entry = in_entry;
            continue;
        }
        if !in_entry {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            keys.push((key.trim_end(), value.trim_start()));
        }
    }
    if !found_entry {
        return Err(EntryError::NoEntryGroup);
    }
    // Later duplicates of a key are invalid per spec; the first one wins.
    let get = |name: &str| keys.iter().find(|(k, _)| *k == name).map(|(_, v)| *v);
    let flag = |name: &str| get(name).is_some_and(|v| v.trim() == "true");

    if get("Type").map(str::trim) != Some("Application") {
        return Ok(None);
    }
    if flag("Hidden") || flag("NoDisplay") || flag("Terminal") {
        return Ok(None);
    }
    let name = unescape(get("Name").ok_or(EntryError::Missing("Name"))?)?;
    let name = clean_name(&name).ok_or(EntryError::BadName)?;
    let exec = tokenize_exec(&unescape(get("Exec").ok_or(EntryError::Missing("Exec"))?)?)?;
    let first = exec.first().ok_or(EntryError::Missing("Exec"))?;
    if !Path::new(first).is_absolute() && first.contains('/') {
        return Err(EntryError::RelativeProgram);
    }
    let string = |name: &str| -> Result<Option<String>, EntryError> {
        get(name)
            .map(unescape)
            .transpose()
            .map(|v| v.filter(|s| !s.is_empty()))
    };
    Ok(Some(DesktopEntry {
        name,
        exec,
        try_exec: string("TryExec")?,
        path: string("Path")?,
        icon: string("Icon")?,
    }))
}

/// The spec's string escapes: `\s`, `\n`, `\t`, `\r`, `\\`. Any other escape is an error.
fn unescape(value: &str) -> Result<String, EntryError> {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('s') => out.push(' '),
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('\\') => out.push('\\'),
            // `Exec` quoting has its own escapes (`\"`, `\$`, `` \` ``); keep them for the
            // tokenizer.
            Some(other @ ('"' | '$' | '`')) => {
                out.push('\\');
                out.push(other);
            }
            _ => return Err(EntryError::BadEscape),
        }
    }
    Ok(out)
}

/// Splits an unescaped `Exec` value into arguments and removes field codes.
pub fn tokenize_exec(exec: &str) -> Result<Vec<String>, EntryError> {
    let mut args = Vec::new();
    let mut current = String::new();
    // Whether the current argument exists even if empty (`""` is an argument).
    let mut started = false;
    let mut chars = exec.chars();
    while let Some(c) = chars.next() {
        match c {
            ' ' | '\t' | '\n' => {
                if started {
                    args.push(std::mem::take(&mut current));
                    started = false;
                }
            }
            '"' => {
                started = true;
                loop {
                    match chars.next() {
                        None => return Err(EntryError::UnterminatedQuote),
                        Some('"') => break,
                        Some('\\') => match chars.next() {
                            Some(e @ ('"' | '`' | '$' | '\\')) => current.push(e),
                            _ => return Err(EntryError::BadEscape),
                        },
                        Some('%') => field_code(chars.next(), &mut current)?,
                        Some(other) => current.push(other),
                    }
                }
            }
            '%' => {
                let code = chars.next();
                if code == Some('%') {
                    started = true;
                }
                field_code(code, &mut current)?;
            }
            other => {
                started = true;
                current.push(other);
            }
        }
        if args.len() > MAX_ARGS || current.len() > MAX_ARG_LEN {
            return Err(EntryError::TooLong);
        }
    }
    if started {
        args.push(current);
    }
    if args.len() > MAX_ARGS + 1 {
        return Err(EntryError::TooLong);
    }
    Ok(args)
}

fn field_code(code: Option<char>, out: &mut String) -> Result<(), EntryError> {
    match code {
        Some('%') => out.push('%'),
        Some('f' | 'F' | 'u' | 'U' | 'i' | 'c' | 'k' | 'd' | 'D' | 'n' | 'N' | 'v' | 'm') => {}
        Some(other) => return Err(EntryError::UnknownFieldCode(other)),
        None => return Err(EntryError::UnknownFieldCode(' ')),
    }
    Ok(())
}

/// `hicolor` sizes tried, largest first; `scalable` holds SVGs.
const ICON_SIZES: [&str; 5] = ["scalable", "256x256", "128x128", "64x64", "48x48"];

/// A small subset of the icon theme spec: an absolute path, or a name looked up in the
/// `hicolor` fallback theme and `pixmaps` beside each root's `applications` directory.
/// Theme-specific icons are not resolved.
fn find_icon(icon: &str, roots: &[PathBuf]) -> Option<PathBuf> {
    let path = Path::new(icon);
    if path.is_absolute() {
        return path.is_file().then(|| path.to_owned());
    }
    if icon.contains(['/', '\\']) || icon.starts_with('.') {
        return None;
    }
    let has_ext = Path::new(icon)
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("png") || e.eq_ignore_ascii_case("svg"));
    let names: Vec<String> = if has_ext {
        vec![icon.to_owned()]
    } else {
        vec![format!("{icon}.svg"), format!("{icon}.png")]
    };
    roots
        .iter()
        .filter_map(|root| root.parent())
        .flat_map(|share| {
            let themed = ICON_SIZES
                .iter()
                .map(move |size| share.join("icons/hicolor").join(size).join("apps"));
            themed.chain(std::iter::once(share.join("pixmaps")))
        })
        .flat_map(|dir| names.iter().map(move |n| dir.join(n)))
        .find(|p| p.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(body: &str) -> Result<Option<DesktopEntry>, EntryError> {
        parse_desktop_entry(&format!("[Desktop Entry]\nType=Application\n{body}"))
    }

    fn exec(s: &str) -> Vec<String> {
        tokenize_exec(s).unwrap()
    }

    #[test]
    fn splits_on_whitespace_and_keeps_quoted_spaces() {
        assert_eq!(exec("app  --a\tb"), ["app", "--a", "b"]);
        assert_eq!(
            exec(r#""/opt/My App/run" "a b""#),
            ["/opt/My App/run", "a b"]
        );
        assert_eq!(
            exec(r#"app """#),
            ["app", ""],
            "an empty quoted argument is kept"
        );
    }

    #[test]
    fn quoted_escapes() {
        assert_eq!(
            exec(r#"app "say \"hi\" \$HOME \` \\""#),
            ["app", r#"say "hi" $HOME ` \"#]
        );
        assert_eq!(tokenize_exec(r#"app "\q""#), Err(EntryError::BadEscape));
        assert_eq!(
            tokenize_exec(r#"app "open"#),
            Err(EntryError::UnterminatedQuote)
        );
    }

    #[test]
    fn field_codes_are_dropped_or_rejected() {
        assert_eq!(exec("app %F"), ["app"]);
        assert_eq!(exec("app --new %u %i %c %k"), ["app", "--new"]);
        assert_eq!(exec("app --file=%f"), ["app", "--file="]);
        assert_eq!(exec("app %%literal 100%%"), ["app", "%literal", "100%"]);
        assert_eq!(
            tokenize_exec("app %z"),
            Err(EntryError::UnknownFieldCode('z'))
        );
        assert_eq!(
            tokenize_exec("app %"),
            Err(EntryError::UnknownFieldCode(' '))
        );
    }

    #[test]
    fn shell_syntax_is_literal_text() {
        assert_eq!(exec("app ; rm -rf ~"), ["app", ";", "rm", "-rf", "~"]);
        assert_eq!(
            exec("sh -c \"echo $HOME | cat\""),
            ["sh", "-c", "echo $HOME | cat"]
        );
    }

    #[test]
    fn rejects_too_many_arguments() {
        let many = format!("app{}", " x".repeat(MAX_ARGS + 1));
        assert_eq!(tokenize_exec(&many), Err(EntryError::TooLong));
        assert_eq!(
            exec(&format!("app{}", " x".repeat(MAX_ARGS))).len(),
            MAX_ARGS + 1
        );
    }

    #[test]
    fn string_escapes_apply_before_exec_quoting() {
        let e = entry("Name=Tab\\tName\nExec=app a\\sb");
        assert_eq!(
            e,
            Err(EntryError::BadName),
            "control characters in Name are rejected"
        );
        // A literal backslash in a quoted argument is four in the file: the string escape
        // halves them, then Exec quoting halves them again.
        let e = entry("Name=Spaced\nExec=\"/opt/x\\\\\\\\y/app\" a\\sb")
            .unwrap()
            .unwrap();
        assert_eq!(
            e.exec,
            ["/opt/x\\y/app", "a", "b"],
            "\\s is a separator after unescaping"
        );
        assert_eq!(entry("Name=A\nExec=app \\q"), Err(EntryError::BadEscape));
    }

    #[test]
    fn hidden_nodisplay_terminal_and_other_types_are_not_listed() {
        for body in ["Hidden=true", "NoDisplay=true", "Terminal=true"] {
            assert_eq!(
                entry(&format!("Name=A\nExec=app\n{body}")),
                Ok(None),
                "{body}"
            );
        }
        let link = parse_desktop_entry("[Desktop Entry]\nType=Link\nName=A\nURL=https://x");
        assert_eq!(link, Ok(None));
        assert!(
            entry("Name=A\nExec=app\nNoDisplay=false")
                .unwrap()
                .is_some()
        );
    }

    #[test]
    fn requires_name_and_exec() {
        assert_eq!(entry("Exec=app"), Err(EntryError::Missing("Name")));
        assert_eq!(entry("Name=A"), Err(EntryError::Missing("Exec")));
        assert_eq!(entry("Name=A\nExec=%f"), Err(EntryError::Missing("Exec")));
        assert_eq!(parse_desktop_entry("Name=A"), Err(EntryError::NoEntryGroup));
    }

    #[test]
    fn rejects_relative_program_paths() {
        assert_eq!(
            entry("Name=A\nExec=bin/app"),
            Err(EntryError::RelativeProgram)
        );
    }

    #[test]
    fn reads_only_the_desktop_entry_group_and_unlocalized_keys() {
        let text = "# c\n[Desktop Entry]\nType=Application\nName[de]=Bearbeiter\nName = Editor\n\
                    Exec=app\n[Desktop Action new]\nName=New Window\nExec=app --new\n";
        let e = parse_desktop_entry(text).unwrap().unwrap();
        assert_eq!(e.name, "Editor");
        assert_eq!(e.exec, ["app"]);
    }

    #[cfg(unix)]
    fn executable(path: &Path) {
        use std::os::unix::fs::PermissionsExt;
        std::fs::write(path, "").unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn scan_resolves_bare_programs_and_skips_missing_ones() {
        let tmp = tempfile::tempdir().unwrap();
        let apps = tmp.path().join("share/applications");
        let bin = tmp.path().join("bin");
        std::fs::create_dir_all(&apps).unwrap();
        std::fs::create_dir_all(&bin).unwrap();
        executable(&bin.join("present"));
        std::fs::write(
            apps.join("a.desktop"),
            "[Desktop Entry]\nType=Application\nName=A\nExec=present %U\n",
        )
        .unwrap();
        std::fs::write(
            apps.join("b.desktop"),
            "[Desktop Entry]\nType=Application\nName=B\nExec=absent\n",
        )
        .unwrap();
        std::fs::write(
            apps.join("c.desktop"),
            "[Desktop Entry]\nType=Application\nName=C\nExec=present\nTryExec=absent\n",
        )
        .unwrap();
        let scan = scan(&[apps], std::slice::from_ref(&bin));
        assert_eq!(scan.skipped, 2);
        assert_eq!(scan.apps.len(), 1);
        assert_eq!(
            scan.apps[0].target,
            LaunchTarget::Program {
                path: bin.join("present"),
                args: vec![],
                cwd: None
            }
        );
    }

    #[test]
    fn earlier_root_wins_and_can_hide_an_app() {
        let tmp = tempfile::tempdir().unwrap();
        let user = tmp.path().join("user/applications");
        let system = tmp.path().join("system/applications");
        std::fs::create_dir_all(user.join("vendor")).unwrap();
        std::fs::create_dir_all(system.join("vendor")).unwrap();
        let app = |name: &str, extra: &str| {
            format!("[Desktop Entry]\nType=Application\nName={name}\nExec=/usr/bin/{name}\n{extra}")
        };
        std::fs::write(user.join("hidden.desktop"), app("Mine", "Hidden=true")).unwrap();
        std::fs::write(system.join("hidden.desktop"), app("Theirs", "")).unwrap();
        std::fs::write(user.join("vendor/tool.desktop"), app("UserTool", "")).unwrap();
        std::fs::write(system.join("vendor/tool.desktop"), app("SystemTool", "")).unwrap();
        std::fs::write(system.join("other.desktop"), app("Other", "")).unwrap();

        let scan = scan(&[user, system], &[]);
        let found: Vec<_> = scan
            .apps
            .iter()
            .map(|a| (a.key.as_str(), a.name.as_str()))
            .collect();
        assert_eq!(
            found,
            [
                ("vendor-tool.desktop", "UserTool"),
                ("other.desktop", "Other")
            ]
        );
    }

    #[test]
    fn finds_hicolor_and_pixmap_icons_beside_the_root() {
        let tmp = tempfile::tempdir().unwrap();
        let share = tmp.path().join("share");
        let apps = share.join("applications");
        std::fs::create_dir_all(&apps).unwrap();
        std::fs::create_dir_all(share.join("icons/hicolor/48x48/apps")).unwrap();
        std::fs::create_dir_all(share.join("pixmaps")).unwrap();
        std::fs::write(share.join("icons/hicolor/48x48/apps/editor.png"), "").unwrap();
        std::fs::write(share.join("pixmaps/old.svg"), "").unwrap();
        let roots = [apps];
        assert_eq!(
            find_icon("editor", &roots),
            Some(share.join("icons/hicolor/48x48/apps/editor.png"))
        );
        assert_eq!(
            find_icon("old", &roots),
            Some(share.join("pixmaps/old.svg"))
        );
        assert_eq!(find_icon("missing", &roots), None);
        assert_eq!(find_icon("../../etc/passwd", &roots), None);
    }
}
