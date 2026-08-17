use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use sqlx::postgres::PgPoolOptions;
use std::str::FromStr;
use template_servico_rust::config::{AppConfig, AppEnvironment};
use template_servico_rust::telemetry::try_init_tracing;
use tower::ServiceExt;

#[test]
fn test_app_environment_parsing() {
    assert_eq!(
        AppEnvironment::from_str("production"),
        Ok(AppEnvironment::Production)
    );
    assert_eq!(
        AppEnvironment::from_str("prod"),
        Ok(AppEnvironment::Production)
    );
    assert_eq!(
        AppEnvironment::from_str("PRODUCTION"),
        Ok(AppEnvironment::Production)
    );
    assert_eq!(
        AppEnvironment::from_str("PROD"),
        Ok(AppEnvironment::Production)
    );
    assert_eq!(
        AppEnvironment::from_str("  prod  "),
        Ok(AppEnvironment::Production)
    );

    assert_eq!(
        AppEnvironment::from_str("development"),
        Ok(AppEnvironment::Development)
    );
    assert_eq!(
        AppEnvironment::from_str("dev"),
        Ok(AppEnvironment::Development)
    );
    assert_eq!(
        AppEnvironment::from_str("DEVELOPMENT"),
        Ok(AppEnvironment::Development)
    );
    assert_eq!(
        AppEnvironment::from_str("DEV"),
        Ok(AppEnvironment::Development)
    );
    assert_eq!(
        AppEnvironment::from_str("  dev\n"),
        Ok(AppEnvironment::Development)
    );

    assert_eq!(AppEnvironment::from_str("test"), Ok(AppEnvironment::Test));
    assert_eq!(AppEnvironment::from_str("TEST"), Ok(AppEnvironment::Test));
    assert_eq!(
        AppEnvironment::from_str("  TEST\t"),
        Ok(AppEnvironment::Test)
    );

    assert!(AppEnvironment::from_str("unknown_env").is_err());
    assert!(AppEnvironment::from_str("staging").is_err());
    assert!(AppEnvironment::from_str("").is_err());
}

#[test]
fn test_app_environment_display() {
    assert_eq!(AppEnvironment::Production.to_string(), "production");
    assert_eq!(AppEnvironment::Development.to_string(), "development");
    assert_eq!(AppEnvironment::Test.to_string(), "test");
}

#[test]
fn test_app_environment_predicates() {
    let prod = AppEnvironment::Production;
    assert!(prod.is_production());
    assert!(!prod.is_development());
    assert!(!prod.is_test());

    let dev = AppEnvironment::Development;
    assert!(dev.is_development());
    assert!(!dev.is_production());
    assert!(!dev.is_test());

    let test_env = AppEnvironment::Test;
    assert!(test_env.is_test());
    assert!(!test_env.is_production());
    assert!(!test_env.is_development());
}

#[test]
fn test_app_config_json_deserialization() {
    let raw = r#"{
        "database_url": "postgres://localhost:5432/db",
        "port": 4000,
        "host": "127.0.0.1",
        "environment": "production"
    }"#;

    let config: AppConfig = serde_json::from_str(raw).unwrap_or_else(|_| AppConfig {
        database_url: String::new(),
        port: 0,
        host: String::new(),
        environment: AppEnvironment::Development,
        allowed_origins: Vec::new(),
        otel_service_name: "test".to_string(),
        otlp_endpoint: None,
    });

    assert_eq!(config.database_url, "postgres://localhost:5432/db");
    assert_eq!(config.port, 4000);
    assert_eq!(config.host, "127.0.0.1");
    assert_eq!(config.environment, AppEnvironment::Production);

    let raw_alias = r#"{
        "database_url": "postgres://localhost:5432/db",
        "port": 4000,
        "host": "127.0.0.1",
        "environment": "prod"
    }"#;
    let config_alias: AppConfig = serde_json::from_str(raw_alias).unwrap_or_else(|_| AppConfig {
        database_url: String::new(),
        port: 0,
        host: String::new(),
        environment: AppEnvironment::Development,
        allowed_origins: Vec::new(),
        otel_service_name: "test".to_string(),
        otlp_endpoint: None,
    });
    assert_eq!(config_alias.environment, AppEnvironment::Production);

    let raw_prod_upper = r#"{
        "database_url": "postgres://localhost:5432/db",
        "port": 4000,
        "host": "127.0.0.1",
        "environment": "PROD"
    }"#;
    let config_prod_upper: AppConfig =
        serde_json::from_str(raw_prod_upper).unwrap_or_else(|_| AppConfig {
            database_url: String::new(),
            port: 0,
            host: String::new(),
            environment: AppEnvironment::Development,
            allowed_origins: Vec::new(),
        });
    assert_eq!(config_prod_upper.environment, AppEnvironment::Production);

    let raw_dev = r#"{
        "database_url": "postgres://localhost:5432/db",
        "port": 4000,
        "host": "127.0.0.1",
        "environment": "dev"
    }"#;
    let config_dev: AppConfig = serde_json::from_str(raw_dev).unwrap_or_else(|_| AppConfig {
        database_url: String::new(),
        port: 0,
        host: String::new(),
        environment: AppEnvironment::Production,
        allowed_origins: Vec::new(),
        otel_service_name: "test".to_string(),
        otlp_endpoint: None,
    });
    assert_eq!(config_dev.environment, AppEnvironment::Development);

    let raw_test = r#"{
        "database_url": "postgres://localhost:5432/db",
        "port": 4000,
        "host": "127.0.0.1",
        "environment": "TEST"
    }"#;
    let config_test: AppConfig = serde_json::from_str(raw_test).unwrap_or_else(|_| AppConfig {
        database_url: String::new(),
        port: 0,
        host: String::new(),
        environment: AppEnvironment::Development,
        allowed_origins: Vec::new(),
        otel_service_name: "test".to_string(),
        otlp_endpoint: None,
    });
    assert_eq!(config_test.environment, AppEnvironment::Test);
}

