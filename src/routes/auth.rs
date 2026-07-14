//! Sign-in and sign-out routes.
//!
//! Auth is delegated entirely to Google Identity Services (GIS). The login
//! page embeds Google's button; on success Google POSTs an ID token JWT
//! here. We verify it against Google's public JWKS, gate on the allowlist,
//! and open a session. No OAuth 2.0 code flow, no client_secret.

use askama::Template;
use axum::Form;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{Html, IntoResponse, Redirect, Response};
use serde::Deserialize;
use tower_sessions::Session;

use crate::auth::{self, SESSION_USER_KEY, SessionUser};
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::web::csrf;

/// Name of the double-submit CSRF cookie Google GIS sets when it POSTs the
/// credential to our login endpoint. Same string appears in the form body.
const GOOGLE_CSRF_COOKIE: &str = "g_csrf_token";

#[derive(Template)]
#[template(path = "login.html")]
struct LoginTemplate<'a> {
    error: Option<&'static str>,
    google_client_id: &'a str,
    login_uri: String,
}

/// GET /login — renders the GIS sign-in button. If a valid session already
/// exists, redirect straight home.
pub async fn show_login(
    State(state): State<AppState>,
    session: Session,
    Query(q): Query<LoginQuery>,
) -> AppResult<Response> {
    if session
        .get::<SessionUser>(SESSION_USER_KEY)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("session get: {e}")))?
        .is_some()
    {
        return Ok(Redirect::to("/").into_response());
    }
    let error = match q.error.as_deref() {
        Some("denied") => Some("That Google account is not on the allowlist for this site."),
        Some("csrf") => Some("Sign-in request failed a security check. Please try again."),
        Some("google") => Some("Google sign-in failed. Please try again."),
        _ => None,
    };
    let login_uri = format!("{}/auth/google/verify", state.config.public_origin);
    let tpl = LoginTemplate {
        error,
        google_client_id: &state.config.google_client_id,
        login_uri,
    };
    Ok(render(&tpl)?.into_response())
}

#[derive(Deserialize)]
pub struct LoginQuery {
    error: Option<String>,
}

/// Form body Google GIS POSTs to our login endpoint.
///
/// The struct field `g_csrf_token` intentionally matches the wire name so
/// we don't need a serde rename attribute (also matches the cookie name).
#[derive(Deserialize)]
pub struct VerifyForm {
    credential: String,
    g_csrf_token: String,
}

/// POST /auth/google/verify — Google GIS delivers the signed ID token here.
///
/// Steps:
///   1. Double-submit CSRF: the `g_csrf_token` cookie must equal the form
///      field. Only our own origin can read the cookie, so if they match,
///      the POST was initiated from a page we rendered.
///   2. Verify the ID token: signature (against Google's JWKS), issuer,
///      audience (== our client_id), expiry, and `email_verified`.
///   3. Allowlist gate: the email must be present in `authorized-users.toml`.
///   4. Upsert user row, cycle session id (fixation defense), stash SessionUser.
pub async fn google_verify(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Form(form): Form<VerifyForm>,
) -> AppResult<Response> {
    let Some(cookie_token) = headers
        .get(header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .and_then(parse_g_csrf_cookie)
    else {
        tracing::warn!("google verify: missing g_csrf_token cookie");
        return Ok(Redirect::to("/login?error=csrf").into_response());
    };
    if cookie_token.is_empty() || cookie_token != form.g_csrf_token {
        tracing::warn!("google verify: g_csrf_token mismatch");
        return Ok(Redirect::to("/login?error=csrf").into_response());
    }

    let claims = match auth::verify_id_token(
        &state.jwks,
        &state.config.google_client_id,
        &form.credential,
    )
    .await
    {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(error = ?e, "google id token verification failed");
            return Ok(Redirect::to("/login?error=google").into_response());
        }
    };

    let Some(entry) = state.access.check(&claims.email) else {
        tracing::warn!(email = %claims.email, "sign-in blocked: not on allowlist");
        return Ok(Redirect::to("/login?error=denied").into_response());
    };
    let admin = entry.admin;

    let user = auth::upsert_user(&state.db, &claims, admin)
        .await
        .map_err(AppError::Internal)?;

    // Session-fixation defense: new session id on any auth-state change.
    session
        .cycle_id()
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("cycle session id: {e}")))?;
    session
        .insert(SESSION_USER_KEY, &user)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("session insert user: {e}")))?;

    tracing::info!(user_id = user.id, email = %user.email, "google sign-in ok");
    Ok(Redirect::to("/").into_response())
}

#[derive(Deserialize)]
pub struct LogoutForm {
    #[serde(rename = "_csrf")]
    csrf: String,
}

pub async fn do_logout(session: Session, Form(form): Form<LogoutForm>) -> AppResult<Response> {
    csrf::verify(&session, &form.csrf).await?;
    session
        .flush()
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("session flush: {e}")))?;
    Ok((StatusCode::SEE_OTHER, Redirect::to("/login")).into_response())
}

/// Pull the value of the `g_csrf_token` cookie from a raw `Cookie:` header.
/// Returns None if the cookie is absent.
fn parse_g_csrf_cookie(cookie_header: &str) -> Option<String> {
    let prefix = format!("{GOOGLE_CSRF_COOKIE}=");
    for part in cookie_header.split(';') {
        if let Some(v) = part.trim().strip_prefix(&prefix) {
            return Some(v.to_owned());
        }
    }
    None
}

fn render<T: Template>(tpl: &T) -> AppResult<Html<String>> {
    let body = tpl
        .render()
        .map_err(|e| AppError::Internal(anyhow::anyhow!("template render: {e}")))?;
    Ok(Html(body))
}
