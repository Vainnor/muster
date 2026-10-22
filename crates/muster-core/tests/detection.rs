//! Detection against fixture directories. Ignored until each OS scanner is implemented; the
//! assertions are the acceptance criteria.

use std::path::PathBuf;

use muster_core::detect::{linux, macos};
use muster_core::launch::LaunchTarget;

fn fixtures(os: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(os)
}

#[test]
#[ignore = "linux detection not implemented"]
fn linux_finds_visible_entries_and_tokenizes_exec() {
    let mut apps = linux::scan(&[fixtures("linux").join("applications")]).unwrap();
    apps.sort_by(|a, b| a.name.cmp(&b.name));
    let names: Vec<_> = apps.iter().map(|a| a.name.as_str()).collect();
    assert_eq!(names, ["Editor", "Quoted"], "NoDisplay entries are skipped");
    let quoted = apps.iter().find(|a| a.name == "Quoted").unwrap();
    assert_eq!(
        quoted.target,
        LaunchTarget::Program {
            path: "/opt/My Tools/run".into(),
            args: vec!["--title".into(), "a b".into(), "%literal".into()],
            cwd: None,
        }
    );
}

#[test]
#[ignore = "macos detection not implemented"]
fn macos_finds_app_bundles() {
    let apps = macos::scan(&[fixtures("macos").join("Applications")]).unwrap();
    assert_eq!(apps.len(), 1);
    assert_eq!(apps[0].name, "Editor");
    assert!(matches!(apps[0].target, LaunchTarget::AppBundle { .. }));
}

#[test]
fn scanners_report_not_implemented_instead_of_an_empty_list() {
    let err = linux::scan(&[fixtures("linux")]).unwrap_err();
    assert_eq!(err.code(), "not_implemented");
}
