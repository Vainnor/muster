//! Security headers on every response, including errors and 404s.
//!
//! The site sends no JavaScript, sets no cookies and loads nothing from another origin, so
//! the policy can be as strict as CSP allows.

use http::{HeaderName, HeaderValue, header};
use topcoat::{
    context::Cx,
    router::{Body, LayerFn, LayerFuture, Next, response::IntoResponse, response::Response},
};

pub(crate) const CSP: &str = "default-src 'none'; style-src 'self'; img-src 'self'; \
font-src 'self'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'";

pub(crate) const HEADERS: &[(&str, &str)] = &[
    ("content-security-policy", CSP),
    ("x-content-type-options", "nosniff"),
    ("referrer-policy", "no-referrer"),
    ("cross-origin-opener-policy", "same-origin"),
    ("cross-origin-resource-policy", "same-origin"),
    (
        "permissions-policy",
        "camera=(), microphone=(), geolocation=(), interest-cohort=(), browsing-topics=()",
    ),
    (
        "strict-transport-security",
        "max-age=31536000; includeSubDomains",
    ),
    ("x-frame-options", "DENY"),
];

pub(crate) fn layer() -> LayerFn {
    LayerFn::new(None::<&str>, handle)
}

fn handle<'a>(cx: &'a Cx, body: Body, next: Next<'a>) -> LayerFuture<'a> {
    Box::pin(async move {
        let mut response = match next.run(cx, body).await {
            Ok(response) => response,
            // An error that cannot render itself still leaves with the headers.
            Err(error) => error.into_response(cx).unwrap_or_else(|e| {
                #[allow(clippy::print_stderr, reason = "no logger configured")]
                {
                    eprintln!("muster-web: rendering an error response failed: {e}");
                }
                internal_error()
            }),
        };
        apply(&mut response);
        Ok(response)
    })
}

fn internal_error() -> Response {
    let mut response = Response::new(Body::empty());
    *response.status_mut() = http::StatusCode::INTERNAL_SERVER_ERROR;
    response
}

pub(crate) fn apply(response: &mut Response) {
    let headers = response.headers_mut();
    for (name, value) in HEADERS {
        headers.insert(
            HeaderName::from_static(name),
            HeaderValue::from_static(value),
        );
    }
    // The download button's label depends on these request headers.
    headers.append(
        header::VARY,
        HeaderValue::from_static("User-Agent, Sec-CH-UA-Platform"),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csp_allows_no_script_and_no_other_origin() {
        assert!(CSP.starts_with("default-src 'none';"));
        assert!(
            !CSP.contains("script-src"),
            "default-src 'none' already blocks scripts"
        );
        for bad in ["unsafe-inline", "unsafe-eval", "http:", "https:", "*"] {
            assert!(!CSP.contains(bad), "{bad}");
        }
    }

    #[test]
    fn the_fallback_error_is_a_bare_500() {
        let response = internal_error();
        assert_eq!(response.status(), http::StatusCode::INTERNAL_SERVER_ERROR);
        assert!(
            response.headers().is_empty(),
            "apply adds the headers after"
        );
    }

    #[test]
    fn apply_sets_every_header() {
        let mut response = Response::new(Body::empty());
        apply(&mut response);
        for (name, value) in HEADERS {
            assert_eq!(response.headers()[*name], *value, "{name}");
        }
        assert!(response.headers().contains_key(header::VARY));
    }
}
