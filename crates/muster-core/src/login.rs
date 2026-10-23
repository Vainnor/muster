//! Start on login: the entry each OS reads at login to start Muster.
//!
//! The shell says where the entry goes and which program it starts; this module builds the
//! entry with that format's own quoting, so a path with spaces, `%`, `&` or quotes starts the
//! right program or is refused. Each entry starts Muster with [`FLAG`], which keeps the main
//! window closed.

use std::fs;
use std::io::{self, Write as _};
use std::path::Path;

/// The argument every login entry passes.
pub const FLAG: &str = "--autostart";

/// The name shown for the entry where the OS shows one (Linux, Windows).
pub const NAME: &str = "Muster";

#[derive(Debug, thiserror::Error)]
pub enum LoginError {
    #[error("the program path is not absolute")]
    RelativeProgram,
    #[error("the program path is not valid Unicode or contains a control character")]
    UnusableProgram,
    #[error("the program is not a file that exists")]
    MissingProgram,
    /// macOS runs an unsigned app from a randomized read-only copy until the user moves it;
    /// an entry pointing there stops working after a restart.
    #[error("Muster is running from a temporary copy")]
    TemporaryLocation,
    #[error("the program path contains a character this OS's login entry cannot hold")]
    UnquotableProgram,
    #[error("the login entry could not be encoded")]
    Encode,
    /// The path stays out of the message: it is under the user's home.
    #[error("the login entry could not be written or removed")]
    Io(#[source] io::Error),
}

/// A program path that a login entry can hold: an existing file, absolute, Unicode, no control
/// characters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Program(String);

impl Program {
    pub fn new(path: &Path) -> Result<Self, LoginError> {
        if !path.is_absolute() {
            return Err(LoginError::RelativeProgram);
        }
        // Also catches Linux's `/usr/bin/muster (deleted)` after an upgrade.
        if !path.is_file() {
            return Err(LoginError::MissingProgram);
        }
        Self::from_text(path.to_str().ok_or(LoginError::UnusableProgram)?)
    }

