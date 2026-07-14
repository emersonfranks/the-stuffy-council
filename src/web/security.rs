//! Security headers applied to every response.
//!
//! Values here are the OWASP-recommended baseline for a server-rendered site
//! with a small amount of inline template output. If we later add JS from
//! third parties (analytics, CDN, etc.) we'll need to loosen `script-src`
//! deliberately — do NOT sprinkle `unsafe-inline` around casually.

use axum::http::{HeaderName, HeaderValue};
use tower_http::set_header::SetResponseHeaderLayer;

use crate::config::Environment;

/// Content-Security-Policy for our own pages.
///
/// * default-src 'self' — everything must come from our origin unless overridden.
/// * script-src 'self' https://unpkg.com — HTMX loaded from a versioned unpkg URL for now.
/// * style-src 'self' 'unsafe-inline' https://cdn.tailwindcss.com — Tailwind CDN + Askama-embedded style attributes.
///   TODO: swap the Tailwind CDN for a self-built stylesheet before production so we can drop 'unsafe-inline'.
/// * img-src 'self' data: — generated images we serve locally; data: URIs allowed for small inline art.
/// * connect-src 'self' — no cross-origin fetches from the browser.
/// * frame-ancestors 'none' — we never want to be iframed.
/// * form-action 'self' — forms may only POST back to us.
/// * base-uri 'self' — kill `<base>` injection tricks.
/// * object-src 'none' — no plugins.
const CSP: &str = "\
default-src 'self'; \
script-src 'self' https://unpkg.com; \
style-src 'self' 'unsafe-inline' https://cdn.tailwindcss.com; \
img-src 'self' data:; \
font-src 'self' data:; \
connect-src 'self'; \
frame-ancestors 'none'; \
form-action 'self'; \
base-uri 'self'; \
object-src 'none'";

pub fn header_layers(env: Environment) -> Vec<SetResponseHeaderLayer<HeaderValue>> {
    let mut out = Vec::new();

    out.push(SetResponseHeaderLayer::overriding(
        HeaderName::from_static("content-security-policy"),
        HeaderValue::from_static(CSP),
    ));
    out.push(SetResponseHeaderLayer::overriding(
        HeaderName::from_static("x-content-type-options"),
        HeaderValue::from_static("nosniff"),
    ));
    out.push(SetResponseHeaderLayer::overriding(
        HeaderName::from_static("referrer-policy"),
        HeaderValue::from_static("strict-origin-when-cross-origin"),
    ));
    out.push(SetResponseHeaderLayer::overriding(
        HeaderName::from_static("permissions-policy"),
        HeaderValue::from_static(
            "accelerometer=(), camera=(), geolocation=(), gyroscope=(), magnetometer=(), microphone=(), payment=(), usb=()",
        ),
    ));
    // X-Frame-Options is legacy but still respected — CSP frame-ancestors is the modern equivalent.
    out.push(SetResponseHeaderLayer::overriding(
        HeaderName::from_static("x-frame-options"),
        HeaderValue::from_static("DENY"),
    ));

    // HSTS is meaningful only over HTTPS; do not send it on plain-HTTP dev builds.
    if env == Environment::Production {
        out.push(SetResponseHeaderLayer::overriding(
            HeaderName::from_static("strict-transport-security"),
            HeaderValue::from_static("max-age=31536000; includeSubDomains"),
        ));
    }

    out
}
