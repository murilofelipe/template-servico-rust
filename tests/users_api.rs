use axum::{
    body::Body,
    http::{header, Request, StatusCode},
    Router,
};
use chrono::DateTime;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tower::ServiceExt;
use uuid::Uuid;

/// DTO for creating a user in integration tests.
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateUserPayloadDto {
    pub name: String,
    pub email: String,
}

/// DTO for user responses in integration tests.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct UserResponseDto {
    pub id: Uuid,
    pub name: String,
    pub email: String,
    pub created_at: String,
    pub updated_at: String,
}

/// Helper function to initialize the Axum application router for testing.
async fn setup_test_app() -> Result<Router, Box<dyn std::error::Error>> {
    let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgres://postgres:postgres@localhost:5432/template_servico_rust_test".to_string()
    });

    let pool = template_servico_rust::db::create_pool(&database_url).await?;
    template_servico_rust::db::run_migrations(&pool).await?;

    let config = template_servico_rust::config::AppConfig {
        database_url: database_url.clone(),
        port: 3000,
        host: "0.0.0.0".to_string(),
        environment: template_servico_rust::config::AppEnvironment::Test,
        allowed_origins: Vec::new(),
        otel_service_name: "test".to_string(),
        otlp_endpoint: None,
        jwks_url: None,
        jwt_audience: None,
        jwt_issuer: None,
    };
    let state = template_servico_rust::state::AppState {
        pool,
        config,
        jwks_cache: None,
    };
    let app = template_servico_rust::routes::create_router(state);

    Ok(app)
}

/// Helper function to execute an HTTP request against the Axum router.
async fn send_request(
    app: &Router,
    method: &str,
    uri: &str,
    body: Option<Value>,
) -> Result<(StatusCode, Value), Box<dyn std::error::Error>> {
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

    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await?;
    let json_val: Value = if body_bytes.is_empty() {
        Value::Null
    } else {
        match serde_json::from_slice(&body_bytes) {
            Ok(v) => v,
            Err(_) => Value::String(String::from_utf8_lossy(&body_bytes).into_owned()),
        }
    };

    Ok((status, json_val))
}

/// Helper function to execute a raw string HTTP request against the Axum router.
async fn send_raw_request(
    app: &Router,
    method: &str,
    uri: &str,
    raw_body: &str,
    content_type: Option<&str>,
) -> Result<(StatusCode, Value), Box<dyn std::error::Error>> {
    let mut req_builder = Request::builder().method(method).uri(uri);

    if let Some(ct) = content_type {
        req_builder = req_builder.header(header::CONTENT_TYPE, ct);
    }

    let request = req_builder.body(Body::from(raw_body.to_string()))?;
    let response = app.clone().oneshot(request).await?;
    let status = response.status();

    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await?;
    let json_val: Value = if body_bytes.is_empty() {
        Value::Null
    } else {
        match serde_json::from_slice(&body_bytes) {
            Ok(v) => v,
            Err(_) => Value::String(String::from_utf8_lossy(&body_bytes).into_owned()),
        }
    };

    Ok((status, json_val))
}

// ============================================================================
// TIER 1: FEATURE COVERAGE TESTS
// ============================================================================

#[tokio::test]
async fn test_tier1_health_check_returns_200_ok() -> Result<(), Box<dyn std::error::Error>> {
    let app = setup_test_app().await?;
    let (status, _body) = send_request(&app, "GET", "/health", None).await?;

    assert_eq!(status, StatusCode::OK);
    Ok(())
}

#[tokio::test]
async fn test_tier1_create_user_returns_201_created() -> Result<(), Box<dyn std::error::Error>> {
    let app = setup_test_app().await?;
    let unique_email = format!("alice.tier1.{}@example.com", Uuid::new_v4());
    let payload = json!({
        "name": "Alice Developer",
        "email": unique_email
    });

    let (status, body) = send_request(&app, "POST", "/users", Some(payload)).await?;

    assert_eq!(status, StatusCode::CREATED);
    assert!(body.get("id").is_some());
    assert_eq!(body["name"], "Alice Developer");
    assert_eq!(body["email"], unique_email);
    assert!(body.get("created_at").is_some());
    assert!(body.get("updated_at").is_some());

    Ok(())
}

#[tokio::test]
async fn test_tier1_list_users_returns_200_ok() -> Result<(), Box<dyn std::error::Error>> {
    let app = setup_test_app().await?;
    let (status, body) = send_request(&app, "GET", "/users", None).await?;

    assert_eq!(status, StatusCode::OK);
    assert!(body.is_array());

    Ok(())
}

