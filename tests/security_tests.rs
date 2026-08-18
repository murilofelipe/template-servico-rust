use axum::{
    body::Body,
    http::{header, Method, Request, StatusCode},
};
use sqlx::postgres::PgPoolOptions;
use template_servico_rust::{
    config::{parse_allowed_origins, AppConfig, AppEnvironment},
    routes::create_router,
    state::AppState,
};
use tower::ServiceExt;

fn create_test_state(
    environment: AppEnvironment,
    allowed_origins: Vec<String>,
) -> Result<AppState, Box<dyn std::error::Error>> {
    let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgres://postgres:postgres@localhost:5432/template_servico_rust_test".to_string()
    });

    let pool = PgPoolOptions::new()
        .connect_lazy(&database_url)
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

    let config = AppConfig {
        database_url,
        port: 3000,
        host: "127.0.0.1".to_string(),
        environment,
        allowed_origins,
        otel_service_name: "test".to_string(),
        otlp_endpoint: None,
        jwks_url: None,
        jwt_audience: None,
        jwt_issuer: None,
    };

    Ok(AppState {
        pool,
        config,
        jwks_cache: None,
    })
}

#[tokio::test]
async fn test_production_cors_allows_configured_origins() -> Result<(), Box<dyn std::error::Error>>
{
    let allowed_origins = vec![
        "https://app.example.com".to_string(),
        "https://dashboard.example.com".to_string(),
    ];
    let state = create_test_state(AppEnvironment::Production, allowed_origins)?;
    let app = create_router(state);

    // 1. GET request with first allowed origin
    let req1 = Request::builder()
        .method("GET")
        .uri("/health")
        .header(header::ORIGIN, "https://app.example.com")
        .body(Body::empty())?;

    let res1 = app.clone().oneshot(req1).await?;
    assert_eq!(
        res1.headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .and_then(|v| v.to_str().ok()),
        Some("https://app.example.com")
    );

    // 2. GET request with second allowed origin
    let req2 = Request::builder()
        .method("GET")
        .uri("/health")
        .header(header::ORIGIN, "https://dashboard.example.com")
        .body(Body::empty())?;

    let res2 = app.clone().oneshot(req2).await?;
    assert_eq!(
        res2.headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .and_then(|v| v.to_str().ok()),
        Some("https://dashboard.example.com")
    );

    // 3. Preflight OPTIONS request for allowed origin
    let preflight_req = Request::builder()
        .method(Method::OPTIONS)
        .uri("/users")
        .header(header::ORIGIN, "https://app.example.com")
        .header(header::ACCESS_CONTROL_REQUEST_METHOD, "POST")
        .header(
            header::ACCESS_CONTROL_REQUEST_HEADERS,
            "content-type,authorization",
        )
        .body(Body::empty())?;

    let preflight_res = app.oneshot(preflight_req).await?;
    assert_eq!(preflight_res.status(), StatusCode::OK);
    assert_eq!(
        preflight_res
            .headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .and_then(|v| v.to_str().ok()),
        Some("https://app.example.com")
    );

    Ok(())
}

#[tokio::test]
async fn test_production_cors_blocks_unauthorized_origins() -> Result<(), Box<dyn std::error::Error>>
{
    let allowed_origins = vec!["https://app.example.com".to_string()];
    let state = create_test_state(AppEnvironment::Production, allowed_origins)?;
    let app = create_router(state);

    // 1. GET request with unauthorized origin
    let req = Request::builder()
        .method("GET")
        .uri("/health")
        .header(header::ORIGIN, "https://evil.attacker.com")
        .body(Body::empty())?;

    let res = app.clone().oneshot(req).await?;
    assert!(
        res.headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .is_none(),
        "Unauthorized origin should not receive Access-Control-Allow-Origin header"
    );

    // 2. Preflight OPTIONS request with unauthorized origin
    let preflight_req = Request::builder()
        .method(Method::OPTIONS)
        .uri("/users")
        .header(header::ORIGIN, "https://evil.attacker.com")
        .header(header::ACCESS_CONTROL_REQUEST_METHOD, "POST")
        .body(Body::empty())?;

    let preflight_res = app.oneshot(preflight_req).await?;
    assert!(
        preflight_res
            .headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .is_none(),
        "Unauthorized preflight should not receive Access-Control-Allow-Origin header"
    );

    Ok(())
}

