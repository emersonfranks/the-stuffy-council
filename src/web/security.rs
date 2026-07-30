//! [`BASE_CSP`] governs every route and admits no third-party origin and no
//! inline anything. [`LOGIN_CSP`] is the single documented relaxation, attached
//! by `routes::auth::show_login` because Google Identity Services needs it. Any
//! new relaxation must be scoped to one route the same way — do NOT widen
//! [`BASE_CSP`].
//!
//! `lib::serve` applies [`header_layers`] outermost, so middleware-synthesized
//! responses (408, 429) are covered too.

use axum::http::{HeaderName, HeaderValue};
use tower_http::set_header::SetResponseHeaderLayer;

use crate::config::Environment;

pub const CSP_HEADER: HeaderName = HeaderName::from_static("content-security-policy");

/// Content-Security-Policy for every page except `/login`.
///
/// Everything is same-origin with no inline execution, which matters most on
/// the story pages: model output is untrusted input (AGENTS.md rule 5), so an
/// escape past Askama's auto-escaping still cannot run script or style.
const BASE_CSP: &str = "\
default-src 'self'; \
script-src 'self'; \
style-src 'self'; \
img-src 'self' data:; \
font-src 'self' data:; \
connect-src 'self'; \
frame-src 'none'; \
frame-ancestors 'none'; \
form-action 'self'; \
base-uri 'self'; \
object-src 'none'";

/// Content-Security-Policy for `/login` only — [`BASE_CSP`] plus the source
/// expressions Google Identity Services requires.
///
/// * `script-src .../gsi/client` — path components ARE enforced on the initial
///   request (W3C CSP L3 §6.7.2.7, §6.7.2.12), so a compromise elsewhere on
///   `accounts.google.com` cannot serve script here without also controlling
///   `/gsi/client`. CAVEAT: paths are IGNORED after a redirect (§7.6); that is
///   acceptable because we trust the whole origin anyway and the pinning is
///   only defense-in-depth against upstream URL surface expansion.
/// * `style-src 'unsafe-inline'` — required by the styles the GIS library
///   injects for its button. This is the sole reason `'unsafe-inline'` still
///   exists anywhere, and it is why the login page renders no model output and
///   no user-supplied values beyond the allowlisted error strings.
/// * `img-src https://*.googleusercontent.com` — avatar on the personalized button.
/// * `connect-src` / `frame-src .../gsi/` — GIS auxiliary fetches and its
///   consent iframe.
const LOGIN_CSP: &str = "\
default-src 'self'; \
script-src 'self' https://accounts.google.com/gsi/client; \
style-src 'self' 'unsafe-inline' https://accounts.google.com/gsi/style; \
img-src 'self' data: https://*.googleusercontent.com; \
font-src 'self' data:; \
connect-src 'self' https://accounts.google.com/gsi/; \
frame-src https://accounts.google.com/gsi/; \
frame-ancestors 'none'; \
form-action 'self'; \
base-uri 'self'; \
object-src 'none'";

/// Header value a handler sets to opt its own response into [`LOGIN_CSP`]
/// instead of [`BASE_CSP`]. Works because the layer below only fills the
/// header in when the handler left it absent.
pub fn login_csp_value() -> HeaderValue {
    HeaderValue::from_static(LOGIN_CSP)
}

pub fn header_layers(env: Environment) -> Vec<SetResponseHeaderLayer<HeaderValue>> {
    let mut out = vec![
        // `if_not_present`, not `overriding`: a handler that already chose a
        // narrower-scoped policy (see `login_csp_value`) must win.
        SetResponseHeaderLayer::if_not_present(CSP_HEADER, HeaderValue::from_static(BASE_CSP)),
        SetResponseHeaderLayer::overriding(
            HeaderName::from_static("x-content-type-options"),
            HeaderValue::from_static("nosniff"),
        ),
        SetResponseHeaderLayer::overriding(
            HeaderName::from_static("referrer-policy"),
            HeaderValue::from_static("strict-origin-when-cross-origin"),
        ),
        SetResponseHeaderLayer::overriding(
            HeaderName::from_static("permissions-policy"),
            HeaderValue::from_static(
                "accelerometer=(), camera=(), geolocation=(), gyroscope=(), magnetometer=(), microphone=(), payment=(), usb=()",
            ),
        ),
        // X-Frame-Options is legacy but still respected; CSP frame-ancestors is the modern equivalent.
        SetResponseHeaderLayer::overriding(
            HeaderName::from_static("x-frame-options"),
            HeaderValue::from_static("DENY"),
        ),
    ];

    // HSTS is meaningful only over HTTPS; do not send it on plain-HTTP dev builds.
    if env == Environment::Production {
        out.push(SetResponseHeaderLayer::overriding(
            HeaderName::from_static("strict-transport-security"),
            HeaderValue::from_static("max-age=31536000; includeSubDomains"),
        ));
    }

    out
}

#[cfg(test)]
mod tests {
    use axum::Router;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use axum::routing::get;
    use tower::ServiceExt;

    use super::*;

    async fn response_for(env: Environment) -> axum::response::Response {
        let mut app = Router::new().route("/", get(|| async { StatusCode::NO_CONTENT }));
        for layer in header_layers(env) {
            app = app.layer(layer);
        }
        app.oneshot(Request::new(Body::empty()))
            .await
            .expect("security header response")
    }