#[tokio::test]
async fn test_tier1_get_user_by_id_returns_200_ok() -> Result<(), Box<dyn std::error::Error>> {
    let app = setup_test_app().await?;
    let unique_email = format!("bob.tier1.{}@example.com", Uuid::new_v4());
    let payload = json!({
        "name": "Bob Test",
        "email": unique_email
    });

    let (create_status, create_body) = send_request(&app, "POST", "/users", Some(payload)).await?;
    assert_eq!(create_status, StatusCode::CREATED);

    let user_id = create_body["id"]
        .as_str()
        .ok_or("Missing ID in user creation response")?;

    let get_uri = format!("/users/{}", user_id);
    let (get_status, get_body) = send_request(&app, "GET", &get_uri, None).await?;

    assert_eq!(get_status, StatusCode::OK);
    assert_eq!(get_body["id"], user_id);
    assert_eq!(get_body["name"], "Bob Test");
    assert_eq!(get_body["email"], unique_email);

    Ok(())
}

// ============================================================================
// TIER 2: BOUNDARY & CORNER CASE TESTS
// ============================================================================

#[tokio::test]
async fn test_tier2_create_user_empty_name_returns_400_bad_request(
) -> Result<(), Box<dyn std::error::Error>> {
    let app = setup_test_app().await?;
    let payload = json!({
        "name": "",
        "email": "emptyname@example.com"
    });

    let (status, _body) = send_request(&app, "POST", "/users", Some(payload)).await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    Ok(())
}

#[tokio::test]
async fn test_tier2_create_user_invalid_email_returns_400_bad_request(
) -> Result<(), Box<dyn std::error::Error>> {
    let app = setup_test_app().await?;
    let payload = json!({
        "name": "Invalid Email User",
        "email": "not-an-email-address"
    });

    let (status, _body) = send_request(&app, "POST", "/users", Some(payload)).await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    Ok(())
}

#[tokio::test]
async fn test_tier2_create_user_duplicate_email_returns_409_conflict(
) -> Result<(), Box<dyn std::error::Error>> {
    let app = setup_test_app().await?;
    let unique_email = format!("duplicate.{}@example.com", Uuid::new_v4());
    let payload1 = json!({
        "name": "Original User",
        "email": unique_email.clone()
    });

    let (status1, _body1) = send_request(&app, "POST", "/users", Some(payload1)).await?;
    assert_eq!(status1, StatusCode::CREATED);

    let payload2 = json!({
        "name": "Duplicate User",
        "email": unique_email
    });

    let (status2, _body2) = send_request(&app, "POST", "/users", Some(payload2)).await?;
    assert_eq!(status2, StatusCode::CONFLICT);

    Ok(())
}

#[tokio::test]
async fn test_tier2_get_user_non_existent_uuid_returns_404_not_found(
) -> Result<(), Box<dyn std::error::Error>> {
    let app = setup_test_app().await?;
    let non_existent_id = Uuid::new_v4().to_string();
    let uri = format!("/users/{}", non_existent_id);

    let (status, _body) = send_request(&app, "GET", &uri, None).await?;
    assert_eq!(status, StatusCode::NOT_FOUND);

    Ok(())
}

#[tokio::test]
async fn test_tier2_get_user_invalid_uuid_format_returns_400_or_404(
) -> Result<(), Box<dyn std::error::Error>> {
    let app = setup_test_app().await?;
    let invalid_id_uri = "/users/invalid-uuid-format-string";

    let (status, _body) = send_request(&app, "GET", invalid_id_uri, None).await?;
    assert!(
        status == StatusCode::BAD_REQUEST || status == StatusCode::NOT_FOUND,
        "Expected status 400 or 404, got {}",
        status
    );

    Ok(())
}

#[tokio::test]
async fn test_tier2_create_user_malformed_json_returns_400_bad_request(
) -> Result<(), Box<dyn std::error::Error>> {
    let app = setup_test_app().await?;
    let malformed_raw_json = r#"{"name": "Malformed User", "email": "#;

    let (status, _body) = send_raw_request(
        &app,
        "POST",
        "/users",
        malformed_raw_json,
        Some("application/json"),
    )
    .await?;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    Ok(())
}

#[tokio::test]
async fn test_tier2_create_user_whitespace_name_returns_400_bad_request(
) -> Result<(), Box<dyn std::error::Error>> {
    let app = setup_test_app().await?;
    let payload = json!({
        "name": "   ",
        "email": "whitespace@example.com"
    });

    let (status, _body) = send_request(&app, "POST", "/users", Some(payload)).await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    Ok(())
}

// ============================================================================
// TIER 3: PAIRWISE COMBINATION & STATE LIFECYCLE TESTS
// ============================================================================