#[tokio::test]
async fn test_production_cors_with_empty_origins_blocks_all(
) -> Result<(), Box<dyn std::error::Error>> {
    let state = create_test_state(AppEnvironment::Production, Vec::new())?;
    let app = create_router(state);

    let req = Request::builder()
        .method("GET")
        .uri("/health")
        .header(header::ORIGIN, "https://any.example.com")
        .body(Body::empty())?;

    let res = app.oneshot(req).await?;
    assert!(
        res.headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .is_none(),
        "When allowed_origins is empty in production, all cross-origin requests should be denied"
    );

    Ok(())
}

#[tokio::test]
async fn test_development_cors_is_permissive() -> Result<(), Box<dyn std::error::Error>> {
    let state = create_test_state(AppEnvironment::Development, Vec::new())?;
    let app = create_router(state);

    // Any origin should receive CORS allow header in development
    let req1 = Request::builder()
        .method("GET")
        .uri("/health")
        .header(header::ORIGIN, "http://localhost:3000")
        .body(Body::empty())?;

    let res1 = app.clone().oneshot(req1).await?;
    let allow_origin1 = res1
        .headers()
        .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
        .and_then(|v| v.to_str().ok());
    assert!(
        allow_origin1 == Some("*") || allow_origin1 == Some("http://localhost:3000"),
        "Expected permissive allow origin, got: {:?}",
        allow_origin1
    );

    let req2 = Request::builder()
        .method("GET")
        .uri("/health")
        .header(header::ORIGIN, "https://random-frontend.net")
        .body(Body::empty())?;

    let res2 = app.clone().oneshot(req2).await?;
    let allow_origin2 = res2
        .headers()
        .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
        .and_then(|v| v.to_str().ok());
    assert!(
        allow_origin2 == Some("*") || allow_origin2 == Some("https://random-frontend.net"),
        "Expected permissive allow origin, got: {:?}",
        allow_origin2
    );

    // Preflight in development
    let preflight_req = Request::builder()
        .method(Method::OPTIONS)
        .uri("/users")
        .header(header::ORIGIN, "http://localhost:5173")
        .header(header::ACCESS_CONTROL_REQUEST_METHOD, "POST")
        .body(Body::empty())?;

    let preflight_res = app.oneshot(preflight_req).await?;
    assert_eq!(preflight_res.status(), StatusCode::OK);
    assert!(preflight_res
        .headers()
        .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
        .is_some());

    Ok(())
}

#[tokio::test]
async fn test_test_environment_cors_is_permissive() -> Result<(), Box<dyn std::error::Error>> {
    let state = create_test_state(AppEnvironment::Test, Vec::new())?;
    let app = create_router(state);

    let req = Request::builder()
        .method("GET")
        .uri("/health")
        .header(header::ORIGIN, "https://test-runner.local")
        .body(Body::empty())?;

    let res = app.oneshot(req).await?;
    let allow_origin = res
        .headers()
        .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
        .and_then(|v| v.to_str().ok());
    assert!(
        allow_origin == Some("*") || allow_origin == Some("https://test-runner.local"),
        "Expected permissive allow origin in test environment, got: {:?}",
        allow_origin
    );

    Ok(())
}

