use axum::{
    body::Body,
    http::{header, Request, StatusCode},
    response::IntoResponse,
    routing::get,
    Router,
};
use serde_json::Value;
use sqlx::postgres::PgPoolOptions;
use template_servico_rust::{
    config::{AppConfig, AppEnvironment},
    error::{AppError, AppResult, InvalidParam, ProblemDetails},
    routes::create_router,
    state::AppState,
};
use tower::ServiceExt;
use uuid::Uuid;

async fn setup_test_app() -> Result<Router, Box<dyn std::error::Error>> {
    let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgres://postgres:postgres@localhost:5432/template_servico_rust_test".to_string()
    });

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .acquire_timeout(std::time::Duration::from_secs(10))
        .connect(&database_url)
        .await?;

    template_servico_rust::db::run_migrations(&pool).await?;

    let config = AppConfig {
        database_url: database_url.clone(),
        port: 3000,
        host: "0.0.0.0".to_string(),
        environment: AppEnvironment::Test,
    };
    let state = AppState { pool, config };
    Ok(create_router(state))
}

async fn send_request(
    app: &Router,
    method: &str,
    uri: &str,
    body: Option<Value>,
) -> Result<(StatusCode, header::HeaderMap, Value), Box<dyn std::error::Error>> {
    let mut req_builder = Request::builder().method(method).uri(uri);

    let req_body = if let Some(json_body) = body {
        req_builder = req_builder.header(header::CONTENT_TYPE, "application/json");
        Body::from(serde_json::to_string(&json_body)?)
    } else {
        Body::empty()
    };

    let request = req_builder.body(req_body)?;
    let response = app.clone().oneshot(request).await?;
    let status = response.status();
    let headers = response.headers().clone();

    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await?;
    let json_val: Value = if body_bytes.is_empty() {
        Value::Null
    } else {
        match serde_json::from_slice(&body_bytes) {
            Ok(v) => v,
            Err(_) => Value::String(String::from_utf8_lossy(&body_bytes).into_owned()),
        }
    };

    Ok((status, headers, json_val))
}

#[tokio::test]
async fn test_rfc7807_bad_request_empty_name() -> Result<(), Box<dyn std::error::Error>> {
    let app = setup_test_app().await?;
    let payload = serde_json::json!({
        "name": "   ",
        "email": "user@example.com"
    });

    let (status, headers, body) = send_request(&app, "POST", "/users", Some(payload)).await?;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(
        headers
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok()),
        Some("application/problem+json")
    );
    assert_eq!(body["type"], "urn:problem-type:bad-request");
    assert_eq!(body["title"], "Bad Request");
    assert_eq!(body["status"], 400);
    assert!(
        body["detail"]
            .as_str()
            .unwrap_or_default()
            .contains("cannot be empty"),
        "Expected detail to explain that name cannot be empty"
    );

    Ok(())
}

#[tokio::test]
async fn test_rfc7807_bad_request_invalid_email() -> Result<(), Box<dyn std::error::Error>> {
    let app = setup_test_app().await?;
    let payload = serde_json::json!({
        "name": "Valid Name",
        "email": "invalid-email-no-at"
    });

    let (status, headers, body) = send_request(&app, "POST", "/users", Some(payload)).await?;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(
        headers
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok()),
        Some("application/problem+json")
    );
    assert_eq!(body["type"], "urn:problem-type:bad-request");
    assert_eq!(body["title"], "Bad Request");
    assert_eq!(body["status"], 400);
    assert!(
        body["detail"]
            .as_str()
            .unwrap_or_default()
            .contains("valid email"),
        "Expected detail to mention valid email requirement"
    );

    Ok(())
}

#[tokio::test]
async fn test_rfc7807_not_found_user() -> Result<(), Box<dyn std::error::Error>> {
    let app = setup_test_app().await?;
    let non_existent_id = Uuid::new_v4();
    let uri = format!("/users/{non_existent_id}");

    let (status, headers, body) = send_request(&app, "GET", &uri, None).await?;

    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(
        headers
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok()),
        Some("application/problem+json")
    );
    assert_eq!(body["type"], "urn:problem-type:not-found");
    assert_eq!(body["title"], "Not Found");
    assert_eq!(body["status"], 404);
    assert!(
        body["detail"]
            .as_str()
            .unwrap_or_default()
            .contains("not found"),
        "Expected detail to mention user not found"
    );

    Ok(())
}

