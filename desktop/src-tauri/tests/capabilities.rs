//! A capability file that `tauri.conf.json` does not list is silently left out of the build,
//! so its window gets no permissions at all.

use std::fs;
use std::path::Path;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn json(path: &Path) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    Ok(serde_json::from_str(&fs::read_to_string(path)?)?)
}

#[test]
fn every_capability_file_is_enabled_in_the_config() -> TestResult {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let config = json(&root.join("tauri.conf.json"))?;
    let enabled: Vec<&str> = config["app"]["security"]["capabilities"]
        .as_array()
        .ok_or("app.security.capabilities is not a list")?
        .iter()
        .filter_map(serde_json::Value::as_str)
        .collect();

    let mut files = Vec::new();
    for entry in fs::read_dir(root.join("capabilities"))? {
        let path = entry?.path();
        if path.extension().is_some_and(|e| e == "json") {
            let capability = json(&path)?;
            let id = capability["identifier"]
                .as_str()
                .ok_or("capability without an identifier")?
                .to_owned();
            files.push(id);
        }
    }
    files.sort();

    assert_eq!(files, ["main", "popup"]);
    for id in &files {
        assert!(
            enabled.contains(&id.as_str()),
            "{id} is not in tauri.conf.json"
        );
    }
    Ok(())
}