#[tokio::test]
async fn test_security_headers_present_on_all_responses() -> Result<(), Box<dyn std::error::Error>>
{
    let state = create_test_state(AppEnvironment::Development, Vec::new())?;
    let app = create_router(state);

    let assert_security_headers = |headers: &axum::http::HeaderMap, context: &str| {
        assert_eq!(
            headers
                .get(header::X_CONTENT_TYPE_OPTIONS)
                .and_then(|v| v.to_str().ok()),
            Some("nosniff"),
            "[{context}] Missing or incorrect X-Content-Type-Options"
        );

        assert_eq!(
            headers
                .get(header::X_FRAME_OPTIONS)
                .and_then(|v| v.to_str().ok()),
            Some("DENY"),
            "[{context}] Missing or incorrect X-Frame-Options"
        );

        assert_eq!(
            headers
                .get(header::STRICT_TRANSPORT_SECURITY)
                .and_then(|v| v.to_str().ok()),
            Some("max-age=31536000; includeSubDomains"),
            "[{context}] Missing or incorrect Strict-Transport-Security"
        );

        assert_eq!(
            headers
                .get("content-security-policy")
                .and_then(|v| v.to_str().ok()),
            Some("default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; frame-ancestors 'none';"),
            "[{context}] Missing or incorrect Content-Security-Policy"
        );

        assert_eq!(
            headers
                .get(header::REFERRER_POLICY)
                .and_then(|v| v.to_str().ok()),
            Some("strict-origin-when-cross-origin"),
            "[{context}] Missing or incorrect Referrer-Policy"
        );

        assert_eq!(
            headers
                .get("x-xss-protection")
                .and_then(|v| v.to_str().ok()),
            Some("0"),
            "[{context}] Missing or incorrect X-XSS-Protection"
        );

        assert_eq!(
            headers
                .get("permissions-policy")
                .and_then(|v| v.to_str().ok()),
            Some("geolocation=(), camera=(), microphone=()"),
            "[{context}] Missing or incorrect Permissions-Policy"
        );
    };

    // 1. Success endpoint: /health
    let req1 = Request::builder()
        .method("GET")
        .uri("/health")
        .body(Body::empty())?;
    let res1 = app.clone().oneshot(req1).await?;
    assert_security_headers(res1.headers(), "GET /health");

    // 2. OpenAPI documentation: /api-docs/openapi.json
    let req2 = Request::builder()
        .method("GET")
        .uri("/api-docs/openapi.json")
        .body(Body::empty())?;
    let res2 = app.clone().oneshot(req2).await?;
    assert_eq!(res2.status(), StatusCode::OK);
    assert_security_headers(res2.headers(), "GET /api-docs/openapi.json");

    // 3. Swagger UI: /swagger-ui
    let req3 = Request::builder()
        .method("GET")
        .uri("/swagger-ui/")
        .body(Body::empty())?;
    let res3 = app.clone().oneshot(req3).await?;
    assert_security_headers(res3.headers(), "GET /swagger-ui/");

    // 4. 404 Not Found response
    let req4 = Request::builder()
        .method("GET")
        .uri("/unknown-endpoint-404")
        .body(Body::empty())?;
    let res4 = app.clone().oneshot(req4).await?;
    assert_eq!(res4.status(), StatusCode::NOT_FOUND);
    assert_security_headers(res4.headers(), "GET 404 route");

    // 5. 405 Method Not Allowed response
    let req5 = Request::builder()
        .method("DELETE")
        .uri("/health")
        .body(Body::empty())?;
    let res5 = app.clone().oneshot(req5).await?;
    assert_eq!(res5.status(), StatusCode::METHOD_NOT_ALLOWED);
    assert_security_headers(res5.headers(), "405 Method Not Allowed");

    // 6. 400 Bad Request response
    let req6 = Request::builder()
        .method("POST")
        .uri("/users")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from("{ malformed json }"))?;
    let res6 = app.clone().oneshot(req6).await?;
    assert_eq!(res6.status(), StatusCode::BAD_REQUEST);
    assert_security_headers(res6.headers(), "400 Bad Request");

    // 7. CORS Preflight OPTIONS request
    let req7 = Request::builder()
        .method(Method::OPTIONS)
        .uri("/users")
        .header(header::ORIGIN, "http://localhost:3000")
        .header(header::ACCESS_CONTROL_REQUEST_METHOD, "POST")
        .body(Body::empty())?;
    let res7 = app.clone().oneshot(req7).await?;
    assert_eq!(res7.status(), StatusCode::OK);
    assert_security_headers(res7.headers(), "OPTIONS Preflight");

    // 8. Normal request without Origin header
    let req8 = Request::builder()
        .method("GET")
        .uri("/health")
        .body(Body::empty())?;
    let res8 = app.oneshot(req8).await?;
    assert_security_headers(res8.headers(), "GET /health without Origin");

    Ok(())
}

#[tokio::test]
async fn test_production_request_without_origin_has_security_headers_and_no_cors(
) -> Result<(), Box<dyn std::error::Error>> {
    let allowed_origins = vec!["https://app.example.com".to_string()];
    let state = create_test_state(AppEnvironment::Production, allowed_origins)?;
    let app = create_router(state);

    let req = Request::builder()
        .method("GET")
        .uri("/health")
        .body(Body::empty())?;

    let res = app.oneshot(req).await?;
    assert_eq!(res.status(), StatusCode::OK);

    // Security headers must be present
    assert_eq!(
        res.headers()
            .get(header::X_CONTENT_TYPE_OPTIONS)
            .and_then(|v| v.to_str().ok()),
        Some("nosniff")
    );
    assert_eq!(
        res.headers()
            .get(header::X_FRAME_OPTIONS)
            .and_then(|v| v.to_str().ok()),
        Some("DENY")
    );
    assert_eq!(
        res.headers()
            .get(header::STRICT_TRANSPORT_SECURITY)
            .and_then(|v| v.to_str().ok()),
        Some("max-age=31536000; includeSubDomains")
    );

    // CORS headers must not be present when Origin header is absent
    assert!(
        res.headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .is_none(),
        "Production request without Origin header should not receive Access-Control-Allow-Origin"
    );

    Ok(())
}

