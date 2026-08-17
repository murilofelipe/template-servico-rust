use template_servico_rust::{config::AppConfig, db, routes, state::AppState, telemetry};

async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(e) = tokio::signal::ctrl_c().await {
            tracing::warn!(error = %e, "Failed to listen for Ctrl+C signal");
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut s) => {
                s.recv().await;
            }
            Err(e) => tracing::warn!(error = %e, "Failed to install SIGTERM handler"),
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
    tracing::info!("Received shutdown signal, shutting down gracefully...");
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = AppConfig::load();
    telemetry::init_tracing(&config.environment);

    tracing::info!(
        environment = %config.environment,
        host = %config.host,
        port = config.port,
        allowed_origins = ?config.allowed_origins,
        "Starting application"
    );

    let pool = db::create_pool(&config.database_url).await?;
    db::run_migrations(&pool).await?;

    let app_state = AppState {
        pool: pool.clone(),
        config: config.clone(),
    };
    let app = routes::create_router(app_state);

    let address = format!("{}:{}", config.host, config.port);
    let listener = tokio::net::TcpListener::bind(&address).await?;
    tracing::info!("Server running on http://{}", address);
    
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    tracing::info!("Closing database pool...");
    pool.close().await;
    tracing::info!("Shutdown complete.");

    Ok(())
}
