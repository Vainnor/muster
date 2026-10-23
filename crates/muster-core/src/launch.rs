//! What a launch target is, how it is validated, and the plan the shell executes.
//!
//! Targets are validated when they are saved and again right before launch
//! ([`plan`] always re-validates), because the filesystem can change in between.
//! A plan is a program plus an argument array: nothing here ever builds a shell string.

use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use url::Url;

/// URL schemes a URL entry may use. Anything else (`javascript:`, `file:`, custom handlers)
/// is rejected, because opening it would hand control to an arbitrary registered program.
pub const ALLOWED_URL_SCHEMES: &[&str] = &["http", "https", "mailto"];

/// Limits on stored arguments, to bound config size and catch garbage imports.
pub const MAX_ARGS: usize = 64;
pub const MAX_ARG_LEN: usize = 4096;
pub const MAX_URL_LEN: usize = 2048;

/// Something the user can launch.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum LaunchTarget {
    /// An executable or script, started directly with an argument array.
    Program {
        path: PathBuf,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default)]
        cwd: Option<PathBuf>,
    },
    /// A macOS `.app` bundle, opened through LaunchServices.
    AppBundle { path: PathBuf },
    /// A web or mail link, opened with the OS default handler.
    Url { url: String },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TargetError {
    #[error("path must be absolute")]
    RelativePath,
    #[error("path must not contain `..`")]
    ParentTraversal,
    #[error("path contains a NUL byte")]
    NulByte,
    #[error("path does not exist")]
    NotFound,
    #[error("path is not a file")]
    NotAFile,
    #[error("path is not an .app bundle")]
    NotABundle,
    #[error("working directory is not a directory")]
    BadWorkingDir,
    #[error("too many arguments (max {MAX_ARGS})")]
    TooManyArgs,
    #[error("an argument is longer than {MAX_ARG_LEN} bytes")]
    ArgTooLong,
    #[error("an argument to a batch file contains a character cmd.exe treats specially")]
    BatchArgument,
    #[error("a batch file's path contains a character cmd.exe treats specially")]
    BatchPath,
    #[error("program file name ends in a dot or space, or names a stream")]
    AmbiguousFileName,
    #[error("a file with the same name plus .exe exists and would run instead")]
    ShadowedByExe,
    #[error("URL is not valid")]
    BadUrl,
    #[error("URL scheme `{0}` is not allowed")]
    SchemeNotAllowed(String),
}

/// What the shell executes. Built only by [`plan`], so it is always validated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaunchPlan {
    Spawn {
        program: PathBuf,
        args: Vec<String>,
        cwd: Option<PathBuf>,
    },
    OpenBundle {
        path: PathBuf,
    },
    OpenUrl {
        url: Url,
    },
}

/// Checks a target against the current filesystem.
pub fn validate(target: &LaunchTarget) -> Result<(), TargetError> {
    match target {
        LaunchTarget::Program { path, args, cwd } => {
            check_path_shape(path)?;
            check_program_name(path)?;
            let meta = std::fs::metadata(path).map_err(|_| TargetError::NotFound)?;
            if !meta.is_file() {
                return Err(TargetError::NotAFile);
            }
            if args.len() > MAX_ARGS {
                return Err(TargetError::TooManyArgs);
            }
            if args.iter().any(|a| a.len() > MAX_ARG_LEN) {
                return Err(TargetError::ArgTooLong);
            }
            if args.iter().any(|a| a.contains('\0')) {
                return Err(TargetError::NulByte);
            }
            if is_batch_file(path) {
                if path.to_string_lossy().contains(BATCH_SPECIAL) {
                    return Err(TargetError::BatchPath);
                }
                if args.iter().any(|a| a.contains(BATCH_SPECIAL)) {
                    return Err(TargetError::BatchArgument);
                }
            }
            if let Some(dir) = cwd {
                check_path_shape(dir)?;
                if !dir.is_dir() {
                    return Err(TargetError::BadWorkingDir);
                }
            }
            Ok(())
        }
        LaunchTarget::AppBundle { path } => {
            check_path_shape(path)?;
            let is_app = path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("app"));
            if !path.exists() {
                return Err(TargetError::NotFound);
            }
            if !is_app || !path.is_dir() {
                return Err(TargetError::NotABundle);
            }
            Ok(())
        }
        LaunchTarget::Url { url } => parse_url(url).map(|_| ()),
    }
}

