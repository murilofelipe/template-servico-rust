use std::sync::Arc;

use axum::{
    body::Body,
    extract::State,
    http::{header, Request},
    middleware::Next,
    response::{IntoResponse, Response},
};
use jsonwebtoken::{decode_header, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::{error::ProblemDetails, state::AppState};

/// Claims extraídas de um token JWT validado.
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    /// Subject — identificador do usuário/entidade.
    pub sub: Option<String>,
    /// Expiration time (Unix timestamp).
    pub exp: u64,
    /// Issuer — emissor do token.
    pub iss: Option<String>,
}

/// Cache em memória para o JWKS (JSON Web Key Set) obtido de um endpoint remoto.
///
/// O JSON bruto é armazenado e re-parseado a cada validação para evitar problemas
/// de clone com tipos do `jsonwebtoken`. A busca ocorre de forma lazy na primeira
/// requisição.
#[derive(Debug, Clone)]
pub struct JwksCache {
    url: String,
    cached_json: Arc<RwLock<Option<String>>>,
}

impl JwksCache {
    /// Cria um novo `JwksCache` apontando para a URL do endpoint JWKS.
    #[must_use]
    pub fn new(url: String) -> Self {
        Self {
            url,
            cached_json: Arc::new(RwLock::new(None)),
        }
    }

    /// Retorna o JSON do JWKS em cache, buscando na URL se ainda não foi carregado.
    async fn get_jwks_json(&self) -> Result<String, String> {
        {
            let cache = self.cached_json.read().await;
            if let Some(json) = cache.as_ref() {
                return Ok(json.clone());
            }
        }

        tracing::debug!(url = %self.url, "Fetching JWKS from remote endpoint");
        let json = reqwest::get(&self.url)
            .await
            .map_err(|e| format!("Failed to fetch JWKS from '{}': {e}", self.url))?
            .text()
            .await
            .map_err(|e| format!("Failed to read JWKS response body: {e}"))?;

        let mut cache = self.cached_json.write().await;
        *cache = Some(json.clone());
        Ok(json)
    }

    /// Retorna a `DecodingKey` correspondente ao `kid` informado (ou a primeira chave do set).
    pub async fn get_decoding_key(&self, kid: Option<&str>) -> Result<DecodingKey, String> {
        let json = self.get_jwks_json().await?;
        let jwks: jsonwebtoken::jwk::JwkSet =
            serde_json::from_str(&json).map_err(|e| format!("Failed to parse JWKS JSON: {e}"))?;

        let jwk = match kid {
            Some(k) => jwks
                .find(k)
                .ok_or_else(|| format!("No JWK found for kid='{k}' in JWKS"))?,
            None => jwks
                .keys
                .first()
                .ok_or_else(|| "JWKS set is empty".to_string())?,
        };

        DecodingKey::from_jwk(jwk)
            .map_err(|e| format!("Failed to build decoding key from JWK: {e}"))
    }
}

/// Middleware Axum para autenticação JWT via JWKS.
///
/// Comportamento:
/// - Se `jwks_cache` não estiver configurado no `AppState`, o middleware é transparente
///   (todas as requisições passam sem validação).
/// - Se configurado, extrai o `Authorization: Bearer <token>` e valida o JWT contra
///   as chaves públicas JWKS, retornando `401 Unauthorized` (RFC 7807) em caso de falha.
pub async fn jwt_auth_middleware(
    State(state): State<AppState>,
    request: Request<Body>,
    next: Next,
) -> Response {
    // Se não há JWKS configurado, passa transparentemente
    let Some(jwks_cache) = state.jwks_cache.as_ref() else {
        return next.run(request).await;
    };

    // Extrai o Bearer token do header Authorization
    let token = match extract_bearer_token(&request) {
        Some(t) => t.to_owned(),
        None => {
            tracing::warn!("Request missing or malformed Authorization header");
            return ProblemDetails::unauthorized(
                "Missing or malformed Authorization header. Expected: 'Bearer <token>'.",
            )
            .into_response();
        }
    };

    // Decodifica o header JWT para obter o kid e o algoritmo
    let jwt_header = match decode_header(&token) {
        Ok(h) => h,
        Err(e) => {
            tracing::warn!(error = %e, "Failed to decode JWT header");
            return ProblemDetails::unauthorized("Invalid JWT token format.").into_response();
        }
    };

    // Busca a chave pública correspondente no JWKS
    let decoding_key = match jwks_cache.get_decoding_key(jwt_header.kid.as_deref()).await {
        Ok(k) => k,
        Err(e) => {
            tracing::warn!(error = %e, "Failed to retrieve JWKS decoding key");
            return ProblemDetails::unauthorized(
                "Unable to retrieve token validation key. Please try again.",
            )
            .into_response();
        }
    };

    // Configura a validação (issuer e audience opcionais via config)
    let mut validation = Validation::new(jwt_header.alg);
    if let Some(ref iss) = state.config.jwt_issuer {
        validation.set_issuer(&[iss.as_str()]);
    }
    if let Some(ref aud) = state.config.jwt_audience {
        validation.set_audience(&[aud.as_str()]);
    }

    // Valida o token
    match jsonwebtoken::decode::<Claims>(&token, &decoding_key, &validation) {
        Ok(token_data) => {
            tracing::debug!(sub = ?token_data.claims.sub, "JWT validated successfully");
            next.run(request).await
        }
        Err(e) => {
            tracing::warn!(error = %e, "JWT validation failed");
            ProblemDetails::unauthorized("Invalid or expired token.").into_response()
        }
    }
}