#[tokio::test]
async fn test_rfc7807_conflict_duplicate_email() -> Result<(), Box<dyn std::error::Error>> {
    let app = setup_test_app().await?;
    let unique_email = format!("rfc7807-conflict-{}@example.com", Uuid::new_v4());
    let payload = serde_json::json!({
        "name": "Conflict User",
        "email": unique_email
    });

    let (status1, _, _) = send_request(&app, "POST", "/users", Some(payload.clone())).await?;
    assert_eq!(status1, StatusCode::CREATED);

    let (status2, headers2, body2) = send_request(&app, "POST", "/users", Some(payload)).await?;

    assert_eq!(status2, StatusCode::CONFLICT);
    assert_eq!(
        headers2
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok()),
        Some("application/problem+json")
    );
    assert_eq!(body2["type"], "urn:problem-type:conflict");
    assert_eq!(body2["title"], "Conflict");
    assert_eq!(body2["status"], 409);
    assert!(
        body2["detail"]
            .as_str()
            .unwrap_or_default()
            .contains("already exists"),
        "Expected conflict detail explaining record already exists"
    );

    Ok(())
}

#[tokio::test]
async fn test_rfc7807_500_hides_internal_technical_details(
) -> Result<(), Box<dyn std::error::Error>> {
    async fn forced_internal_error_handler() -> AppResult<String> {
        Err(AppError::Internal(
            "FATAL: connection to server at 192.168.1.50:5432 failed: password authentication failed for user 'secret_admin'".to_string(),
        ))
    }

    let router = Router::new().route("/force-500", get(forced_internal_error_handler));
    let (status, headers, body) = send_request(&router, "GET", "/force-500", None).await?;

    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(
        headers
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok()),
        Some("application/problem+json")
    );
    assert_eq!(body["type"], "urn:problem-type:internal-server-error");
    assert_eq!(body["title"], "Internal Server Error");
    assert_eq!(body["status"], 500);
    assert_eq!(
        body["detail"],
        "An unexpected internal server error occurred. Please try again later."
    );

    let raw_response = body.to_string();
    assert!(!raw_response.contains("FATAL"));
    assert!(!raw_response.contains("192.168.1.50"));
    assert!(!raw_response.contains("password"));
    assert!(!raw_response.contains("secret_admin"));
    assert!(!raw_response.contains("5432"));

    Ok(())
}

#[tokio::test]
async fn test_rfc7807_service_unavailable_response() -> Result<(), Box<dyn std::error::Error>> {
    async fn forced_503_handler() -> AppResult<String> {
        Err(AppError::ServiceUnavailable(
            "Database maintenance in progress".to_string(),
        ))
    }

    let router = Router::new().route("/force-503", get(forced_503_handler));
    let (status, headers, body) = send_request(&router, "GET", "/force-503", None).await?;

    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        headers
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok()),
        Some("application/problem+json")
    );
    assert_eq!(body["type"], "urn:problem-type:service-unavailable");
    assert_eq!(body["title"], "Service Unavailable");
    assert_eq!(body["status"], 503);
    assert_eq!(body["detail"], "Database maintenance in progress");

    Ok(())
}

#[test]
fn test_rfc7807_problem_details_builder_methods() {
    let problem = ProblemDetails::bad_request("Validation failed")
        .with_type("urn:problem-type:validation-error")
        .with_instance("/api/v1/users/123")
        .with_invalid_params(vec![
            InvalidParam {
                name: "email".to_string(),
                reason: "Invalid email syntax".to_string(),
            },
            InvalidParam {
                name: "age".to_string(),
                reason: "Must be positive".to_string(),
            },
        ]);

    let response = problem.into_response();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok()),
        Some("application/problem+json")
    );
}

