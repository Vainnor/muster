//! Plain-language privacy policy, written from PRIVACY.md. Keep the two in step
//! (`.claude/rules/docs.md`).

use topcoat::{
    Result,
    router::page,
    view::{View, view},
};

use crate::site::{CONTACT_EMAIL, GITHUB_URL, LEGAL_UPDATED};

#[page]
pub(crate) async fn page() -> Result<impl View> {
    let mailto = format!("mailto:{CONTACT_EMAIL}");
    Ok(view! {
        <article class="max-w-2xl space-y-4">
            <h1 class="text-3xl font-bold">"Privacy policy"</h1>
            <p class="text-sm text-muted-foreground">
                "Last updated "
                (LEGAL_UPDATED)
            </p>

            <h2 class="text-xl font-semibold pt-4">"The short version"</h2>
            <p>
                "Muster does not collect, send or sell any information about you. The app and this website have no analytics, no trackers and no accounts."
            </p>

            <h2 class="text-xl font-semibold pt-4">"The Muster app"</h2>
            <ul class="list-disc pl-6 space-y-1">
                <li>
                    "The app does not send usage data, crash reports or anything else to us or to anyone else."
                </li>
                <li>
                    "It works without an internet connection and makes no network requests at all. If a check for new versions is added later, it will run only when you ask, and this page will say so before it ships."
                </li>
                <li>
                    "The window is drawn by your system's own web view: WebKit on macOS, WebView2 on Windows, WebKitGTK on Linux. On Windows, Muster turns off Microsoft SmartScreen in it, so the pages it shows are not sent to Microsoft for checking. Your system keeps the web view up to date, not Muster."
                </li>
                <li>
                    "Your apps, groups, themes, settings and the programs you approved to run are saved in a \
                     file on your own computer. They are never uploaded."
                </li>
                <li>
                    "To find installed apps, Muster reads app names, icons and launch commands from the standard app folders. It does not read your documents, recent files or browser data."
                </li>
                <li>
                    "If you turn on start on login, Muster adds one entry to your system's list of programs to start when you log in. It holds only where Muster is installed. Turning the setting off removes it."
                </li>
                <li>"Muster stores no passwords, tokens or keys."</li>
            </ul>

            <h2 class="text-xl font-semibold pt-4">"This website"</h2>
            <ul class="list-disc pl-6 space-y-1">
                <li>"The website sets no cookies and runs no scripts."</li>
                <li>
                    "It loads nothing from other companies: no fonts, images, scripts or embeds."
                </li>
                <li>
                    "The server that hosts it may keep standard access logs (IP address, time, page requested, browser name) to keep it running and secure. These logs are not used to track you and are not shared."
                </li>
                <li>
                    "The site is delivered through Cloudflare, which sees each request, including your IP address, under "
                    <a
                        class="underline"
                        href="https://www.cloudflare.com/privacypolicy/"
                    >
                        "Cloudflare's privacy policy"
                    </a>
                    ". Cloudflare also asks browsers to report network errors: if loading a page fails, some browsers send Cloudflare a short note saying which page and what went wrong. This does not change the page."
                </li>
                <li>
                    "The download button sends you to GitHub, where "
                    <a
                        class="underline"
                        href="https://docs.github.com/site-policy/privacy-policies/github-general-privacy-statement"
                    >
                        "GitHub's privacy statement"
                    </a>
                    " applies."
                </li>
            </ul>

            <h2 class="text-xl font-semibold pt-4">"Changes"</h2>
            <p>
                "If this ever changes, the change will be described in the release notes and on this page before it ships. The source code is public on "
                <a class="underline" href=(GITHUB_URL)>"GitHub"</a>
                ", so anyone can check."
            </p>

            <h2 class="text-xl font-semibold pt-4">"Contact"</h2>
            <p>
                "Questions: "
                <a class="underline" href=(mailto)>(CONTACT_EMAIL)</a>
            </p>
        </article>
    })
}
