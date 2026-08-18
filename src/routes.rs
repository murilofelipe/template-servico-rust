use axum::{
    http::Uri,
    middleware,
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use axum_prometheus::PrometheusMetricLayer;
use metrics_exporter_prometheus::PrometheusHandle;
use std::sync::OnceLock;
use std::time::Duration;
use tower_http::{
    timeout::TimeoutLayer,
    trace::{DefaultOnFailure, DefaultOnRequest, DefaultOnResponse, TraceLayer},
    LatencyUnit,
};
use tracing::Level;
use tracing_opentelemetry::OpenTelemetrySpanExt;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::{
    auth::jwt_auth_middleware,
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

pub async fn fallback_404_handler(uri: Uri) -> impl IntoResponse {
    tracing::info!(uri = %uri, "Route not found");
    ProblemDetails::not_found(format!("The requested endpoint '{uri}' was not found."))
}

struct HeaderExtractor<'a>(&'a axum::http::HeaderMap);
impl<'a> opentelemetry::propagation::Extractor for HeaderExtractor<'a> {
    fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).and_then(|v| v.to_str().ok())
    }
    fn keys(&self) -> Vec<&str> {
        self.0.keys().map(|k| k.as_str()).collect()
    }
}

/// Recorder global do Prometheus — instalado apenas uma vez para evitar pânico em testes
/// onde `create_router` é chamado múltiplas vezes no mesmo processo.
static PROMETHEUS_HANDLE: OnceLock<PrometheusHandle> = OnceLock::new();

pub fn create_router(state: AppState) -> Router {
    let cors = build_cors_layer(&state.config);

    let trace_layer = TraceLayer::new_for_http()
        .make_span_with(|request: &axum::http::Request<_>| {
            let parent_context = opentelemetry::global::get_text_map_propagator(|propagator| {
                propagator.extract(&HeaderExtractor(request.headers()))
            });

            let span = tracing::info_span!(
                "http_request",
                method = %request.method(),
                uri = %request.uri(),
                version = ?request.version(),
                trace_id = tracing::field::Empty,
            );

            span.set_parent(parent_context);

            span
        })
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

    // Rotas protegidas por JWT (opt-in: transparente se JWKS_URL não configurada)
    let protected_routes = Router::new()
        .route(
            "/users",
            post(handlers::create_user).get(handlers::list_users),
        )
        .route("/users/:id", get(handlers::get_user))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            jwt_auth_middleware,
        ));

    // Inicializa o recorder do Prometheus apenas uma vez (OnceLock).
    // Chamadas subsequentes (e.g. em testes) reutilizam o handle existente e criam
    // um novo layer via Default, que NÃO tenta reinstalar o recorder global.
    let metric_handle = PROMETHEUS_HANDLE
        .get_or_init(|| {
            let (_, h) = PrometheusMetricLayer::pair();
            h
        })
        .clone();
    let prometheus_layer = PrometheusMetricLayer::default();

    // Rotas públicas (sem autenticação)
    let public_routes = Router::new()
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .route("/health", get(handlers::health_check))
        .route("/metrics", get(|| async move { metric_handle.render() }));

    Router::new()
        .merge(public_routes)
        .merge(protected_routes)
        .fallback(fallback_404_handler)
        .layer(prometheus_layer)
        .layer(trace_layer)
        .layer(cors)
        .layer(middleware::from_fn(security_headers_middleware))
        .layer(TimeoutLayer::with_status_code(
            axum::http::StatusCode::REQUEST_TIMEOUT,
            Duration::from_secs(10),
        ))
        .with_state(state)
}
