//! macOS: `.app` bundles in `/Applications`, `/System/Applications` and `~/Applications`,
//! including one level of folders such as `/Applications/Utilities`.
//!
//! Each bundle's `Contents/Info.plist` (XML or binary) decides whether it is listed: it must
//! be an application (`CFBundlePackageType` absent or `APPL`) and not background-only
//! (`LSBackgroundOnly`). Menu-bar apps (`LSUIElement`) are listed; they are launchable. The
//! name is the bundle's file name without `.app`, which is what Finder shows. Launching goes
//! through LaunchServices with the bundle path (`LaunchTarget::AppBundle`).

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use super::{DetectedApp, MAX_APPS, Scan, clean_name, read_small, walk};
use crate::launch::LaunchTarget;

/// Bundles sit at the top of a root or one folder down; nothing deeper is an installed app.
const MAX_DEPTH: usize = 1;

#[must_use]
pub fn default_roots() -> Vec<PathBuf> {
    let mut roots = vec![
        PathBuf::from("/Applications"),
        PathBuf::from("/System/Applications"),
    ];
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from)
        && home.is_absolute()
    {
        roots.push(home.join("Applications"));
    }
    roots
}

/// Scans `roots` in order. The first bundle with a given identifier wins.
#[must_use]
pub fn scan(roots: &[PathBuf]) -> Scan {
    let mut seen = HashSet::new();
    let mut scan = Scan::default();
    for root in roots {
        walk(root, MAX_DEPTH, &mut |v| {
            if !v.is_dir {
                return false;
            }
            if !has_app_extension(v.path) {
                return true;
            }
            if scan.apps.len() >= MAX_APPS {
                return false;
            }
            match read_bundle(v.path) {
                Some(Bundle::App(app)) => {
                    if seen.insert(app.key.clone()) {
                        scan.apps.push(app);
                    }
                }
                Some(Bundle::NotListed) => {}
                None => scan.skipped += 1,
            }
            false
        });
    }
    scan
}

fn has_app_extension(path: &Path) -> bool {
    path.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("app"))
}

enum Bundle {
    App(DetectedApp),
    NotListed,
}

fn read_bundle(bundle: &Path) -> Option<Bundle> {
    let contents = bundle.join("Contents");
    let bytes = read_small(&contents.join("Info.plist"))?;
    let info = parse_info_plist(&bytes)?;
    if !info.listed {
        return Some(Bundle::NotListed);
    }
    let name = clean_name(bundle.file_stem()?.to_str()?)?;
    let key = info
        .identifier
        .unwrap_or_else(|| bundle.to_string_lossy().into_owned());
    let icon = info
        .icon_file
        .map(|file| {
            let resources = contents.join("Resources");
            if Path::new(&file).extension().is_some() {
                resources.join(file)
            } else {
                resources.join(format!("{file}.icns"))
            }
        })
        .filter(|p| p.is_file());
    Some(Bundle::App(DetectedApp {
        key,
        name,
        target: LaunchTarget::AppBundle {
            path: bundle.to_owned(),
        },
        icon,
    }))
}

/// What Muster reads from an `Info.plist`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InfoPlist {
    pub identifier: Option<String>,
    /// A bare file name inside `Contents/Resources`, never a path.
    pub icon_file: Option<String>,
    /// An application that is not background-only.
    pub listed: bool,
}

