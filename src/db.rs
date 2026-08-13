use sqlx::postgres::{PgPool, PgPoolOptions};
use sqlx::Error as SqlxError;
use std::time::Duration;

/// Creates a PostgreSQL connection pool given a database connection string.
///
/// # Errors
/// Returns `SqlxError` if pool creation or connection validation fails.
pub async fn create_pool(database_url: &str) -> Result<PgPool, SqlxError> {
    PgPoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(3))
        .connect(database_url)
        .await
}

/// Runs embedded database migrations against the provided connection pool.
///
/// # Errors
/// Returns `sqlx::migrate::MigrateError` if running embedded migrations fails.
pub async fn run_migrations(pool: &PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!("./migrations").run(pool).await
}
