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
/// * script-src 'self' https://unpkg.com https://accounts.google.com/gsi/client
///   — HTMX from unpkg + the GIS client library. Path components ARE enforced
///   by browsers on the initial request (W3C CSP L3 §6.7.2.7, §6.7.2.12), so
///   this restricts scripts to those two exact URLs; a compromise elsewhere on
///   `accounts.google.com` cannot serve script into our pages without also
///   controlling `/gsi/client`. CAVEAT: paths are IGNORED after a redirect
///   (§7.6), so if Google ever 302s `/gsi/client` to another path on the same
///   host the browser will still load it — which is fine here because we trust
///   the whole `accounts.google.com` origin, and the pinning is just
///   defense-in-depth against upstream URL surface expansion.
/// * style-src 'self' 'unsafe-inline' https://accounts.google.com/gsi/style
///   — our self-hosted /static/app.css loads via 'self'; 'unsafe-inline' is
///   now required ONLY by the GIS button's injected inline styles (there is no
///   Tailwind CDN anymore — it was a <script> this CSP never allowed). #9 can
///   pin GIS and drop 'unsafe-inline'.
/// * img-src 'self' data: https://*.googleusercontent.com — user avatars appear on the GIS personalized button.
/// * connect-src 'self' https://accounts.google.com/gsi/ — GIS auxiliary fetches (revocation, one-tap resources).
/// * frame-src https://accounts.google.com/gsi/ — GIS renders its consent UI in an iframe.
/// * frame-ancestors 'none' — WE never want to be iframed by anyone else.
/// * form-action 'self' — Google's GIS POST is initiated on Google's page (governed by their CSP)
///   and targets us; our CSP only governs forms rendered on our origin.
/// * base-uri 'self' — kill `<base>` injection tricks.
/// * object-src 'none' — no plugins.
const CSP: &str = "\
default-src 'self'; \
script-src 'self' https://unpkg.com https://accounts.google.com/gsi/client; \
style-src 'self' 'unsafe-inline' https://accounts.google.com/gsi/style; \
img-src 'self' data: https://*.googleusercontent.com; \
font-src 'self' data:; \
connect-src 'self' https://accounts.google.com/gsi/; \
frame-src https://accounts.google.com/gsi/; \
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
