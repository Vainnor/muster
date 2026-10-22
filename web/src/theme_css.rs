//! `/theme.css`: the neutral light and dark themes from `muster-ui`, switched by
//! `prefers-color-scheme`. Generated from the same presets the desktop app ships.

use http::{HeaderValue, header};
use topcoat::{
    Result,
    router::{response::Response, route},
};

#[route(GET "/theme.css")]
pub(crate) async fn theme_css() -> Result<Response> {
    let mut response = Response::new(css().into());
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/css; charset=utf-8"),
    );
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=3600"),
    );
    Ok(response)
}

/// Light by default, dark when the visitor's system prefers it.
pub(crate) fn css() -> String {
    let mut css = String::new();
    if let Some(light) = muster_core::themes::preset("light") {
        css.push_str(&muster_ui::stylesheet(&light, ":root"));
    }
    if let Some(dark) = muster_core::themes::preset("dark") {
        css.push_str("@media (prefers-color-scheme: dark) {\n");
        css.push_str(&muster_ui::stylesheet(&dark, ":root"));
        css.push_str("}\n");
    }
    css
}

#[cfg(test)]
mod tests {
    #[test]
    fn has_light_defaults_and_a_dark_override() {
        let css = super::css();
        assert!(css.starts_with(":root {\n  --mu-color-scheme: light;"));
        assert!(css.contains(
            "@media (prefers-color-scheme: dark) {\n:root {\n  --mu-color-scheme: dark;"
        ));
        assert!(!css.contains("url("));
    }
}
