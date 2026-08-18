use std::sync::Arc;

use template_servico_rust::{
    auth::JwksCache, cache::RedisCache, config::AppConfig, db, routes, state::AppState, telemetry,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = AppConfig::load();
    telemetry::init_tracing(
        &config.environment,
        Some(&config.otel_service_name),
        config.otlp_endpoint.as_deref(),
    );

    tracing::info!(
        environment = %config.environment,
        host = %config.host,
        port = config.port,
        "Starting application"
    );

    let pool = db::create_pool(&config.database_url).await?;
    db::run_migrations(&pool).await?;

    let jwks_cache = config
        .jwks_url
        .as_ref()
        .map(|url| Arc::new(JwksCache::new(url.clone())));

    let redis_cache = match &config.redis_url {
        Some(url) => {
            let cache = RedisCache::new(url).await?;
            Some(Arc::new(cache))
        }
        None => None,
    };

    let app_state = AppState {
        pool: pool.clone(),
        config: config.clone(),
        jwks_cache,
        redis_cache,
    };
    let app = routes::create_router(app_state);

    let address = format!("{}:{}", config.host, config.port);
    let listener = tokio::net::TcpListener::bind(&address).await?;
    tracing::info!("Server running on http://{}", address);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    tracing::info!("HTTP server stopped, closing database pool...");
    pool.close().await;

    if config.otlp_endpoint.is_some() && config.environment.is_production() {
        opentelemetry::global::shutdown_tracer_provider();
    }

    tracing::info!("Graceful shutdown complete.");

    Ok(())
}

/// Aguarda um sinal de encerramento do sistema operacional (SIGINT ou SIGTERM).
///
/// Essa função bloqueia até que um dos sinais seja recebido, permitindo que o
/// servidor Axum encerre o tráfego HTTP limpamente via `with_graceful_shutdown`.
async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(e) = tokio::signal::ctrl_c().await {
            tracing::warn!(error = %e, "Failed to listen for Ctrl+C (SIGINT) signal");
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut s) => {
                s.recv().await;
            }
            Err(e) => {
                tracing::warn!(error = %e, "Failed to install SIGTERM handler");
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }

    tracing::info!("Shutdown signal received, starting graceful shutdown...");
}