/// Parses an XML or binary `Info.plist`. `None` if it is not a plist dictionary.
#[must_use]
pub fn parse_info_plist(bytes: &[u8]) -> Option<InfoPlist> {
    let value = plist::Value::from_reader(std::io::Cursor::new(bytes)).ok()?;
    let dict = value.as_dictionary()?;
    let string = |key: &str| dict.get(key).and_then(plist::Value::as_string);
    let flag = |key: &str| match dict.get(key) {
        Some(plist::Value::Boolean(b)) => *b,
        // Older bundles store these flags as strings or integers.
        Some(plist::Value::String(s)) => matches!(s.as_str(), "1" | "YES" | "true"),
        Some(plist::Value::Integer(i)) => i.as_signed().is_some_and(|i| i != 0),
        _ => false,
    };
    let is_app = string("CFBundlePackageType").is_none_or(|t| t == "APPL");
    let identifier = string("CFBundleIdentifier")
        .filter(|id| !id.is_empty() && id.len() <= 255 && !id.chars().any(char::is_control))
        .map(str::to_owned);
    let icon_file = string("CFBundleIconFile")
        .filter(|f| !f.is_empty() && !f.contains('/') && !f.starts_with('.'))
        .map(str::to_owned);
    Some(InfoPlist {
        identifier,
        icon_file,
        listed: is_app && !flag("LSBackgroundOnly"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn xml(body: &str) -> Vec<u8> {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><plist version="1.0"><dict>{body}</dict></plist>"#
        )
        .into_bytes()
    }

    #[test]
    fn reads_identifier_and_icon() {
        let info = parse_info_plist(&xml(
            "<key>CFBundleIdentifier</key><string>com.example.app</string>\
             <key>CFBundleIconFile</key><string>AppIcon</string>",
        ))
        .unwrap();
        assert_eq!(info.identifier.as_deref(), Some("com.example.app"));
        assert_eq!(info.icon_file.as_deref(), Some("AppIcon"));
        assert!(info.listed);
    }

    #[test]
    fn reads_binary_plists() {
        let mut dict = plist::Dictionary::new();
        dict.insert("CFBundleIdentifier".into(), "com.example.bin".into());
        let mut bytes = Vec::new();
        plist::Value::Dictionary(dict)
            .to_writer_binary(&mut bytes)
            .unwrap();
        assert_eq!(
            parse_info_plist(&bytes).unwrap().identifier.as_deref(),
            Some("com.example.bin")
        );
    }

    #[test]
    fn background_only_and_non_app_bundles_are_not_listed() {
        for body in [
            "<key>LSBackgroundOnly</key><true/>",
            "<key>LSBackgroundOnly</key><string>1</string>",
            "<key>CFBundlePackageType</key><string>BNDL</string>",
        ] {
            assert!(!parse_info_plist(&xml(body)).unwrap().listed, "{body}");
        }
        assert!(
            parse_info_plist(&xml("<key>LSUIElement</key><true/>"))
                .unwrap()
                .listed
        );
    }

    #[test]
    fn icon_file_cannot_be_a_path() {
        let info = parse_info_plist(&xml("<key>CFBundleIconFile</key><string>../../x</string>"));
        assert_eq!(info.unwrap().icon_file, None);
    }

    #[test]
    fn rejects_non_plists() {
        assert_eq!(parse_info_plist(b"not a plist"), None);
        assert_eq!(
            parse_info_plist(br#"<plist version="1.0"><string>x</string></plist>"#),
            None
        );
    }

    fn bundle(dir: &Path, name: &str, body: &str) {
        let contents = dir.join(name).join("Contents");
        std::fs::create_dir_all(&contents).unwrap();
        std::fs::write(contents.join("Info.plist"), xml(body)).unwrap();
    }

    #[test]
    fn scans_one_folder_deep_and_dedupes_by_identifier() {
        let tmp = tempfile::tempdir().unwrap();
        let system = tmp.path().join("System");
        let user = tmp.path().join("User");
        let id = |id: &str| format!("<key>CFBundleIdentifier</key><string>{id}</string>");
        bundle(&system, "Mail.app", &id("com.example.mail"));
        bundle(
            &system,
            "Utilities/Terminal.app",
            &id("com.example.terminal"),
        );
        bundle(&system, "Agent.app", "<key>LSBackgroundOnly</key><true/>");
        bundle(&system, "Deep/Deeper/Hidden.app", &id("com.example.hidden"));
        bundle(&user, "Mail Copy.app", &id("com.example.mail"));
        std::fs::create_dir_all(system.join("Broken.app")).unwrap();

        let scan = scan(&[system, user]);
        let names: Vec<_> = scan.apps.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, ["Mail", "Terminal"]);
        assert_eq!(scan.skipped, 1, "Broken.app has no Info.plist");
    }
}
