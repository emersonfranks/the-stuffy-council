//! Shared state passed to every handler.

use std::sync::Arc;

use sqlx::SqlitePool;

use crate::cast::CastRegistry;
use crate::config::Config;
use crate::stories::StoryService;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub db: SqlitePool,
    pub cast: Arc<CastRegistry>,
    pub stories: StoryService,
}
