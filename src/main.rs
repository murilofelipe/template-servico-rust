use template_servico_rust::{config::AppConfig, db, routes, state::AppState, telemetry};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    telemetry::init_tracing();
    let config = AppConfig::load();

    let pool = db::create_pool(&config.database_url).await?;
    db::run_migrations(&pool).await?;

    let app_state = AppState {
        pool,
        config: config.clone(),
    };
    let app = routes::create_router(app_state);

    let address = format!("{}:{}", config.host, config.port);
    let listener = tokio::net::TcpListener::bind(&address).await?;
    tracing::info!("Server running on http://{}", address);
    axum::serve(listener, app).await?;

    Ok(())
}
