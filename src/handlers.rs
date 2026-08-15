use axum::{
    extract::State,
    http::StatusCode,
    Json,
};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{
    error::{AppError, AppJson, AppPath, AppResult},
    models::{CreateUserPayload, User},
    state::AppState,
};

#[utoipa::path(
    get,
    path = "/health",
    responses(
        (status = 200, description = "Service is healthy and DB is up"),
        (status = 503, description = "Service or DB is down", body = ProblemDetails)
    )
)]
pub async fn health_check(State(state): State<AppState>) -> AppResult<(StatusCode, Json<Value>)> {
    let db_ok = sqlx::query("SELECT 1").execute(&state.pool).await.is_ok();

    if db_ok {
        Ok((StatusCode::OK, Json(json!({ "status": "ok", "db": "up" }))))
    } else {
        Err(AppError::ServiceUnavailable(
            "Database connection is currently unavailable".to_string(),
        ))
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
    AppJson(payload): AppJson<CreateUserPayload>,
) -> AppResult<(StatusCode, Json<User>)> {
    let trimmed_name = payload.name.trim();
    if trimmed_name.is_empty() {
        return Err(AppError::BadRequest(
            "Field 'name' cannot be empty or contain only whitespace".to_string(),
        ));
    }
    if !payload.email.contains('@') {
        return Err(AppError::BadRequest(
            "Field 'email' must contain a valid email address".to_string(),
        ));
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
    .bind(&payload.email)
    .fetch_one(&state.pool)
    .await
    .map_err(AppError::Database)?;

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
    .await
    .map_err(AppError::Database)?;

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
    AppPath(id): AppPath<Uuid>,
) -> AppResult<Json<User>> {
    let user = sqlx::query_as::<_, User>(
        r#"
        SELECT id, name, email, created_at, updated_at FROM users
        WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await
    .map_err(AppError::Database)?
    .ok_or_else(|| AppError::NotFound(format!("User with id '{id}' was not found")))?;

    Ok(Json(user))
}

