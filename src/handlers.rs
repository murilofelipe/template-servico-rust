use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde_json::json;
use uuid::Uuid;

use crate::{
    error::{AppError, AppResult, ProblemDetails},
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
        (status = 400, description = "Invalid payload", body = ProblemDetails),
        (status = 409, description = "Conflict, email already exists", body = ProblemDetails),
        (status = 500, description = "Internal Server Error", body = ProblemDetails)
    )
)]
pub async fn create_user(
    State(state): State<AppState>,
    Json(payload): Json<CreateUserPayload>,
) -> AppResult<(StatusCode, Json<User>)> {
    let trimmed_name = payload.name.trim();
    if trimmed_name.is_empty() {
        return Err(AppError::BadRequest("Name cannot be empty".to_string()));
    }
    let trimmed_email = payload.email.trim();
    if !trimmed_email.contains('@') {
        return Err(AppError::BadRequest("Invalid email".to_string()));
    }

    let user_id = Uuid::new_v4();

    let user = sqlx::query_as::<_, User>(
        r#"
        INSERT INTO users (id, name, email)
        VALUES ($1, $2, $3)
        RETURNING id, name, email, created_at, updated_at
        "#,
    )
    .bind(user_id)
    .bind(trimmed_name)
    .bind(trimmed_email)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        if let sqlx::Error::Database(ref db_err) = e {
            if db_err.is_unique_violation()
                || db_err.code().as_deref() == Some("23505")
                || db_err.message().to_lowercase().contains("unique")
                || db_err.message().to_lowercase().contains("duplicate")
            {
                return AppError::Conflict("A user with this email already exists".to_string());
            }
        }
        let err_str = e.to_string().to_lowercase();
        if err_str.contains("unique constraint") || err_str.contains("duplicate key") {
            return AppError::Conflict("A user with this email already exists".to_string());
        }
        AppError::Database(e)
    })?;

    Ok((StatusCode::CREATED, Json(user)))
}

#[utoipa::path(
    get,
    path = "/users",
    responses(
        (status = 200, description = "List of all users", body = [User]),
        (status = 500, description = "Internal Server Error", body = ProblemDetails)
    )
)]
pub async fn list_users(State(state): State<AppState>) -> AppResult<Json<Vec<User>>> {
    let users = sqlx::query_as::<_, User>(
        r#"
        SELECT id, name, email, created_at, updated_at FROM users
        "#,
    )
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(users))
}

#[utoipa::path(
    get,
    path = "/users/{id}",
    params(
        ("id" = Uuid, Path, description = "User ID")
    ),
    responses(
        (status = 200, description = "User found", body = User),
        (status = 404, description = "User not found", body = ProblemDetails),
        (status = 500, description = "Internal Server Error", body = ProblemDetails)
    )
)]
pub async fn get_user(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<User>> {
    let user = sqlx::query_as::<_, User>(
        r#"
        SELECT id, name, email, created_at, updated_at FROM users
        WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

    Ok(Json(user))
}

/// Fallback handler for unmatched routes, returning RFC 7807 404 Not Found problem details.
pub async fn fallback_not_found(uri: axum::http::Uri) -> impl IntoResponse {
    AppError::NotFound(format!("Route '{}' not found", uri.path()))
}
