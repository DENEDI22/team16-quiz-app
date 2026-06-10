mod db;
mod jwt;
mod requests;
use std::{clone, env::var};

use axum::{
    Json, Router,
    extract::{FromRequestParts, State},
    http::{Response, header},
    response::IntoResponse,
    routing::{get, post},
};
use dotenvy::dotenv;
use serde_json::json;
use shared::jwt::{Claims, decode_jwt};
use sqlx::PgPool;
use tokio::net::TcpListener;

use crate::db::{get_user, migrate};
use crate::{
    db::{create_user, find_user_by_email},
    jwt::{hash_password, verify_password},
    requests::{LoginRequest, RegisterRequest},
};

#[derive(clone::Clone)]
struct AppState {
    pool: PgPool,
    jwt_secret: String,
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

    let app = Router::new()
        .route("/health", get(health))
        .route("/register", post(register_handler))
        .route("/login", post(login_handler))
        .route("/me", get(me_handler))
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
                Err(sqlx::Error::RowNotFound) => return unauthorized("invalid credentials"),
                Err(e) => return server_error(&e.to_string()),
            };
            let token = jwt::get_jwt(user, state.jwt_secret);
            match token {
                Ok(token) => return token_response(&token.to_string()),
                Err(e) => return unauthorized(&e.to_string()),
            }
        }
        Err(sqlx::Error::Database(e)) if e.is_unique_violation() => {
            return conflict("User with this email already exists");
        }
        Err(e) => return server_error(&e.to_string()),
    }
}
async fn login_handler(
    State(state): State<AppState>,
    Json(request): Json<LoginRequest>,
) -> Response<String> {
    let user = match find_user_by_email(&state.pool, &request.email).await {
        Ok(user) => user,
        Err(sqlx::Error::RowNotFound) => return unauthorized("invalid credentials"),
        Err(e) => return server_error(&e.to_string()),
    };

    match verify_password(&request.password, &user.password_hash) {
        true => {
            let token = jwt::get_jwt(user, state.jwt_secret);

            match token {
                Ok(token) => success(&token.to_string()),

                Err(e) => unauthorized(&e.to_string()),
            }
        }
        false => unauthorized("invalid credentials"),
    }
}

fn conflict(msg: &str) -> Response<String> {
    Response::builder()
        .status(409)
        .header(header::CONTENT_TYPE, "application/json")
        .body(
            json!({
                "success": false,
                "data": {
                    "message": msg
                }
            })
            .to_string(),
        )
        .unwrap_or_default()
}

fn token_response(token: &str) -> Response<String> {
    Response::builder()
        .status(200)
        .header(header::CONTENT_TYPE, "application/json")
        .body(
            json!({
                "success": true,
                "token": token,
            })
            .to_string(),
        )
        .unwrap_or_default()
}

fn success(msg: &str) -> Response<String> {
    Response::builder()
        .status(200)
        .header(header::CONTENT_TYPE, "application/json")
        .body(
            json!({
                "success": true,
                "data": {
                    "message": msg
                }
            })
            .to_string(),
        )
        .unwrap_or_default()
}

fn unauthorized(msg: &str) -> Response<String> {
    Response::builder()
        .status(401)
        .header(header::CONTENT_TYPE, "application/json")
        .body(
            json!({
                "success": false,
                "data": {
                    "message": msg
                }
            })
            .to_string(),
        )
        .unwrap_or_default()
}

fn server_error(msg: &str) -> Response<String> {
    tracing::error!("internal error: {}", msg);
    Response::builder()
        .status(500)
        .header(header::CONTENT_TYPE, "application/json")
        .body(
            json!({
                "success": false,
                "data": {
                    "message": msg
                }
            })
            .to_string(),
        )
        .unwrap_or_default()
}

async fn me_handler(Auth(claims): Auth) -> Response<String> {
    Response::builder()
        .status(200)
        .header(header::CONTENT_TYPE, "application/json")
        .body(
            json!({
                "success": true,
                "data": claims
            })
            .to_string(),
        )
        .unwrap_or_default()
}

pub struct Auth(Claims);

#[cfg_attr(cfg, async_trait)]
impl<S> FromRequestParts<S> for Auth
where
    S: Send + Sync,
{
    type Rejection = Response<String>;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        _: &S,
    ) -> Result<Self, Self::Rejection> {
        let access_token = parts
            .headers
            .get(header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.split(" ").nth(1));

        match access_token {
            Some(token) => {
                let claims = decode_jwt(token, var("JWT_SECRET").unwrap());

                match claims {
                    Ok(claims) => Ok(Auth(claims)),

                    Err(e) => Err(unauthorized(&e.to_string())),
                }
            }

            None => Err(unauthorized("No token provided")),
        }
    }
}

async fn health() -> impl IntoResponse {
    "healthy"
}
