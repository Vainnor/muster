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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
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

/// Executes a plan. Not implemented yet: the per-OS spawners (`CreateProcessW`,
/// LaunchServices, `posix_spawn` via `std::process::Command`) land with the launch feature.
pub fn launch(_plan: &LaunchPlan) -> crate::Result<()> {
    Err(crate::Error::NotImplemented("launching"))
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

    #[test]
    fn launching_is_not_implemented_yet() {
        let plan = LaunchPlan::OpenUrl {
            url: Url::parse("https://example.com").unwrap(),
        };
        assert!(matches!(
            launch(&plan),
            Err(crate::Error::NotImplemented("launching"))
        ));
    }
}
