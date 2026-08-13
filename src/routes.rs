use axum::{
    routing::{get, post},
    Router,
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
    Router::new()
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .route("/health", get(handlers::health_check))
        .route(
            "/users",
            post(handlers::create_user).get(handlers::list_users),
        )
        .route("/users/:id", get(handlers::get_user))
        .with_state(state)
}
