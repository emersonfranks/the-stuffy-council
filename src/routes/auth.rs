//! Login / logout routes.

use askama::Template;
use axum::Form;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Redirect, Response};
use serde::Deserialize;
use tower_sessions::Session;

use crate::auth::{self, SESSION_USER_KEY, SessionUser};
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::web::csrf;

#[derive(Template)]
#[template(path = "login.html")]
struct LoginTemplate {
    csrf_token: String,
    error: Option<&'static str>,
}

pub async fn show_login(session: Session) -> AppResult<Response> {
    // If already logged in, bounce home.
    if session
        .get::<SessionUser>(SESSION_USER_KEY)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("session get: {e}")))?
        .is_some()
    {
        return Ok(Redirect::to("/").into_response());
    }
    let csrf_token = csrf::token(&session).await?;
    let tpl = LoginTemplate {
        csrf_token,
        error: None,
    };
    Ok(render(&tpl)?.into_response())
}

#[derive(Deserialize)]
pub struct LoginForm {
    username: String,
    password: String,
    #[serde(rename = "_csrf")]
    csrf: String,
}

pub async fn do_login(
    State(state): State<AppState>,
    session: Session,
    Form(form): Form<LoginForm>,
) -> AppResult<Response> {
    csrf::verify(&session, &form.csrf).await?;

    let user = match auth::authenticate(&state.db, form.username.trim(), &form.password).await? {
        Some(u) => u,
        None => {
            let csrf_token = csrf::token(&session).await?;
            let tpl = LoginTemplate {
                csrf_token,
                error: Some("Invalid username or password."),
            };
            return Ok((StatusCode::UNAUTHORIZED, render(&tpl)?).into_response());
        }
    };

    // Session-fixation defense: rotate the session id when the auth state changes.
    session
        .cycle_id()
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("cycle session id: {e}")))?;
    session
        .insert(SESSION_USER_KEY, &user)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("session insert user: {e}")))?;

    tracing::info!(user = %user.username, "login ok");
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
    Ok(Redirect::to("/login").into_response())
}

fn render<T: Template>(tpl: &T) -> AppResult<Html<String>> {
    let body = tpl
        .render()
        .map_err(|e| AppError::Internal(anyhow::anyhow!("template render: {e}")))?;
    Ok(Html(body))
}