#[test]
fn test_telemetry_try_init() {
    // Calling try_init_tracing in test environment should not panic
    let result_dev = try_init_tracing(&AppEnvironment::Development, None, None);
    let _ = result_dev;

    let result_prod = try_init_tracing(&AppEnvironment::Production, None, None);
    let _ = result_prod;

    let result_test = try_init_tracing(&AppEnvironment::Test, None, None);
    let _ = result_test;
}

#[tokio::test]
async fn test_routes_middleware_integration() -> Result<(), Box<dyn std::error::Error>> {
    let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgres://postgres:postgres@localhost:5432/template_servico_rust_test".to_string()
    });

    let pool = PgPoolOptions::new()
        .connect_lazy(&database_url)
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

    let config = AppConfig {
        database_url: database_url.clone(),
        port: 3000,
        host: "0.0.0.0".to_string(),
        environment: AppEnvironment::Test,
        allowed_origins: Vec::new(),
        otel_service_name: "test".to_string(),
        otlp_endpoint: None,
    };

    let state = template_servico_rust::state::AppState { pool, config };
    let app = template_servico_rust::routes::create_router(state);

    // Test OpenAPI route (does not require DB connection)
    let openapi_request = Request::builder()
        .method("GET")
        .uri("/api-docs/openapi.json")
        .body(Body::empty())?;

    let openapi_response = app.clone().oneshot(openapi_request).await?;
    assert_eq!(openapi_response.status(), StatusCode::OK);

    // Test Health route through TraceLayer middleware
    let health_request = Request::builder()
        .method("GET")
        .uri("/health")
        .body(Body::empty())?;

    let health_response = app.clone().oneshot(health_request).await?;
    assert!(
        health_response.status() == StatusCode::OK
            || health_response.status() == StatusCode::SERVICE_UNAVAILABLE,
        "Expected OK or SERVICE_UNAVAILABLE, got {}",
        health_response.status()
    );

    // Test 404 route through TraceLayer middleware
    let not_found_request = Request::builder()
        .method("GET")
        .uri("/non-existent-route")
        .body(Body::empty())?;

    let not_found_response = app.clone().oneshot(not_found_request).await?;
    assert_eq!(not_found_response.status(), StatusCode::NOT_FOUND);

    // Test Method Not Allowed (405) through TraceLayer middleware
    let method_not_allowed_request = Request::builder()
        .method("DELETE")
        .uri("/health")
        .body(Body::empty())?;

    let method_not_allowed_response = app.oneshot(method_not_allowed_request).await?;
    assert_eq!(
        method_not_allowed_response.status(),
        StatusCode::METHOD_NOT_ALLOWED
    );

    Ok(())
}

#[test]
fn test_app_environment_default_value() {
    assert_eq!(AppEnvironment::default(), AppEnvironment::Development);
}

#[test]
fn test_app_environment_serde_roundtrip() {
    let prod_json = serde_json::to_string(&AppEnvironment::Production).unwrap_or_default();
    assert_eq!(prod_json, "\"production\"");
    let prod_parsed: Result<AppEnvironment, _> = serde_json::from_str(&prod_json);
    assert_eq!(prod_parsed.ok(), Some(AppEnvironment::Production));

    let dev_json = serde_json::to_string(&AppEnvironment::Development).unwrap_or_default();
    assert_eq!(dev_json, "\"development\"");
    let dev_parsed: Result<AppEnvironment, _> = serde_json::from_str(&dev_json);
    assert_eq!(dev_parsed.ok(), Some(AppEnvironment::Development));

    let test_json = serde_json::to_string(&AppEnvironment::Test).unwrap_or_default();
    assert_eq!(test_json, "\"test\"");
    let test_parsed: Result<AppEnvironment, _> = serde_json::from_str(&test_json);
    assert_eq!(test_parsed.ok(), Some(AppEnvironment::Test));
}

#[test]
fn test_init_tracing_function() {
    template_servico_rust::telemetry::init_tracing(&AppEnvironment::Development, None, None);
    template_servico_rust::telemetry::init_tracing(&AppEnvironment::Production, None, None);
    template_servico_rust::telemetry::init_tracing(&AppEnvironment::Test, None, None);
}
