//! Root of the module router: `/` plus the shared layout.

mod features;
mod license;
mod privacy;
mod terms;

use topcoat::{
    Result,
    asset::{AssetBundle, RouterBuilderAssetExt, asset},
    context::Cx,
    router::{Router, RouterBuilderDiscoverExt, Slot, href, layout, module_router, page, request},
    view::{View, component, view},
};

use crate::components::button::{ButtonSize, ButtonVariant, button_variants};
use crate::components::card::{card, card_description, card_header, card_title};
use crate::download::{self, Platform};
use crate::site::{GITHUB_URL, RELEASES_URL};

pub(crate) fn router(assets: AssetBundle) -> Router {
    module_router!()
        .layer(crate::security::layer())
        .discover()
        .assets(assets)
        .build()
}

#[layout]
async fn root_layout(slot: Slot<'_>) -> Result<impl View> {
    Ok(view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8">
                <meta name="viewport" content="width=device-width, initial-scale=1">
                <meta name="color-scheme" content="light dark">
                <meta
                    name="description"
                    content="Muster is a small, private app launcher for Windows, macOS and Linux. Launch groups of apps with one click."
                >
                <title>"Muster: launch your apps in groups"</title>
                <link rel="icon" type="image/svg+xml" href=(asset!("./favicon.svg"))>
                <link rel="stylesheet" href="/theme.css">
                <link rel="stylesheet" href=(topcoat::tailwind::stylesheet!())>
            </head>
            <body class="min-h-screen flex flex-col">
                <header class="border-b border-border">
                    <nav
                        class="mx-auto max-w-4xl flex flex-wrap items-center gap-x-6 gap-y-2 px-4 py-4"
                        aria-label="Main"
                    >
                        <a href=(href!(page)) class="font-semibold text-lg mr-auto">
                            "Muster"
                        </a>
                        <a
                            href=(href!(features::page))
                            class="text-muted-foreground hover:text-foreground"
                        >
                            "Features"
                        </a>
                        <a
                            href=(href!(privacy::page))
                            class="text-muted-foreground hover:text-foreground"
                        >
                            "Privacy"
                        </a>
                        <a
                            href=(GITHUB_URL)
                            class="text-muted-foreground hover:text-foreground"
                        >
                            "GitHub"
                        </a>
                    </nav>
                </header>
                <main class="mx-auto w-full max-w-4xl flex-1 px-4 py-10">(slot)</main>
                <footer class="border-t border-border">
                    <div
                        class="mx-auto max-w-4xl flex flex-wrap gap-x-6 gap-y-2 px-4 py-6 text-sm text-muted-foreground"
                    >
                        <span class="mr-auto">"© 2026 Vainnor · MIT license"</span>
                        <a href=(href!(privacy::page)) class="hover:text-foreground">
                            "Privacy policy"
                        </a>
                        <a href=(href!(terms::page)) class="hover:text-foreground">
                            "Terms"
                        </a>
                        <a href=(href!(license::page)) class="hover:text-foreground">
                            "License"
                        </a>
                        <a href=(GITHUB_URL) class="hover:text-foreground">
                            "Source code"
                        </a>
                    </div>
                </footer>
            </body>
        </html>
    })
}

#[page]
pub(crate) async fn page(cx: &Cx) -> Result<impl View> {
    let headers = request::headers(cx);
    let header = |name: &str| headers.get(name).and_then(|v| v.to_str().ok());
    let platform = download::detect(header("sec-ch-ua-platform"), header("user-agent"));

    Ok(view! {
        <section class="py-10">
            <h1 class="text-4xl font-bold tracking-tight">
                "Launch your apps in groups."
            </h1>
            <p class="mt-4 max-w-2xl text-lg text-muted-foreground">
                "Muster is a small app launcher for Windows, macOS and Linux. Put the apps you use \
                 together in a group and start all of them with one click. It runs offline, \
                 collects nothing, and costs almost nothing while it waits."
            </p>
            <div class="mt-8 flex flex-wrap items-center gap-4">
                download_button(platform: platform)
                <a
                    href=(GITHUB_URL)
                    class=(button_variants(ButtonVariant::Outline, ButtonSize::Lg))
                >
                    "View on GitHub"
                </a>
            </div>
            <p class="mt-3 text-sm text-muted-foreground">
                "Free and open source (MIT). The installers are not code-signed, so your OS will ask \
                 you to confirm the first launch."
            </p>
        </section>
        <section class="grid gap-4 sm:grid-cols-2" aria-label="Highlights">
            highlight(
                title: "Groups",
                text: "Start your editor, terminal and browser together, in order, with optional delays."
            )
            highlight(
                title: "Finds your apps",
                text: "Lists installed apps automatically. Add any program, script or link by hand."
            )
            highlight(
                title: "Private",
                text: "No telemetry, no account, no network access unless you check for updates."
            )
            highlight(
                title: "Yours to theme",
                text: "Light, dark or follow the system, presets like Nord and Gruvbox, or your own."
            )
        </section>
        <p class="mt-8">
            <a href=(href!(features::page)) class="underline">"See all features"</a>
        </p>
    })
}

#[component]
async fn download_button(platform: Option<Platform>) -> Result<impl View> {
    let classes = button_variants(ButtonVariant::Primary, ButtonSize::Lg);
    Ok(view! {
        <a href=(RELEASES_URL) class=(classes)>
            match platform {
                Some(p) => {
                    "Download for "
                    (p.label())
                }
                None => {
                    "Download"
                }
            }
        </a>
    })
}

#[component]
async fn highlight(title: &str, text: &str) -> Result<impl View> {
    Ok(view! {
        card(
            card_header(
                card_title((title))
                card_description((text))
            )
        )
    })
}
