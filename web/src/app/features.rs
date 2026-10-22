use topcoat::{
    Result,
    router::page,
    view::{View, component, view},
};

#[page]
pub(crate) async fn page() -> Result<impl View> {
    Ok(view! {
        <h1 class="text-3xl font-bold">"Features"</h1>
        <p class="mt-3 text-muted-foreground">
            "Muster is in early development and not released yet. Finding apps, search, \
             launching, groups, and theme import and export work in the preview builds. \
             Closing groups, hotkeys, the theme editor and update checks are still to come."
        </p>
        <div class="mt-8 grid gap-8">
            feature(
                title: "Launch groups",
                items: &[
                    "Build named groups of apps and start the whole group with one button.",
                    "Set the launch order and an optional delay before each app.",
                    "Give each app its own arguments and working folder inside a group.",
                    "Close every app in a group again with one action.",
                    "A global keyboard shortcut per group.",
                ]
            )
            feature(
                title: "Find and add apps",
                items: &[
                    "Finds the apps installed on Windows, macOS and Linux.",
                    "Add any program, script or web link by hand.",
                    "Before a group runs a script you added yourself for the first time, Muster asks you to confirm it.",
                ]
            )
            feature(
                title: "Out of your way",
                items: &[
                    "Keyboard first: type to search, Enter to launch.",
                    "Lives in the tray or menu bar; optional start on login.",
                    "Uses close to no CPU while idle. Closing the window frees its memory.",
                ]
            )
            feature(
                title: "Themes",
                items: &[
                    "Light, dark, or follow your system.",
                    "Presets: Nord, Gruvbox, Catppuccin, Solarized, High Contrast and Vainnor.",
                    "Edit a theme with a live preview, and import or export themes as files.",
                    "Warns you when a theme's text would be hard to read.",
                ]
            )
            feature(
                title: "Private by design",
                items: &[
                    "No telemetry, analytics or account.",
                    "Works offline. The only network request is an update check you start yourself.",
                    "Your settings stay in a file on your computer.",
                ]
            )
        </div>
    })
}

#[component]
async fn feature(title: &str, items: &[&str]) -> Result<impl View> {
    Ok(view! {
        <section>
            <h2 class="text-xl font-semibold">(title)</h2>
            <ul class="mt-2 list-disc pl-6 text-muted-foreground">
                for item in items {
                    <li>(item)</li>
                }
            </ul>
        </section>
    })
}
