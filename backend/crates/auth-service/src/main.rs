mod db;
mod jwt;
mod requests;
use std::env::var;

use axum::{
    Json, Router,
    extract::{FromRef, Query, State},
    http::{HeaderMap, Response, StatusCode, header},
    response::IntoResponse,
    routing::{get, post},
};
use chrono::{Duration, Utc};
use dotenvy::dotenv;
use serde::Deserialize;
use serde_json::json;
use shared::auth::{Auth, JwtSecret};
use shared::jwt::{Claims, decode_jwt_with_grace, encode_jwt};
use shared::respond;
use sqlx::PgPool;
use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;
use uuid::Uuid;

use crate::db::{get_user, get_usernames, migrate};
use crate::{
    db::{create_user, find_user_by_email},
    jwt::{hash_password, verify_password},
    requests::{LoginRequest, RegisterRequest},
};

/// How long after `exp` a token is still accepted by `/refresh` (§2.3).
const REFRESH_GRACE_SECONDS: i64 = 3600;

#[derive(Clone)]
struct AppState {
    pool: PgPool,
    jwt_secret: String,
}

impl FromRef<AppState> for JwtSecret {
    fn from_ref(state: &AppState) -> Self {
        JwtSecret(state.jwt_secret.clone())
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    dotenv().ok();
    let jwt_secret = var("JWT_SECRET").expect("JWT Secret not set in environment");
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(5)
        .connect(&var("DATABASE_URL").unwrap())
        .await
        .expect("Database connection failed");
    migrate(&pool).await.expect("Migration failed");
    let state = AppState { pool, jwt_secret };
    //change permissive to something reasonable and configurable before production deployment.
    let cors = CorsLayer::permissive();

    let app = Router::new()
        .route("/health", get(health))
        .route("/register", post(register_handler))
        .route("/login", post(login_handler))
        .route("/refresh", post(refresh_handler))
        .route("/me", get(me_handler))
        .route("/users/usernames", get(usernames_handler))
        .layer(cors)
        .with_state(state);

    // listen globally on port 3000
    let listener = TcpListener::bind("0.0.0.0:3000")
        .await
        .expect("Address must be free and valid");
    tracing::info!("auth-service listening on 0.0.0.0:3000");
    axum::serve(listener, app)
        .await
        .expect("Error serving application");
}

async fn register_handler(
    State(state): State<AppState>,
    Json(request): Json<RegisterRequest>,
) -> Response<String> {
    let hash = hash_password(&request.password).unwrap();
    match create_user(&state.pool, &request.email, &hash, &request.username).await {
        Ok(user_id) => {
            let user = match get_user(&state.pool, &user_id).await {
                Ok(user) => user,
                Err(sqlx::Error::RowNotFound) => {
                    return respond::error(StatusCode::UNAUTHORIZED, "invalid credentials");
                }
                Err(e) => return respond::error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
            };
            match jwt::get_jwt(user, state.jwt_secret) {
                Ok(token) => respond::created(json!({ "token": token })),
                Err(e) => respond::error(StatusCode::UNAUTHORIZED, &e),
            }
        }
        Err(sqlx::Error::Database(e)) if e.is_unique_violation() => {
            respond::error(StatusCode::CONFLICT, "User with this email already exists")
        }
        Err(e) => respond::error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

async fn login_handler(
    State(state): State<AppState>,
    Json(request): Json<LoginRequest>,
) -> Response<String> {
    let user = match find_user_by_email(&state.pool, &request.email).await {
        Ok(user) => user,
        Err(sqlx::Error::RowNotFound) => {
            return respond::error(StatusCode::UNAUTHORIZED, "invalid credentials");
        }
        Err(e) => return respond::error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    };

    match verify_password(&request.password, &user.password_hash) {
        true => match jwt::get_jwt(user, state.jwt_secret) {
            Ok(token) => respond::ok(json!({ "token": token })),
            Err(e) => respond::error(StatusCode::UNAUTHORIZED, &e),
        },
        false => respond::error(StatusCode::UNAUTHORIZED, "invalid credentials"),
    }
}

/// Stateless refresh (§2.3): the presented token's signature must be valid and
/// its `exp` at most [`REFRESH_GRACE_SECONDS`] in the past; a re-signed token
/// with the same claims and a fresh expiry is returned.
async fn refresh_handler(State(state): State<AppState>, headers: HeaderMap) -> Response<String> {
    let token = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "));

    let Some(token) = token else {
        return respond::error(StatusCode::UNAUTHORIZED, "No token provided");
    };

    match decode_jwt_with_grace(token, &state.jwt_secret, REFRESH_GRACE_SECONDS) {
        Ok(claims) => {
            let refreshed = Claims {
                exp: (Utc::now() + Duration::minutes(jwt::TOKEN_TTL_MINUTES)).timestamp(),
                ..claims
            };
            match encode_jwt(&refreshed, &state.jwt_secret) {
                Ok(token) => respond::ok(json!({ "token": token })),
                Err(e) => respond::error(StatusCode::INTERNAL_SERVER_ERROR, &e),
            }
        }
        Err(e) => respond::error(StatusCode::UNAUTHORIZED, &e),
    }
}

async fn me_handler(Auth(claims): Auth) -> Response<String> {
    respond::ok(json!(claims))
}

#[derive(Debug, Deserialize)]
struct UsernamesQuery {
    /// Comma-separated list of user UUIDs.
    ids: String,
}

/// Resolves a batch of user ids to their public usernames. Any caller with a
/// valid token may use it (e.g. scoreboard-service labelling leaderboards);
/// only the non-sensitive `username` is returned, never the email.
async fn usernames_handler(
    Auth(_claims): Auth,
    State(state): State<AppState>,
    Query(query): Query<UsernamesQuery>,
) -> Response<String> {
    let ids: Vec<Uuid> = query
        .ids
        .split(',')
        .filter(|s| !s.is_empty())
        .filter_map(|s| Uuid::parse_str(s.trim()).ok())
        .collect();

    if ids.is_empty() {
        return respond::ok(json!([]));
    }

    match get_usernames(&state.pool, &ids).await {
        Ok(rows) => {
            let data: Vec<_> = rows
                .into_iter()
                .map(|(id, username)| json!({ "id": id.to_string(), "username": username }))
                .collect();
            respond::ok(json!(data))
        }
        Err(e) => respond::error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

async fn health() -> impl IntoResponse {
    Json(json!({ "status": "healthy" }))
}
