use dotenvy::dotenv;
use std::env;
use template_servico_rust::{db, routes, state::AppState};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();

    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/template_db".to_string());

    let pool = db::create_pool(&database_url).await?;
    db::run_migrations(&pool).await?;

    let state = AppState { pool };
    let app = routes::create_router(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    println!("Server running on http://0.0.0.0:3000");
    axum::serve(listener, app).await?;

    Ok(())
}
