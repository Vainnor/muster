//! Scans this machine's app folders and prints counts and timing. Names are printed only with
//! `--names`; they describe your machine, so do not paste them into issues.

#![allow(
    clippy::print_stdout,
    reason = "a command-line example prints its result"
)]

fn main() {
    let start = std::time::Instant::now();
    let scan = muster_core::detect::detect_installed();
    let elapsed = start.elapsed();
    let valid = scan
        .apps
        .iter()
        .filter(|a| muster_core::launch::validate(&a.target).is_ok())
        .count();
    let icons = scan.apps.iter().filter(|a| a.icon.is_some()).count();
    println!(
        "{} apps ({valid} pass launch::validate, {icons} with an icon), {} skipped, {elapsed:?}",
        scan.apps.len(),
        scan.skipped
    );
    if std::env::args().any(|a| a == "--names") {
        for app in &scan.apps {
            println!("{}", app.name);
        }
    }
}
