use std::sync::Arc;

use crate::{auth::JwksCache, config::AppConfig};
use sqlx::PgPool;

/// Estado compartilhado da aplicação, injetado em todos os handlers via Axum.
#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub config: AppConfig,
    /// Cache de JWKS para validação de JWT. `None` quando `JWKS_URL` não está configurada
    /// (middleware JWT passa de forma transparente).
    pub jwks_cache: Option<Arc<JwksCache>>,
    pub redis_cache: Option<Arc<crate::cache::RedisCache>>,
}