#[test]
fn test_parse_allowed_origins_edge_cases() {
    assert_eq!(
        parse_allowed_origins("https://domain1.com, https://domain2.com"),
        vec!["https://domain1.com", "https://domain2.com"]
    );
    assert_eq!(
        parse_allowed_origins("   http://localhost:3000   "),
        vec!["http://localhost:3000"]
    );
    assert_eq!(
        parse_allowed_origins(",, , https://valid.com ,,,"),
        vec!["https://valid.com"]
    );
    assert_eq!(parse_allowed_origins(""), Vec::<String>::new());
    assert_eq!(parse_allowed_origins("   \n\t  "), Vec::<String>::new());
}

#[tokio::test]
async fn test_production_cors_ports_and_exact_match() -> Result<(), Box<dyn std::error::Error>> {
    let allowed_origins = vec!["http://localhost:8080".to_string()];
    let state = create_test_state(AppEnvironment::Production, allowed_origins)?;
    let app = create_router(state);

    // Exact port match: http://localhost:8080 -> Allowed
    let req_match = Request::builder()
        .method("GET")
        .uri("/health")
        .header(header::ORIGIN, "http://localhost:8080")
        .body(Body::empty())?;
    let res_match = app.clone().oneshot(req_match).await?;
    assert_eq!(
        res_match
            .headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .and_then(|v| v.to_str().ok()),
        Some("http://localhost:8080")
    );

    // Different port: http://localhost:3000 -> Blocked
    let req_diff_port = Request::builder()
        .method("GET")
        .uri("/health")
        .header(header::ORIGIN, "http://localhost:3000")
        .body(Body::empty())?;
    let res_diff_port = app.clone().oneshot(req_diff_port).await?;
    assert!(
        res_diff_port
            .headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .is_none(),
        "Origin with different port should not be allowed"
    );

    Ok(())
}

#[tokio::test]
async fn test_production_cors_malformed_origin_recovery() -> Result<(), Box<dyn std::error::Error>>
{
    // List includes an invalid header value containing non-ASCII / control chars, alongside a valid origin
    let allowed_origins = vec![
        "https://valid-service.internal".to_string(),
        "invalid origin with spaces and \n newline".to_string(),
    ];
    let state = create_test_state(AppEnvironment::Production, allowed_origins)?;
    let app = create_router(state);

    // Valid origin in list works properly
    let req_valid = Request::builder()
        .method("GET")
        .uri("/health")
        .header(header::ORIGIN, "https://valid-service.internal")
        .body(Body::empty())?;
    let res_valid = app.clone().oneshot(req_valid).await?;
    assert_eq!(
        res_valid
            .headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .and_then(|v| v.to_str().ok()),
        Some("https://valid-service.internal")
    );

    // Unlisted origin blocked
    let req_unlisted = Request::builder()
        .method("GET")
        .uri("/health")
        .header(header::ORIGIN, "https://other.com")
        .body(Body::empty())?;
    let res_unlisted = app.oneshot(req_unlisted).await?;
    assert!(res_unlisted
        .headers()
        .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
        .is_none());

    Ok(())
}

#[tokio::test]
async fn test_production_cors_vary_header_behavior() -> Result<(), Box<dyn std::error::Error>> {
    let allowed_origins = vec!["https://app.example.com".to_string()];
    let state = create_test_state(AppEnvironment::Production, allowed_origins)?;
    let app = create_router(state);

    let req = Request::builder()
        .method("GET")
        .uri("/health")
        .header(header::ORIGIN, "https://app.example.com")
        .body(Body::empty())?;

    let res = app.oneshot(req).await?;
    assert_eq!(res.status(), StatusCode::OK);

    // When dynamic CORS origin list is used, Vary header should be present
    let vary_header = res
        .headers()
        .get(header::VARY)
        .and_then(|v| v.to_str().ok());
    assert!(
        vary_header.is_some(),
        "Expected Vary header on CORS request with dynamic origin list"
    );

    Ok(())
}
