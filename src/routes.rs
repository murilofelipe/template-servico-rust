use axum::{
    routing::{get, post},
    Router,
};

use crate::{handlers, state::AppState};

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(handlers::health_check))
        .route(
            "/users",
            post(handlers::create_user).get(handlers::list_users),
        )
        .route("/users/:id", get(handlers::get_user))
        .with_state(state)
}
