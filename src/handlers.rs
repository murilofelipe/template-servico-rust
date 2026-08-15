use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde_json::json;
use uuid::Uuid;

use crate::{
    models::{CreateUserPayload, User},
    state::AppState,
};

#[utoipa::path(
    get,
    path = "/health",
    responses(
        (status = 200, description = "Service is healthy and DB is up"),
        (status = 503, description = "Service or DB is down")
    )
)]
pub async fn health_check(State(state): State<AppState>) -> impl IntoResponse {
    let db_ok = sqlx::query("SELECT 1").execute(&state.pool).await.is_ok();

    if db_ok {
        (StatusCode::OK, Json(json!({ "status": "ok", "db": "up" })))
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({ "status": "error", "db": "down" })),
        )
    }
}

#[utoipa::path(
    post,
    path = "/users",
    request_body = CreateUserPayload,
    responses(
        (status = 201, description = "User created successfully", body = User),
        (status = 400, description = "Invalid payload"),
        (status = 409, description = "Conflict, email already exists"),
        (status = 500, description = "Internal Server Error")
    )
)]
pub async fn create_user(
    State(state): State<AppState>,
    Json(payload): Json<CreateUserPayload>,
) -> impl IntoResponse {
    if payload.name.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Name cannot be empty" })),
        );
    }
    if !payload.email.contains('@') {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Invalid email" })),
        );
    }

    let user_id = Uuid::new_v4();

    let result = sqlx::query_as::<_, User>(
        r#"
        INSERT INTO users (id, name, email)
        VALUES ($1, $2, $3)
        RETURNING id, name, email, created_at, updated_at
        "#,
    )
    .bind(user_id)
    .bind(payload.name)
    .bind(payload.email)
    .fetch_one(&state.pool)
    .await;

    match result {
        Ok(user) => (StatusCode::CREATED, Json(json!(user))),
        Err(e) => {
            let error_msg = e.to_string();
            let status =
                if error_msg.contains("unique constraint") || error_msg.contains("duplicate key") {
                    StatusCode::CONFLICT
                } else {
                    StatusCode::INTERNAL_SERVER_ERROR
                };
            (status, Json(json!({ "error": error_msg })))
        }
    }
}

#[utoipa::path(
    get,
    path = "/users",
    responses(
        (status = 200, description = "List of all users", body = [User]),
        (status = 500, description = "Internal Server Error")
    )
)]
pub async fn list_users(State(state): State<AppState>) -> impl IntoResponse {
    let result = sqlx::query_as::<_, User>(
        r#"
        SELECT id, name, email, created_at, updated_at FROM users
        "#,
    )
    .fetch_all(&state.pool)
    .await;

    match result {
        Ok(users) => (StatusCode::OK, Json(json!(users))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        ),
    }
}

#[utoipa::path(
    get,
    path = "/users/{id}",
    params(
        ("id" = Uuid, Path, description = "User ID")
    ),
    responses(
        (status = 200, description = "User found", body = User),
        (status = 404, description = "User not found"),
        (status = 500, description = "Internal Server Error")
    )
)]
pub async fn get_user(State(state): State<AppState>, Path(id): Path<Uuid>) -> impl IntoResponse {
    let result = sqlx::query_as::<_, User>(
        r#"
        SELECT id, name, email, created_at, updated_at FROM users
        WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await;

    match result {
        Ok(Some(user)) => (StatusCode::OK, Json(json!(user))),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "User not found" })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        ),
    }
}
