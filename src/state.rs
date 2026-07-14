//! Shared state passed to every handler.

use std::sync::Arc;

use sqlx::SqlitePool;

use crate::access::AccessList;
use crate::auth::JwkCache;
use crate::cast::CastRegistry;
use crate::config::Config;
use crate::stories::StoryService;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub db: SqlitePool,
    pub cast: Arc<CastRegistry>,
    pub stories: StoryService,
    /// Committed allowlist. Sole gate after a successful Google sign-in;
    /// also carries the `admin` flag stashed on `SessionUser`.
    pub access: Arc<AccessList>,
    /// Google JWKS cache. Verifies signatures on incoming ID tokens
    /// without ever holding a client secret.
    pub jwks: Arc<JwkCache>,
}
