use axum::{
    extract::Request,
    http::{
        header::{
            CONTENT_SECURITY_POLICY, REFERRER_POLICY, STRICT_TRANSPORT_SECURITY,
            X_CONTENT_TYPE_OPTIONS, X_FRAME_OPTIONS,
        },
        HeaderName, HeaderValue,
    },
    middleware::Next,
    response::Response,
};
use tower_http::cors::{AllowOrigin, Any, CorsLayer};

use crate::config::AppConfig;

static X_XSS_PROTECTION: HeaderName = HeaderName::from_static("x-xss-protection");
static PERMISSIONS_POLICY: HeaderName = HeaderName::from_static("permissions-policy");

/// Builds a `CorsLayer` configured dynamically for the application's environment.
///
/// In **Production** mode:
/// - Cross-origin requests are only permitted for origins listed in `config.allowed_origins`.
/// - If `allowed_origins` is empty, no cross-origin requests are permitted.
/// - Methods and headers are permitted for the authorized origins.
///
/// In **Development** and **Test** mode:
/// - Permissive CORS is applied (`CorsLayer::permissive()`), allowing all origins, methods, and headers.
pub fn build_cors_layer(config: &AppConfig) -> CorsLayer {
    if config.environment.is_production() {
        let parsed_origins: Vec<HeaderValue> = config
            .allowed_origins
            .iter()
            .filter_map(|s| s.parse::<HeaderValue>().ok())
            .collect();

        CorsLayer::new()
            .allow_origin(AllowOrigin::list(parsed_origins))
            .allow_methods(Any)
            .allow_headers(Any)
    } else {
        CorsLayer::permissive()
    }
}

/// Axum middleware that adds recommended HTTP response security headers to all responses.
///
/// Injected headers:
/// - `X-Content-Type-Options: nosniff`
/// - `X-Frame-Options: DENY`
/// - `Strict-Transport-Security: max-age=31536000; includeSubDomains`
/// - `Content-Security-Policy: default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; frame-ancestors 'none';`
/// - `Referrer-Policy: strict-origin-when-cross-origin`
/// - `X-XSS-Protection: 0`
/// - `Permissions-Policy: geolocation=(), camera=(), microphone=()`
pub async fn security_headers_middleware(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();

    headers.insert(X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    headers.insert(X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    headers.insert(
        STRICT_TRANSPORT_SECURITY,
        HeaderValue::from_static("max-age=31536000; includeSubDomains"),
    );
    headers.insert(
        CONTENT_SECURITY_POLICY.clone(),
        HeaderValue::from_static(
            "default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; frame-ancestors 'none';",
        ),
    );
    headers.insert(
        REFERRER_POLICY,
        HeaderValue::from_static("strict-origin-when-cross-origin"),
    );
    headers.insert(X_XSS_PROTECTION.clone(), HeaderValue::from_static("0"));
    headers.insert(
        PERMISSIONS_POLICY.clone(),
        HeaderValue::from_static("geolocation=(), camera=(), microphone=()"),
    );

    response
}