#[tokio::test]
async fn test_tier3_pairwise_user_lifecycle_state_consistency(
) -> Result<(), Box<dyn std::error::Error>> {
    let app = setup_test_app().await?;

    // Step 1: Create User
    let unique_email = format!("carol.pairwise.{}@example.com", Uuid::new_v4());
    let create_payload = json!({
        "name": "Carol Pairwise",
        "email": unique_email.clone()
    });
    let (create_status, create_body) =
        send_request(&app, "POST", "/users", Some(create_payload)).await?;
    assert_eq!(create_status, StatusCode::CREATED);

    let user_id = create_body["id"].as_str().ok_or("Missing ID in response")?;

    // Step 2: Fetch by ID
    let get_uri = format!("/users/{}", user_id);
    let (get_status, get_body) = send_request(&app, "GET", &get_uri, None).await?;
    assert_eq!(get_status, StatusCode::OK);
    assert_eq!(get_body["id"], user_id);
    assert_eq!(get_body["email"], unique_email);

    // Step 3: List all users to establish baseline count
    let (list_status1, list_body1) = send_request(&app, "GET", "/users", None).await?;
    assert_eq!(list_status1, StatusCode::OK);
    let count_before = list_body1
        .as_array()
        .ok_or("Expected array for user list")?
        .iter()
        .filter(|u| u["email"] == unique_email)
        .count();

    // Step 4: Attempt duplicate user creation (should fail with 409)
    let dup_payload = json!({
        "name": "Carol Clone",
        "email": unique_email.clone()
    });
    let (dup_status, _dup_body) = send_request(&app, "POST", "/users", Some(dup_payload)).await?;
    assert_eq!(dup_status, StatusCode::CONFLICT);

    // Step 5: Verify user list count and state remain unchanged
    let (list_status2, list_body2) = send_request(&app, "GET", "/users", None).await?;
    assert_eq!(list_status2, StatusCode::OK);
    let count_after = list_body2
        .as_array()
        .ok_or("Expected array for user list")?
        .iter()
        .filter(|u| u["email"] == unique_email)
        .count();

    assert_eq!(
        count_before, count_after,
        "User list count for email changed after failed duplicate creation"
    );

    Ok(())
}

// ============================================================================
// TIER 4: REAL-WORLD WORKLOAD & DATA INTEGRITY TESTS
// ============================================================================

#[tokio::test]
async fn test_tier4_multi_user_sequential_creation_and_batch_listing(
) -> Result<(), Box<dyn std::error::Error>> {
    let app = setup_test_app().await?;

    let run_id = Uuid::new_v4();
    let email1 = format!("user1.tier4.{}@example.com", run_id);
    let email2 = format!("user2.tier4.{}@example.com", run_id);
    let email3 = format!("user3.tier4.{}@example.com", run_id);
    let users_to_create = vec![
        ("User One", email1),
        ("User Two", email2),
        ("User Three", email3),
    ];

    for (name, email) in &users_to_create {
        let payload = json!({ "name": name, "email": email });
        let (status, _body) = send_request(&app, "POST", "/users", Some(payload)).await?;
        assert_eq!(status, StatusCode::CREATED);
    }

    let (list_status, list_body) = send_request(&app, "GET", "/users", None).await?;
    assert_eq!(list_status, StatusCode::OK);

    let list_arr = list_body
        .as_array()
        .ok_or("Expected array response for /users")?;

    for (name, email) in &users_to_create {
        let found = list_arr
            .iter()
            .any(|u| u["email"] == email.as_str() && u["name"] == *name);
        assert!(
            found,
            "User {} with email {} not found in user list",
            name, email
        );
    }

    Ok(())
}

#[tokio::test]
async fn test_tier4_timestamp_data_integrity_and_rfc3339_validation(
) -> Result<(), Box<dyn std::error::Error>> {
    let app = setup_test_app().await?;

    let unique_email = format!("dave.tier4.{}@example.com", Uuid::new_v4());
    let payload = json!({
        "name": "Dave Timestamp",
        "email": unique_email
    });

    let (status, body) = send_request(&app, "POST", "/users", Some(payload)).await?;
    assert_eq!(status, StatusCode::CREATED);

    let created_at_str = body["created_at"]
        .as_str()
        .ok_or("Missing created_at in response")?;
    let updated_at_str = body["updated_at"]
        .as_str()
        .ok_or("Missing updated_at in response")?;

    let created_at = DateTime::parse_from_rfc3339(created_at_str)?;
    let updated_at = DateTime::parse_from_rfc3339(updated_at_str)?;

    assert!(
        created_at <= updated_at,
        "created_at ({}) should be <= updated_at ({})",
        created_at_str,
        updated_at_str
    );

    Ok(())
}
