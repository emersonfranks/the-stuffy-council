//! Shared state passed to every handler.

use std::sync::Arc;

use sqlx::SqlitePool;

use crate::auth::GoogleOAuthClient;
use crate::cast::CastRegistry;
use crate::config::Config;
use crate::stories::StoryService;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub db: SqlitePool,
    pub cast: Arc<CastRegistry>,
    pub stories: StoryService,
    /// Configured oauth2 client for Google. Cheap to clone (Arc-shared internally).
    pub oauth: GoogleOAuthClient,
    /// Shared HTTP client. Same instance is used for the token exchange and the
    /// userinfo call — both need `redirect(Policy::none())` per oauth2 guidance.
    pub http: reqwest::Client,
}