/// Extrai o token Bearer bruto do header `Authorization`.
fn extract_bearer_token(request: &Request<Body>) -> Option<&str> {
    let auth_header = request.headers().get(header::AUTHORIZATION)?;
    let auth_str = auth_header.to_str().ok()?;
    auth_str.strip_prefix("Bearer ")
}

#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
        middleware,
        routing::get,
        Router,
    };
    use tower::ServiceExt;

    use crate::{
        config::{AppConfig, AppEnvironment},
        state::AppState,
    };

    fn make_test_state(jwks_url: Option<String>) -> AppState {
        use sqlx::postgres::PgPoolOptions;
        // Cria um pool inerte (não conectado) apenas para testes de middleware
        let pool = PgPoolOptions::new()
            .max_connections(1)
            .connect_lazy("postgres://postgres:postgres@localhost:5432/test")
            .unwrap_or_else(|_| {
                // Fallback: pool mínimo não conectado
                PgPoolOptions::new().connect_lazy_with(sqlx::postgres::PgConnectOptions::new())
            });

        let jwks_cache = jwks_url.map(|url| std::sync::Arc::new(super::JwksCache::new(url)));

        AppState {
            pool,
            config: AppConfig {
                database_url: "postgres://localhost/test".to_string(),
                port: 3000,
                host: "0.0.0.0".to_string(),
                environment: AppEnvironment::Test,
                allowed_origins: vec![],
                otel_service_name: "test".to_string(),
                otlp_endpoint: None,
                jwks_url: None,
                jwt_audience: None,
                jwt_issuer: None,
                redis_url: None,
            },
            jwks_cache,
            redis_cache: None,
        }
    }

    fn test_app(state: AppState) -> Router {
        Router::new()
            .route("/protected", get(|| async { "ok" }))
            .route_layer(middleware::from_fn_with_state(
                state.clone(),
                super::jwt_auth_middleware,
            ))
            .with_state(state)
    }

    #[tokio::test]
    async fn test_jwt_no_jwks_url_passes_through() {
        // Sem JWKS configurado, middleware é transparente
        let state = make_test_state(None);
        let app = test_app(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/protected")
                    .body(Body::empty())
                    .unwrap_or_else(|_| Request::new(Body::empty())),
            )
            .await
            .unwrap_or_else(|_| {
                axum::http::Response::builder()
                    .status(500)
                    .body(Body::empty())
                    .unwrap_or_default()
            });

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_jwt_missing_bearer_returns_401() {
        // Com JWKS configurado mas sem header Authorization, deve retornar 401
        let state = make_test_state(Some(
            "http://localhost:9999/.well-known/jwks.json".to_string(),
        ));
        let app = test_app(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/protected")
                    .body(Body::empty())
                    .unwrap_or_else(|_| Request::new(Body::empty())),
            )
            .await
            .unwrap_or_else(|_| {
                axum::http::Response::builder()
                    .status(500)
                    .body(Body::empty())
                    .unwrap_or_default()
            });

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_jwt_invalid_token_returns_401() {
        // Token malformado deve retornar 401
        let state = make_test_state(Some(
            "http://localhost:9999/.well-known/jwks.json".to_string(),
        ));
        let app = test_app(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/protected")
                    .header("Authorization", "Bearer invalid.token.here")
                    .body(Body::empty())
                    .unwrap_or_else(|_| Request::new(Body::empty())),
            )
            .await
            .unwrap_or_else(|_| {
                axum::http::Response::builder()
                    .status(500)
                    .body(Body::empty())
                    .unwrap_or_default()
            });

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
}
