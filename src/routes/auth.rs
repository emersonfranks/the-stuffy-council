//! Login / logout routes. Sign-in delegates to Google OAuth.

use askama::Template;
use axum::Form;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Redirect, Response};
use serde::Deserialize;
use tower_sessions::Session;

use crate::auth::{self, OAUTH_STATE_KEY, PendingOAuth, SESSION_USER_KEY, SessionUser};
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::web::csrf;

#[derive(Template)]
#[template(path = "login.html")]
struct LoginTemplate {
    error: Option<&'static str>,
}

/// GET /login — shows the "Sign in with Google" button. If a valid
/// session already exists, redirect straight home.
pub async fn show_login(session: Session, Query(q): Query<LoginQuery>) -> AppResult<Response> {
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
        Some("state") => Some("Sign-in state expired. Please try again."),
        Some("google") => Some("Google sign-in failed. Please try again."),
        _ => None,
    };
    let tpl = LoginTemplate { error };
    Ok(render(&tpl)?.into_response())
}

#[derive(Deserialize)]
pub struct LoginQuery {
    error: Option<String>,
}

/// GET /auth/google — kicks off the OAuth 2.0 authorization-code flow with
/// PKCE. Stashes CSRF+verifier in the session, redirects to Google.
pub async fn start_google(
    State(state): State<AppState>,
    session: Session,
) -> AppResult<Response> {
    let (auth_url, pending) = auth::begin_authorization(&state.oauth);
    session
        .insert(OAUTH_STATE_KEY, &pending)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("session insert oauth state: {e}")))?;
    Ok(Redirect::to(auth_url.as_str()).into_response())
}

#[derive(Deserialize)]
pub struct GoogleCallback {
    code: Option<String>,
    state: Option<String>,
    /// Present when the user cancels or Google rejects consent.
    error: Option<String>,
}

/// GET /auth/google/callback — Google redirects the browser here with a
/// `code` (success) or an `error` (denial). We verify state, exchange the
/// code, fetch userinfo, check the allowlist, upsert the user, rotate the
/// session id, and drop them at `/`.
pub async fn google_callback(
    State(state): State<AppState>,
    session: Session,
    Query(cb): Query<GoogleCallback>,
) -> AppResult<Response> {
    // Consume the pending state regardless of outcome — do NOT leave
    // stale challenges sitting in the session.
    let pending: Option<PendingOAuth> = session
        .remove(OAUTH_STATE_KEY)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("session remove oauth state: {e}")))?;

    if let Some(err) = cb.error.as_deref() {
        tracing::info!(google_error = %err, "user denied or Google rejected consent");
        return Ok(Redirect::to("/login?error=google").into_response());
    }

    let Some(pending) = pending else {
        // Callback with no matching session state: someone hit /callback
        // directly, or the session expired between clicks.
        tracing::warn!("oauth callback without pending session state");
        return Ok(Redirect::to("/login?error=state").into_response());
    };

    let (Some(code), Some(returned_state)) = (cb.code, cb.state) else {
        tracing::warn!("oauth callback missing code or state param");
        return Ok(Redirect::to("/login?error=state").into_response());
    };

    if !constant_time_eq(pending.csrf.as_bytes(), returned_state.as_bytes()) {
        tracing::warn!("oauth callback state mismatch");
        return Ok(Redirect::to("/login?error=state").into_response());
    }

    let token = match auth::exchange_code(&state.oauth, &state.http, code, pending.pkce_verifier)
        .await
    {
        Ok(t) => t,
        Err(e) => {
            tracing::warn!(error = ?e, "google token exchange failed");
            return Ok(Redirect::to("/login?error=google").into_response());
        }
    };

    use oauth2::TokenResponse;
    let info = match auth::fetch_userinfo(&state.http, token.access_token().secret()).await {
        Ok(i) => i,
        Err(e) => {
            tracing::warn!(error = ?e, "google userinfo fetch failed");
            return Ok(Redirect::to("/login?error=google").into_response());
        }
    };

    // Allowlist gate — the only access-control decision beyond Google.
    let Some(entry) = state.access.check(&info.email) else {
        tracing::warn!(email = %info.email, "sign-in blocked: not on allowlist");
        return Ok(Redirect::to("/login?error=denied").into_response());
    };
    let admin = entry.admin;

    let user = auth::upsert_user(&state.db, &info, admin)
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

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    use subtle::ConstantTimeEq;
    if a.len() != b.len() {
        return false;
    }
    a.ct_eq(b).unwrap_u8() == 1
}

fn render<T: Template>(tpl: &T) -> AppResult<Html<String>> {
    let body = tpl
        .render()
        .map_err(|e| AppError::Internal(anyhow::anyhow!("template render: {e}")))?;
    Ok(Html(body))
}