#[tokio::test]
async fn test_rfc7807_unrouted_fallback_404() -> Result<(), Box<dyn std::error::Error>> {
    let app = setup_test_app().await?;
    let (status, headers, body) =
        send_request(&app, "GET", "/non-existent-unmatched-endpoint", None).await?;

    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(
        headers
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok()),
        Some("application/problem+json")
    );
    assert_eq!(body["type"], "urn:problem-type:not-found");
    assert_eq!(body["title"], "Not Found");
    assert_eq!(body["status"], 404);
    assert!(
        body["detail"]
            .as_str()
            .unwrap_or_default()
            .contains("/non-existent-unmatched-endpoint"),
        "Expected detail to mention the requested endpoint"
    );

    Ok(())
}

#[tokio::test]
async fn test_rfc7807_malformed_json_extractor_rejection() -> Result<(), Box<dyn std::error::Error>>
{
    let app = setup_test_app().await?;
    let malformed_raw_json = r#"{"name": "Malformed User", "email": "#;

    let mut req_builder = Request::builder().method("POST").uri("/users");
    req_builder = req_builder.header(header::CONTENT_TYPE, "application/json");
    let request = req_builder.body(Body::from(malformed_raw_json.to_string()))?;

    let response = app.clone().oneshot(request).await?;
    let status = response.status();
    let headers = response.headers().clone();

    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await?;
    let json_val: Value = serde_json::from_slice(&body_bytes)?;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(
        headers
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok()),
        Some("application/problem+json")
    );
    assert_eq!(json_val["type"], "urn:problem-type:bad-request");
    assert_eq!(json_val["title"], "Bad Request");
    assert_eq!(json_val["status"], 400);
    assert!(
        json_val["detail"]
            .as_str()
            .unwrap_or_default()
            .to_lowercase()
            .contains("json"),
        "Expected detail to indicate JSON parsing failure"
    );

    Ok(())
}

#[tokio::test]
async fn test_rfc7807_invalid_path_extractor_rejection() -> Result<(), Box<dyn std::error::Error>> {
    let app = setup_test_app().await?;
    let (status, headers, body) =
        send_request(&app, "GET", "/users/not-a-valid-uuid-format", None).await?;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(
        headers
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok()),
        Some("application/problem+json")
    );
    assert_eq!(body["type"], "urn:problem-type:bad-request");
    assert_eq!(body["title"], "Bad Request");
    assert_eq!(body["status"], 400);
    assert!(
        body["detail"]
            .as_str()
            .unwrap_or_default()
            .to_lowercase()
            .contains("parameter"),
        "Expected detail to indicate path parameter error"
    );

    Ok(())
}

#[tokio::test]
async fn test_rfc7807_unauthorized_and_forbidden_responses(
) -> Result<(), Box<dyn std::error::Error>> {
    async fn unauth_handler() -> AppResult<String> {
        Err(AppError::Unauthorized(
            "Invalid or expired bearer token".to_string(),
        ))
    }
    async fn forbidden_handler() -> AppResult<String> {
        Err(AppError::Forbidden(
            "Insufficient permissions to access this resource".to_string(),
        ))
    }

    let router = Router::new()
        .route("/unauth", get(unauth_handler))
        .route("/forbidden", get(forbidden_handler));

    let (status1, headers1, body1) = send_request(&router, "GET", "/unauth", None).await?;
    assert_eq!(status1, StatusCode::UNAUTHORIZED);
    assert_eq!(
        headers1
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok()),
        Some("application/problem+json")
    );
    assert_eq!(body1["type"], "urn:problem-type:unauthorized");
    assert_eq!(body1["title"], "Unauthorized");
    assert_eq!(body1["status"], 401);

    let (status2, headers2, body2) = send_request(&router, "GET", "/forbidden", None).await?;
    assert_eq!(status2, StatusCode::FORBIDDEN);
    assert_eq!(
        headers2
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok()),
        Some("application/problem+json")
    );
    assert_eq!(body2["type"], "urn:problem-type:forbidden");
    assert_eq!(body2["title"], "Forbidden");
    assert_eq!(body2["status"], 403);

    Ok(())
}
