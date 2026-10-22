//! Detection against the fixture directories, through the public API.

use std::path::PathBuf;

use muster_core::detect::{linux, macos};
use muster_core::launch::LaunchTarget;

fn fixtures(os: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(os)
}

#[test]
fn linux_finds_visible_entries_and_tokenizes_exec() {
    let scan = linux::scan(&[fixtures("linux").join("applications")], &[]);
    let names: Vec<_> = scan.apps.iter().map(|a| a.name.as_str()).collect();
    assert_eq!(names, ["Editor", "Quoted"], "NoDisplay entries are skipped");
    assert_eq!(scan.skipped, 0);
    let editor = &scan.apps[0];
    assert_eq!(editor.key, "editor.desktop");
    assert_eq!(
        editor.target,
        LaunchTarget::Program {
            path: "/usr/bin/editor".into(),
            args: vec!["--new-window".into()],
            cwd: None,
        }
    );
    assert_eq!(
        scan.apps[1].target,
        LaunchTarget::Program {
            path: "/opt/My Tools/run".into(),
            args: vec!["--title".into(), "a b".into(), "%literal".into()],
            cwd: None,
        }
    );
}

#[test]
fn macos_finds_app_bundles() {
    let root = fixtures("macos").join("Applications");
    let scan = macos::scan(std::slice::from_ref(&root));
    assert_eq!(scan.apps.len(), 1);
    let app = &scan.apps[0];
    assert_eq!(app.name, "Editor");
    assert_eq!(app.key, "com.example.editor");
    assert_eq!(
        app.target,
        LaunchTarget::AppBundle {
            path: root.join("Editor.app")
        }
    );
}

#[test]
fn missing_roots_are_empty_not_errors() {
    let missing = fixtures("does-not-exist");
    assert_eq!(linux::scan(std::slice::from_ref(&missing), &[]).apps, []);
    assert_eq!(macos::scan(&[missing]).apps, []);
}

/// Parses a shortcut written by Windows itself (`WScript.Shell`), so the reader is checked
/// against the real format and not only against `windows::fixture`. Writes to a temp dir only.
#[cfg(windows)]
#[test]
fn windows_reads_a_shortcut_created_by_the_shell() {
    use muster_core::detect::windows;

    let dir = tempfile::tempdir().unwrap();
    let windir = std::env::var("windir").unwrap();
    let notepad = format!(r"{windir}\System32\notepad.exe");
    let link = dir.path().join("Note Pad.lnk");
    let script = format!(
        "$s = (New-Object -ComObject WScript.Shell).CreateShortcut('{}'); \
         $s.TargetPath = '{notepad}'; $s.Arguments = '/A \"two words\"'; \
         $s.WorkingDirectory = '{windir}'; $s.Save()",
        link.display()
    );
    let status = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .status()
        .unwrap();
    assert!(status.success());

    let scan = windows::scan(&[dir.path().to_owned()], &|name| std::env::var(name).ok());
    assert_eq!(scan.skipped, 0);
    assert_eq!(scan.apps.len(), 1);
    let app = &scan.apps[0];
    assert_eq!(app.name, "Note Pad");
    let LaunchTarget::Program { path, args, cwd } = &app.target else {
        panic!("expected a program, got {:?}", app.target);
    };
    assert!(
        path.to_string_lossy().eq_ignore_ascii_case(&notepad),
        "{path:?}"
    );
    assert_eq!(args, &["/A", "two words"]);
    assert!(
        cwd.as_ref()
            .is_some_and(|c| c.to_string_lossy().eq_ignore_ascii_case(&windir))
    );
    assert!(muster_core::launch::validate(&app.target).is_ok());
}