/// Characters `cmd.exe` acts on even inside quotes (`%` and `!` expand variables) or that end a
/// command (`&`, `|`, `<`, `>`, `^`, `"`, line breaks). Windows runs a `.bat` or `.cmd` file
/// through `cmd.exe`, which parses its own command line; Rust escapes batch arguments, but an
/// argument with none of these needs no escaping to mean what the approval dialog showed.
const BATCH_SPECIAL: &[char] = &['%', '!', '&', '|', '<', '>', '^', '"', '\r', '\n'];

/// A file Windows runs through `cmd.exe`. Rust's standard library decides with a
/// case-insensitive suffix test on the file name, so this does too: `.bat` alone counts, which
/// `Path::extension` would miss. Checked on every OS so the rule is tested everywhere.
fn is_batch_file(path: &Path) -> bool {
    path.file_name().is_some_and(|name| {
        let name = name.to_string_lossy();
        ends_with_ignore_case(&name, ".bat") || ends_with_ignore_case(&name, ".cmd")
    })
}

fn ends_with_ignore_case(name: &str, suffix: &str) -> bool {
    name.get(name.len().saturating_sub(suffix.len())..)
        .is_some_and(|end| end.len() == suffix.len() && end.eq_ignore_ascii_case(suffix))
}

/// Rejects program names Windows would read differently from the path that was validated and
/// shown: a trailing dot or space (Windows strips them, so `run.bat.` runs `run.bat`), a `:`
/// (an alternate data stream), and a path without `.exe` next to a file that adds it (Windows
/// runs `run.bat.exe` in place of `run.bat`). Checked on every OS for the same reason.
fn check_program_name(path: &Path) -> Result<(), TargetError> {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    if name.is_empty() || name.ends_with(['.', ' ']) || name.contains(':') {
        return Err(TargetError::AmbiguousFileName);
    }
    if !ends_with_ignore_case(&name, ".exe") {
        let mut with_exe = path.as_os_str().to_owned();
        with_exe.push(".exe");
        if Path::new(&with_exe).exists() {
            return Err(TargetError::ShadowedByExe);
        }
    }
    Ok(())
}

/// Re-validates `target` and turns it into a plan.
pub fn plan(target: &LaunchTarget) -> Result<LaunchPlan, TargetError> {
    validate(target)?;
    Ok(match target {
        LaunchTarget::Program { path, args, cwd } => LaunchPlan::Spawn {
            program: path.clone(),
            args: args.clone(),
            cwd: cwd.clone(),
        },
        LaunchTarget::AppBundle { path } => LaunchPlan::OpenBundle { path: path.clone() },
        LaunchTarget::Url { url } => LaunchPlan::OpenUrl {
            url: parse_url(url)?,
        },
    })
}

/// Starts a plan detached from Muster: no inherited stdio, its own process group on Unix, so
/// it keeps running after Muster exits and does not receive Muster's terminal signals.
///
/// Returns the child so the caller can reap it (`Child::wait` on a blocking thread) instead of
/// leaving a zombie or polling. Bundles and URLs go through the OS opener named by
/// [`opener`], always with an argument array.
pub fn launch(plan: &LaunchPlan) -> crate::Result<std::process::Child> {
    command(plan, opener().as_ref())?
        .spawn()
        .map_err(|source| crate::Error::Launch { source })
}

/// How the OS opens a bundle or URL: an absolute program and the arguments before the target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Opener {
    pub program: PathBuf,
    pub prefix: Vec<String>,
}

