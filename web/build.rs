//! Compiles Tailwind with the standalone CLI (no Node). The CLI download is pinned and its
//! checksum verified, per host platform.

const TAILWIND_VERSION: &str = "4.3.2";

/// sha256 of each `tailwindcss-<platform>` asset in the v4.3.2 release (`sha256sums.txt`).
fn checksum() -> Option<&'static str> {
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    Some(match (os, arch) {
        ("macos", "aarch64") => "b800b0659dc64b9f03ede5660244d9415d777d5739ae2889280877ca37be742a",
        ("macos", "x86_64") => "cef8f110471e889c3c4409055cf8aff33076f58a081867b0dfc6534b290bfbb0",
        ("linux", "x86_64") => "5036c4fb4328e0bcdbb6065c70d8ac9452e0d4c947113a788a8f94fd390425c1",
        ("linux", "aarch64") => "394ddccc2402cfa3abd97dfba56f3587781a3d6e6ce66e65ceada14beb7664b8",
        ("windows", "x86_64") => "224a62a8351d3b8da9d950a4eb1d7176dc901dc4735b47f816f3dfcbc67d8654",
        _ => return None,
    })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = topcoat::tailwind::BuildConfig::new().input("styles.css");
    let config = match checksum() {
        Some(sha) => config.version_checksum(TAILWIND_VERSION, format!("sha256:{sha}")),
        None => return Err("no pinned Tailwind CLI checksum for this host platform".into()),
    };
    config.render()?;
    Ok(())
}
