use axum::{
    routing::{get, post},
    Router,
};
use std::time::Duration;
use tower_http::{
    cors::{Any, CorsLayer},
    timeout::TimeoutLayer,
    trace::TraceLayer,
};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::{
    handlers,
    models::{CreateUserPayload, User},
    state::AppState,
};

#[derive(OpenApi)]
#[openapi(
    paths(
        handlers::health_check,
        handlers::create_user,
        handlers::list_users,
        handlers::get_user
    ),
    components(schemas(User, CreateUserPayload))
)]
struct ApiDoc;

pub fn create_router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .route("/health", get(handlers::health_check))
        .route(
            "/users",
            post(handlers::create_user).get(handlers::list_users),
        )
        .route("/users/:id", get(handlers::get_user))
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .layer(TimeoutLayer::with_status_code(
            Duration::from_secs(10),
            axum::http::StatusCode::REQUEST_TIMEOUT,
        ))
        .with_state(state)
}