/// The opener for this OS. `None` on Linux when `xdg-open` is not installed.
#[must_use]
pub fn opener() -> Option<Opener> {
    #[cfg(target_os = "macos")]
    return Some(Opener {
        program: "/usr/bin/open".into(),
        prefix: Vec::new(),
    });
    #[cfg(target_os = "windows")]
    return std::env::var_os("SystemRoot")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .map(|root| Opener {
            program: root.join(r"System32\rundll32.exe"),
            prefix: vec!["url.dll,FileProtocolHandler".into()],
        });
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    crate::detect::find_in(&crate::detect::search_path(), "xdg-open").map(|program| Opener {
        program,
        prefix: Vec::new(),
    })
}

/// Builds the command for `plan` without running it.
pub fn command(plan: &LaunchPlan, opener: Option<&Opener>) -> crate::Result<std::process::Command> {
    use std::process::{Command, Stdio};

    let mut cmd = match plan {
        LaunchPlan::Spawn { program, args, cwd } => {
            let mut cmd = Command::new(program);
            cmd.args(args);
            if let Some(dir) = cwd {
                cmd.current_dir(dir);
            }
            cmd
        }
        LaunchPlan::OpenBundle { path } => {
            let opener = opener.ok_or(crate::Error::NoOpener)?;
            let mut cmd = Command::new(&opener.program);
            cmd.args(&opener.prefix).arg(path);
            cmd
        }
        LaunchPlan::OpenUrl { url } => {
            let opener = opener.ok_or(crate::Error::NoOpener)?;
            let mut cmd = Command::new(&opener.program);
            cmd.args(&opener.prefix).arg(url.as_str());
            cmd
        }
    };
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        /// `CREATE_NEW_PROCESS_GROUP`: Ctrl+C sent to Muster's group does not reach the app.
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        cmd.creation_flags(CREATE_NEW_PROCESS_GROUP);
    }
    Ok(cmd)
}

fn check_path_shape(path: &Path) -> Result<(), TargetError> {
    if path.as_os_str().as_encoded_bytes().contains(&0) {
        return Err(TargetError::NulByte);
    }
    if !path.is_absolute() {
        return Err(TargetError::RelativePath);
    }
    if path.components().any(|c| matches!(c, Component::ParentDir)) {
        return Err(TargetError::ParentTraversal);
    }
    Ok(())
}

fn parse_url(raw: &str) -> Result<Url, TargetError> {
    if raw.len() > MAX_URL_LEN || raw.chars().any(char::is_control) {
        return Err(TargetError::BadUrl);
    }
    let url = Url::parse(raw).map_err(|_| TargetError::BadUrl)?;
    if !ALLOWED_URL_SCHEMES.contains(&url.scheme()) {
        return Err(TargetError::SchemeNotAllowed(url.scheme().to_owned()));
    }
    if matches!(url.scheme(), "http" | "https") && url.host().is_none() {
        return Err(TargetError::BadUrl);
    }
    Ok(url)
}