    #[tokio::test]
    async fn header_layers_development_emits_baseline_without_hsts() {
        let response = response_for(Environment::Development).await;
        let headers = response.headers();

        assert_eq!(
            headers.get("content-security-policy").unwrap(),
            HeaderValue::from_static(BASE_CSP)
        );
        assert_eq!(headers.get("x-content-type-options").unwrap(), "nosniff");
        assert_eq!(
            headers.get("referrer-policy").unwrap(),
            "strict-origin-when-cross-origin"
        );
        assert!(headers.contains_key("permissions-policy"));
        assert_eq!(headers.get("x-frame-options").unwrap(), "DENY");
        assert!(!headers.contains_key("strict-transport-security"));
    }

    #[tokio::test]
    async fn header_layers_production_adds_hsts() {
        let response = response_for(Environment::Production).await;

        assert_eq!(
            response.headers().get("strict-transport-security").unwrap(),
            "max-age=31536000; includeSubDomains"
        );
    }

    #[tokio::test]
    async fn header_layers_preserve_handler_chosen_login_policy() {
        let mut app = Router::new().route(
            "/",
            get(|| async { ([(CSP_HEADER, login_csp_value())], StatusCode::NO_CONTENT) }),
        );
        for layer in header_layers(Environment::Development) {
            app = app.layer(layer);
        }

        let response = app
            .oneshot(Request::new(Body::empty()))
            .await
            .expect("security header response");

        assert_eq!(
            response.headers().get(CSP_HEADER).unwrap(),
            HeaderValue::from_static(LOGIN_CSP)
        );
    }

    #[test]
    fn base_csp_admits_exactly_the_same_origin_token_set() {
        assert_eq!(
            directives(BASE_CSP),
            vec![
                ("default-src", vec!["'self'"]),
                ("script-src", vec!["'self'"]),
                ("style-src", vec!["'self'"]),
                ("img-src", vec!["'self'", "data:"]),
                ("font-src", vec!["'self'", "data:"]),
                ("connect-src", vec!["'self'"]),
                ("frame-src", vec!["'none'"]),
                ("frame-ancestors", vec!["'none'"]),
                ("form-action", vec!["'self'"]),
                ("base-uri", vec!["'self'"]),
                ("object-src", vec!["'none'"]),
            ]
        );
    }

    #[test]
    fn login_csp_admits_exactly_the_same_origin_token_set_plus_gis() {
        assert_eq!(
            directives(LOGIN_CSP),
            vec![
                ("default-src", vec!["'self'"]),
                (
                    "script-src",
                    vec!["'self'", "https://accounts.google.com/gsi/client"]
                ),
                (
                    "style-src",
                    vec![
                        "'self'",
                        "'unsafe-inline'",
                        "https://accounts.google.com/gsi/style"
                    ]
                ),
                (
                    "img-src",
                    vec!["'self'", "data:", "https://*.googleusercontent.com"]
                ),
                ("font-src", vec!["'self'", "data:"]),
                (
                    "connect-src",
                    vec!["'self'", "https://accounts.google.com/gsi/"]
                ),
                ("frame-src", vec!["https://accounts.google.com/gsi/"]),
                ("frame-ancestors", vec!["'none'"]),
                ("form-action", vec!["'self'"]),
                ("base-uri", vec!["'self'"]),
                ("object-src", vec!["'none'"]),
            ]
        );
    }

    /// Both policies are exact-matched above, so this pins the *reason* those
    /// literals were chosen: a future widening must fail a named assertion
    /// rather than only a diff-shaped one.
    #[test]
    fn neither_policy_admits_a_wildcard_scheme_or_inline_script() {
        for (label, policy) in [("base", BASE_CSP), ("login", LOGIN_CSP)] {
            for (directive, sources) in directives(policy) {
                for source in sources {
                    assert!(
                        source != "*" && source != "http:" && source != "https:",
                        "{label} {directive} admits wildcard source {source}"
                    );
                    assert!(
                        !source.starts_with("http://"),
                        "{label} {directive} admits plaintext origin {source}"
                    );
                }
            }
        }
        assert!(!BASE_CSP.contains("unsafe-"));
        assert!(!LOGIN_CSP.contains("unsafe-eval"));
        // 'unsafe-inline' is style-only and login-only; script must never get it.
        assert_eq!(LOGIN_CSP.matches("'unsafe-inline'").count(), 1);
        assert!(
            source_set(LOGIN_CSP, "style-src").contains(&"'unsafe-inline'"),
            "the sole 'unsafe-inline' must be the GIS style relaxation"
        );
    }

    fn directives(policy: &str) -> Vec<(&str, Vec<&str>)> {
        policy
            .split("; ")
            .map(|d| {
                let mut tokens = d.trim_end_matches(';').split_whitespace();
                let name = tokens.next().expect("directive name");
                (name, tokens.collect())
            })
            .collect()
    }

    fn source_set<'a>(policy: &'a str, directive: &str) -> Vec<&'a str> {
        directives(policy)
            .into_iter()
            .find(|(name, _)| *name == directive)
            .map(|(_, sources)| sources)
            .unwrap_or_default()
    }
}
