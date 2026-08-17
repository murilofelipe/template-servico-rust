use axum::{
    http::Uri,
    middleware,
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use std::time::Duration;
use tower_http::{
    timeout::TimeoutLayer,
    trace::{DefaultMakeSpan, DefaultOnFailure, DefaultOnRequest, DefaultOnResponse, TraceLayer},
    LatencyUnit,
};
use tracing::Level;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::{
    error::ProblemDetails,
    handlers,
    models::{CreateUserPayload, User},
    security::{build_cors_layer, security_headers_middleware},
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
    components(schemas(
        User,
        CreateUserPayload,
        crate::error::ProblemDetails,
        crate::error::InvalidParam
    ))
)]
struct ApiDoc;

/// Fallback handler for unmatched routes, returning RFC 7807 Problem Details.
pub async fn fallback_404_handler(uri: Uri) -> impl IntoResponse {
    tracing::info!(uri = %uri, "Route not found");
    ProblemDetails::not_found(format!("The requested endpoint '{uri}' was not found."))
}

pub fn create_router(state: AppState) -> Router {
    let cors = build_cors_layer(&state.config);

    let trace_layer = TraceLayer::new_for_http()
        .make_span_with(
            DefaultMakeSpan::new()
                .level(Level::INFO)
                .include_headers(false),
        )
        .on_request(DefaultOnRequest::new().level(Level::INFO))
        .on_response(
            DefaultOnResponse::new()
                .level(Level::INFO)
                .latency_unit(LatencyUnit::Millis),
        )
        .on_failure(
            DefaultOnFailure::new()
                .level(Level::ERROR)
                .latency_unit(LatencyUnit::Millis),
        );

    Router::new()
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .route("/health", get(handlers::health_check))
        .route(
            "/users",
            post(handlers::create_user).get(handlers::list_users),
        )
        .route("/users/:id", get(handlers::get_user))
        .fallback(fallback_404_handler)
        .layer(trace_layer)
        .layer(cors)
        .layer(middleware::from_fn(security_headers_middleware))
        .layer(TimeoutLayer::with_status_code(
            axum::http::StatusCode::REQUEST_TIMEOUT,
            Duration::from_secs(10),
        ))
        .with_state(state)
}