/// A target as one line the user can read before approving it. Every path and argument is
/// quoted and escaped, so `["a b"]` and `["a", "b"]` look different and control characters
/// show as escapes.
#[must_use]
pub fn describe(target: &LaunchTarget) -> String {
    match target {
        LaunchTarget::Program { path, args, cwd } => {
            let words = std::iter::once(format!("{:?}", path.display().to_string()))
                .chain(args.iter().map(|a| format!("{a:?}")));
            let line = words.collect::<Vec<_>>().join(" ");
            match cwd {
                Some(cwd) => format!("{line} (in {:?})", cwd.display().to_string()),
                None => line,
            }
        }
        LaunchTarget::AppBundle { path } => format!("open {:?}", path.display().to_string()),
        // The parsed form is what the opener receives: an IDN host shows as punycode.
        LaunchTarget::Url { url } => match parse_url(url) {
            Ok(parsed) => format!("open {:?}", parsed.as_str()),
            Err(_) => format!("invalid link {url:?}"),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn program(path: PathBuf) -> LaunchTarget {
        LaunchTarget::Program {
            path,
            args: vec![],
            cwd: None,
        }
    }

    fn url(u: &str) -> LaunchTarget {
        LaunchTarget::Url { url: u.into() }
    }

    #[test]
    fn accepts_existing_absolute_file() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("tool");
        std::fs::write(&exe, b"").unwrap();
        assert_eq!(validate(&program(exe)), Ok(()));
    }

    fn program_with_args(path: PathBuf, args: &[&str]) -> LaunchTarget {
        LaunchTarget::Program {
            path,
            args: args.iter().map(|a| (*a).to_owned()).collect(),
            cwd: None,
        }
    }

    #[test]
    fn accepts_plain_arguments_to_a_batch_file() {
        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("start.bat");
        std::fs::write(&script, b"").unwrap();
        let target = program_with_args(script, &["--port", "8080", "C:\\My Files\\a b.txt"]);
        assert_eq!(validate(&target), Ok(()));
    }

    #[test]
    fn rejects_cmd_special_characters_in_batch_arguments() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["start.bat", "START.CMD"] {
            let script = dir.path().join(name);
            std::fs::write(&script, b"").unwrap();
            for arg in [
                "a&calc", "%PATH%", "!x!", "a|b", "a>b", "a<b", "a^b", "\"x", "a\nb",
            ] {
                assert_eq!(
                    validate(&program_with_args(script.clone(), &[arg])),
                    Err(TargetError::BatchArgument),
                    "{name} {arg:?}"
                );
            }
        }
    }

    #[test]
    fn treats_every_name_windows_runs_through_cmd_as_a_batch_file() {
        let dir = tempfile::tempdir().unwrap();
        for name in [".bat", ".CMD", "run.Bat"] {
            let script = dir.path().join(name);
            std::fs::write(&script, b"").unwrap();
            assert_eq!(
                validate(&program_with_args(script, &["a&b"])),
                Err(TargetError::BatchArgument),
                "{name}"
            );
        }
    }

    #[test]
    fn rejects_cmd_special_characters_in_a_batch_files_path() {
        let dir = tempfile::tempdir().unwrap();
        let odd = dir.path().join("x%CD:~0,0%y");
        std::fs::create_dir(&odd).unwrap();
        let script = odd.join("run.bat");
        std::fs::write(&script, b"").unwrap();
        assert_eq!(validate(&program(script)), Err(TargetError::BatchPath));
    }

    #[test]
    fn rejects_names_windows_would_read_differently() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["run.bat.", "run.bat ", "run.bat:stream"] {
            let path = dir.path().join(name);
            assert_eq!(
                validate(&program(path)),
                Err(TargetError::AmbiguousFileName),
                "{name:?}"
            );
        }
    }

    #[test]
    fn rejects_a_program_shadowed_by_an_exe_beside_it() {
        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("run.bat");
        std::fs::write(&script, b"").unwrap();
        assert_eq!(validate(&program(script.clone())), Ok(()));
        std::fs::write(dir.path().join("run.bat.exe"), b"").unwrap();
        assert_eq!(validate(&program(script)), Err(TargetError::ShadowedByExe));
    }

    #[test]
    fn other_programs_may_take_cmd_special_characters() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("tool.exe");
        std::fs::write(&exe, b"").unwrap();
        assert_eq!(validate(&program_with_args(exe, &["a&b", "100%"])), Ok(()));
    }

    #[test]
    fn rejects_relative_path() {
        assert_eq!(
            validate(&program("bin/tool".into())),
            Err(TargetError::RelativePath)
        );
    }

    #[test]
    fn rejects_parent_traversal_even_when_it_resolves() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("tool");
        std::fs::write(&exe, b"").unwrap();
        let sneaky = dir.path().join("sub").join("..").join("tool");
        assert_eq!(
            validate(&program(sneaky)),
            Err(TargetError::ParentTraversal)
        );
    }

    #[test]
    fn rejects_missing_file_and_directory() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            validate(&program(dir.path().join("nope"))),
            Err(TargetError::NotFound)
        );
        assert_eq!(
            validate(&program(dir.path().to_path_buf())),
            Err(TargetError::NotAFile)
        );
    }

    #[test]
    fn rejects_argument_limits_and_nul() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("tool");
        std::fs::write(&exe, b"").unwrap();
        let with = |args: Vec<String>| LaunchTarget::Program {
            path: exe.clone(),
            args,
            cwd: None,
        };
        assert_eq!(
            validate(&with(vec![String::new(); MAX_ARGS + 1])),
            Err(TargetError::TooManyArgs)
        );
        assert_eq!(
            validate(&with(vec!["x".repeat(MAX_ARG_LEN + 1)])),
            Err(TargetError::ArgTooLong)
        );
        assert_eq!(
            validate(&with(vec!["a\0b".into()])),
            Err(TargetError::NulByte)
        );
        assert_eq!(validate(&with(vec!["--flag".into(); MAX_ARGS])), Ok(()));
    }

    #[test]
    fn rejects_working_dir_that_is_a_file() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("tool");
        std::fs::write(&exe, b"").unwrap();
        let target = LaunchTarget::Program {
            path: exe.clone(),
            args: vec![],
            cwd: Some(exe),
        };
        assert_eq!(validate(&target), Err(TargetError::BadWorkingDir));
    }

    #[test]
    fn app_bundle_must_be_a_dot_app_directory() {
        let dir = tempfile::tempdir().unwrap();
        let bundle = dir.path().join("Editor.app");
        std::fs::create_dir(&bundle).unwrap();
        let plain = dir.path().join("Editor");
        std::fs::create_dir(&plain).unwrap();
        assert_eq!(validate(&LaunchTarget::AppBundle { path: bundle }), Ok(()));
        assert_eq!(
            validate(&LaunchTarget::AppBundle { path: plain }),
            Err(TargetError::NotABundle)
        );
    }

    #[test]
    fn allows_only_listed_url_schemes() {
        for ok in [
            "https://example.com",
            "http://localhost:8080/x",
            "mailto:a@b.c",
        ] {
            assert_eq!(validate(&url(ok)), Ok(()), "{ok}");
        }
        for (bad, scheme) in [
            ("javascript:alert(1)", "javascript"),
            ("file:///etc/passwd", "file"),
            ("vscode://open", "vscode"),
            ("data:text/html,hi", "data"),
        ] {
            assert_eq!(
                validate(&url(bad)),
                Err(TargetError::SchemeNotAllowed(scheme.into())),
                "{bad}"
            );
        }
    }

    #[test]
    fn rejects_malformed_urls() {
        for bad in [
            "not a url",
            "https://",
            "https://exa mple.com",
            "https://a.b/\n",
        ] {
            assert_eq!(validate(&url(bad)), Err(TargetError::BadUrl), "{bad:?}");
        }
        let long = format!("https://example.com/{}", "a".repeat(MAX_URL_LEN));
        assert_eq!(validate(&url(&long)), Err(TargetError::BadUrl));
    }

    #[test]
    fn plan_revalidates_at_launch_time() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("tool");
        std::fs::write(&exe, b"").unwrap();
        let target = program(exe.clone());
        assert!(matches!(plan(&target), Ok(LaunchPlan::Spawn { .. })));
        std::fs::remove_file(&exe).unwrap();
        assert_eq!(plan(&target), Err(TargetError::NotFound));
    }

    #[test]
    fn plan_keeps_arguments_as_an_array() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("tool");
        std::fs::write(&exe, b"").unwrap();
        let args = vec!["a b".to_owned(), "; rm -rf /".to_owned()];
        let target = LaunchTarget::Program {
            path: exe.clone(),
            args: args.clone(),
            cwd: None,
        };
        assert_eq!(
            plan(&target),
            Ok(LaunchPlan::Spawn {
                program: exe,
                args,
                cwd: None
            })
        );
    }

    fn opener() -> Opener {
        Opener {
            program: "/usr/bin/opener".into(),
            prefix: vec!["--flag".into()],
        }
    }

    fn parts(cmd: &std::process::Command) -> (String, Vec<String>) {
        let args = cmd
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        (cmd.get_program().to_string_lossy().into_owned(), args)
    }

    #[test]
    fn spawn_passes_arguments_verbatim_with_no_shell() {
        let plan = LaunchPlan::Spawn {
            program: "/opt/app".into(),
            args: vec!["a b".into(), "; rm -rf ~".into(), "$HOME".into()],
            cwd: Some("/tmp".into()),
        };
        let cmd = command(&plan, None).unwrap();
        assert_eq!(
            parts(&cmd),
            (
                "/opt/app".into(),
                vec!["a b".into(), "; rm -rf ~".into(), "$HOME".into()]
            )
        );
        assert_eq!(cmd.get_current_dir(), Some(Path::new("/tmp")));
    }

    #[test]
    fn bundles_and_urls_go_through_the_opener_as_one_argument() {
        let url = LaunchPlan::OpenUrl {
            url: Url::parse("https://example.com/a b?q=\"x\"").unwrap(),
        };
        assert_eq!(
            parts(&command(&url, Some(&opener())).unwrap()),
            (
                "/usr/bin/opener".into(),
                vec![
                    "--flag".into(),
                    "https://example.com/a%20b?q=%22x%22".into()
                ]
            )
        );
        let bundle = LaunchPlan::OpenBundle {
            path: "/Applications/A.app".into(),
        };
        assert_eq!(
            parts(&command(&bundle, Some(&opener())).unwrap()).1,
            ["--flag", "/Applications/A.app"]
        );
    }

    #[test]
    fn missing_opener_is_an_error_not_a_fallback() {
        let url = LaunchPlan::OpenUrl {
            url: Url::parse("https://example.com").unwrap(),
        };
        assert!(matches!(command(&url, None), Err(crate::Error::NoOpener)));
    }

    /// The one test that starts a real process (`.claude/rules/testing.md`).
    #[test]
    #[ignore = "spawns a process; run with --ignored"]
    fn smoke_spawns_and_reaps_a_harmless_program() {
        #[cfg(unix)]
        let target = program("/usr/bin/true".into());
        #[cfg(windows)]
        let target = LaunchTarget::Program {
            path: format!(r"{}\System32\cmd.exe", std::env::var("SystemRoot").unwrap()).into(),
            args: vec!["/c".into(), "exit".into(), "0".into()],
            cwd: None,
        };
        let mut child = launch(&plan(&target).unwrap()).unwrap();
        assert!(child.wait().unwrap().success());
    }

    #[test]
    fn describe_quotes_every_part() {
        let split = LaunchTarget::Program {
            path: "/bin/sh".into(),
            args: vec!["-c".into(), "a b".into()],
            cwd: Some("/tmp".into()),
        };
        assert_eq!(describe(&split), r#""/bin/sh" "-c" "a b" (in "/tmp")"#);
        let joined = LaunchTarget::Program {
            path: "/bin/sh".into(),
            args: vec!["-c".into(), "a".into(), "b\u{7}".into()],
            cwd: None,
        };
        assert_eq!(describe(&joined), r#""/bin/sh" "-c" "a" "b\u{7}""#);
        assert_eq!(
            describe(&LaunchTarget::Url {
                url: "https://a.example".into()
            }),
            r#"open "https://a.example/""#
        );
        // Cyrillic "а" in place of the Latin one: shown as the punycode host that opens.
        let lookalike = describe(&LaunchTarget::Url {
            url: "https://ex\u{430}mple.com".into(),
        });
        assert!(
            lookalike.starts_with(r#"open "https://xn--"#),
            "{lookalike}"
        );
    }
}