    /// The text checks without `Path::is_absolute`, so every OS's format can be tested on any
    /// OS with that OS's paths.
    fn from_text(path: &str) -> Result<Self, LoginError> {
        if path.is_empty() || path.chars().any(char::is_control) {
            return Err(LoginError::UnusableProgram);
        }
        Ok(Self(path.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The program a login entry should start: this executable, or the `AppImage` it runs from. An
/// `AppImage` runs from a temporary mount, so its entry starts the `AppImage` file instead, but only
/// when `exe` is really inside that mount (`appdir`, from `APPDIR`): `APPIMAGE` alone is
/// inherited by anything an `AppImage` starts.
pub fn entry_program(
    exe: &Path,
    appimage: Option<&Path>,
    appdir: Option<&Path>,
) -> Result<Program, LoginError> {
    if let (Some(image), Some(dir)) = (appimage, appdir)
        && exe.starts_with(dir)
    {
        return Program::new(image);
    }
    if exe
        .components()
        .any(|c| c.as_os_str() == "AppTranslocation")
    {
        return Err(LoginError::TemporaryLocation);
    }
    Program::new(exe)
}

/// macOS: a `LaunchAgent` plist for `~/Library/LaunchAgents/<label>.plist`. The plist writer
/// escapes the XML; `AssociatedBundleIdentifiers` makes System Settings list it as Muster.
pub fn launch_agent(program: &Program, label: &str) -> Result<Vec<u8>, LoginError> {
    let mut dict = plist::Dictionary::new();
    dict.insert("Label".into(), label.into());
    dict.insert(
        "ProgramArguments".into(),
        plist::Value::Array(vec![program.as_str().into(), FLAG.into()]),
    );
    dict.insert("RunAtLoad".into(), true.into());
    dict.insert("ProcessType".into(), "Interactive".into());
    dict.insert(
        "AssociatedBundleIdentifiers".into(),
        plist::Value::Array(vec![label.into()]),
    );
    let mut out = Vec::new();
    plist::Value::Dictionary(dict)
        .to_writer_xml(&mut out)
        .map_err(|_| LoginError::Encode)?;
    Ok(out)
}

/// Linux: an XDG autostart entry for `$XDG_CONFIG_HOME/autostart/<id>.desktop`.
#[must_use]
pub fn desktop_entry(program: &Program) -> String {
    format!(
        "[Desktop Entry]\nType=Application\nName={NAME}\nExec={} {FLAG}\nTerminal=false\n\
         X-GNOME-Autostart-enabled=true\n",
        exec_quote(program.as_str())
    )
}

/// Quotes one `Exec` argument. The Desktop Entry spec escapes `"`, `` ` ``, `$` and `\` inside
/// quotes, writes `%` as `%%`, and then applies the string escapes on top, so a literal `\`
/// becomes four.
fn exec_quote(arg: &str) -> String {
    let mut out = String::with_capacity(arg.len() + 2);
    out.push('"');
    for c in arg.chars() {
        match c {
            '"' | '`' | '$' => {
                out.push_str("\\\\");
                out.push(c);
            }
            '\\' => out.push_str("\\\\\\\\"),
            '%' => out.push_str("%%"),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Windows: the value for `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`. The path is
/// quoted so Windows does not try each space-separated prefix (`C:\Program.exe`) first.
pub fn run_value(program: &Program) -> Result<String, LoginError> {
    if program.as_str().contains('"') {
        return Err(LoginError::UnquotableProgram);
    }
    Ok(format!("\"{}\" {FLAG}", program.as_str()))
}

/// Writes a login entry file atomically, creating its folder.
pub fn write_entry(path: &Path, contents: &[u8]) -> Result<(), LoginError> {
    let dir = path
        .parent()
        .ok_or(LoginError::Io(io::ErrorKind::NotFound.into()))?;
    fs::create_dir_all(dir).map_err(LoginError::Io)?;
    let mut tmp = tempfile::NamedTempFile::new_in(dir).map_err(LoginError::Io)?;
    tmp.write_all(contents).map_err(LoginError::Io)?;
    tmp.as_file().sync_all().map_err(LoginError::Io)?;
    tmp.persist(path).map_err(|e| LoginError::Io(e.error))?;
    Ok(())
}

/// Removes a login entry file. One that is already gone is fine.
pub fn remove_entry(path: &Path) -> Result<(), LoginError> {
    match fs::remove_file(path) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(LoginError::Io(e)),
        _ => Ok(()),
    }
}

/// Whether a login entry file exists.
pub fn entry_exists(path: &Path) -> Result<bool, LoginError> {
    path.try_exists().map_err(LoginError::Io)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::detect::linux::parse_desktop_entry;
    use crate::detect::windows::split_command_line;

    const AWKWARD: &str = r#"/home/some one/50% "odd" $HOME `x` \n & <b>/muster"#;

    fn program(path: &str) -> Program {
        Program::from_text(path).unwrap()
    }

    #[test]
    fn rejects_relative_and_control_character_paths() {
        assert!(matches!(
            Program::new(Path::new("muster")),
            Err(LoginError::RelativeProgram)
        ));
        assert!(matches!(
            Program::from_text("/opt/mu\nster"),
            Err(LoginError::UnusableProgram)
        ));
        assert!(matches!(
            Program::from_text(""),
            Err(LoginError::UnusableProgram)
        ));
    }

    #[test]
    fn accepts_an_absolute_path() {
        let exe = std::env::current_exe().unwrap();
        assert_eq!(Program::new(&exe).unwrap().as_str(), exe.to_str().unwrap());
    }

    #[test]
    fn rejects_a_program_that_is_not_a_file() {
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(
            Program::new(&dir.path().join("muster (deleted)")),
            Err(LoginError::MissingProgram)
        ));
        assert!(matches!(
            Program::new(dir.path()),
            Err(LoginError::MissingProgram)
        ));
    }

    /// A temp dir laid out as an `AppImage`: the file itself, and its mount holding the exe.
    fn appimage_layout() -> (
        tempfile::TempDir,
        std::path::PathBuf,
        std::path::PathBuf,
        std::path::PathBuf,
    ) {
        let dir = tempfile::tempdir().unwrap();
        let image = dir.path().join("Muster.AppImage");
        let mount = dir.path().join(".mount_Muster");
        let exe = mount.join("usr/bin/muster");
        fs::create_dir_all(exe.parent().unwrap()).unwrap();
        fs::write(&image, b"").unwrap();
        fs::write(&exe, b"").unwrap();
        (dir, image, mount, exe)
    }

    #[test]
    fn an_appimage_entry_starts_the_appimage() {
        let (_dir, image, mount, exe) = appimage_layout();
        let program = entry_program(&exe, Some(&image), Some(&mount)).unwrap();
        assert_eq!(program.as_str(), image.to_str().unwrap());
    }

    #[test]
    fn an_inherited_appimage_variable_is_ignored() {
        let (dir, image, mount, _exe) = appimage_layout();
        let installed = dir.path().join("muster");
        fs::write(&installed, b"").unwrap();
        // Started by some other AppImage: APPIMAGE and APPDIR are its, not Muster's.
        let program = entry_program(&installed, Some(&image), Some(&mount)).unwrap();
        assert_eq!(program.as_str(), installed.to_str().unwrap());
        let program = entry_program(&installed, Some(&image), None).unwrap();
        assert_eq!(program.as_str(), installed.to_str().unwrap());
    }

    #[test]
    fn a_missing_appimage_file_is_refused() {
        let (_dir, image, mount, exe) = appimage_layout();
        fs::remove_file(&image).unwrap();
        assert!(matches!(
            entry_program(&exe, Some(&image), Some(&mount)),
            Err(LoginError::MissingProgram)
        ));
    }

    #[test]
    fn a_translocated_app_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir
            .path()
            .join("AppTranslocation/ABC/d/Muster.app/Contents/MacOS/muster");
        fs::create_dir_all(exe.parent().unwrap()).unwrap();
        fs::write(&exe, b"").unwrap();
        assert!(matches!(
            entry_program(&exe, None, None),
            Err(LoginError::TemporaryLocation)
        ));
    }

    #[test]
    fn launch_agent_round_trips_an_awkward_path() {
        let bytes = launch_agent(&program(AWKWARD), "com.example.muster").unwrap();
        let value = plist::Value::from_reader_xml(bytes.as_slice()).unwrap();
        let dict = value.as_dictionary().unwrap();
        let args: Vec<&str> = dict["ProgramArguments"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_string().unwrap())
            .collect();
        assert_eq!(args, [AWKWARD, FLAG]);
        assert_eq!(dict["Label"].as_string(), Some("com.example.muster"));
        assert_eq!(dict["RunAtLoad"].as_boolean(), Some(true));
    }

    #[test]
    fn desktop_entry_round_trips_an_awkward_path_through_the_exec_parser() {
        let entry = parse_desktop_entry(&desktop_entry(&program(AWKWARD)))
            .unwrap()
            .unwrap();
        assert_eq!(entry.exec, [AWKWARD, FLAG]);
        assert_eq!(entry.name, NAME);
    }

    #[test]
    fn desktop_entry_quotes_a_plain_path() {
        assert!(
            desktop_entry(&program("/opt/Muster/muster"))
                .contains("\nExec=\"/opt/Muster/muster\" --autostart\n")
        );
    }

    #[test]
    fn run_value_quotes_a_path_with_spaces() {
        let path = r"C:\Program Files\Muster\muster.exe";
        let value = run_value(&program(path)).unwrap();
        assert_eq!(value, r#""C:\Program Files\Muster\muster.exe" --autostart"#);
        assert_eq!(split_command_line(&value).unwrap(), [path, FLAG]);
    }

    #[test]
    fn run_value_refuses_a_quote() {
        assert!(matches!(
            run_value(&program(r#"C:\a"b\muster.exe"#)),
            Err(LoginError::UnquotableProgram)
        ));
    }

    #[test]
    fn writes_checks_and_removes_an_entry() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir
            .path()
            .join("autostart")
            .join("com.example.muster.desktop");
        assert!(!entry_exists(&path).unwrap());
        write_entry(&path, b"[Desktop Entry]\n").unwrap();
        assert!(entry_exists(&path).unwrap());
        assert_eq!(fs::read(&path).unwrap(), b"[Desktop Entry]\n");
        assert_eq!(fs::read_dir(path.parent().unwrap()).unwrap().count(), 1);
        remove_entry(&path).unwrap();
        assert!(!entry_exists(&path).unwrap());
        remove_entry(&path).unwrap();
    }

    #[test]
    fn io_error_message_leaves_out_the_path() {
        let err = LoginError::Io(io::ErrorKind::PermissionDenied.into());
        assert_eq!(
            err.to_string(),
            "the login entry could not be written or removed"
        );
    }
}
