use topcoat::{
    Result,
    router::page,
    view::{View, view},
};

use crate::site::LICENSE_TEXT;

#[page]
pub(crate) async fn page() -> Result<impl View> {
    Ok(view! {
        <h1 class="text-3xl font-bold">"License"</h1>
        <p class="mt-3 text-muted-foreground">
            "Muster is released under the MIT license."
        </p>
        <pre
            class="mt-6 whitespace-pre-wrap rounded-md border border-border bg-card p-4 font-mono text-sm"
        >
            (LICENSE_TEXT)
        </pre>
    })
}
