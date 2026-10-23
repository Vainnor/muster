use topcoat::{
    Result,
    router::{href, page},
    view::{View, view},
};

use super::license;
use crate::site::{CONTACT_EMAIL, LEGAL_UPDATED};

#[page]
pub(crate) async fn page() -> Result<impl View> {
    let mailto = format!("mailto:{CONTACT_EMAIL}");
    Ok(view! {
        <article class="max-w-2xl space-y-4">
            <h1 class="text-3xl font-bold">"Terms of use"</h1>
            <p class="text-sm text-muted-foreground">
                "Last updated "
                (LEGAL_UPDATED)
            </p>

            <h2 class="text-xl font-semibold pt-4">"The software"</h2>
            <p>
                "Muster is free, open-source software released under the "
                <a class="underline" href=(href!(license::page))>"MIT license"</a>
                ". You may use, copy, change and share it under that license."
            </p>

            <h2 class="text-xl font-semibold pt-4">"No warranty"</h2>
            <p>
                "Muster is provided as is, without any warranty. You use it at your own risk. The authors are not liable for any damage or loss that comes from using it, as the license states in full."
            </p>

            <h2 class="text-xl font-semibold pt-4">"What you launch is up to you"</h2>
            <p>
                "Muster starts programs, scripts and links that you choose. You are responsible for what you add to it and run with it. Muster never asks for administrator rights."
            </p>

            <h2 class="text-xl font-semibold pt-4">"Unsigned preview builds"</h2>
            <p>
                "Early builds are not code-signed. Download them only from the official GitHub releases page and compare the published SHA-256 checksums."
            </p>

            <h2 class="text-xl font-semibold pt-4">"This website"</h2>
            <p>
                "The website is provided for information. It may change or go offline at any time."
            </p>

            <h2 class="text-xl font-semibold pt-4">"Contact"</h2>
            <p><a class="underline" href=(mailto)>(CONTACT_EMAIL)</a></p>
        </article>
    })
}
