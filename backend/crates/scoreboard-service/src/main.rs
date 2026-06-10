mod db;
mod models;
mod stats;

use std::{clone, env::var};

use axum::{
    Json, Router,
    extract::{FromRequestParts, Query, State},
    http::{Response, header, request::Parts},
    response::IntoResponse,
    routing::{get, post},
};
use dotenvy::dotenv;
use serde::Deserialize;
use serde_json::{Value, json};
use shared::jwt::{Claims, decode_jwt};
use sqlx::PgPool;
use tokio::{self, net::TcpListener};
use uuid::Uuid;

use crate::db::{get_question_stats, get_user_duels, insert_answer, insert_duel_result, migrate};
use crate::models::{CreateAnswerRequest, CreateDuelResultRequest};

#[derive(clone::Clone)]
struct AppState {
    pool: PgPool,
    jwt_secret: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UserDuelsQuery {
    user_id: Uuid,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct QuestionStatsQuery {
    question_id: Uuid,
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
        .route("/post-answer", post(post_answer))
        .route("/duel-results", post(post_duel_results))
        .route("/user-duels", get(user_duels))
        .route("/question-stats", get(question_stats))
        .with_state(state);

    // listen globally on port 3000
    let listener = TcpListener::bind("0.0.0.0:3000")
        .await
        .expect("Address must be free and valid");
    tracing::info!("scoreboard-service listening on 0.0.0.0:3000");
    axum::serve(listener, app)
        .await
        .expect("Error serving application");
}

/// Records an answer for the *authenticated* user. The stored `user_id` is taken
/// from the token, never from the request body.
async fn post_answer(
    Auth(claims): Auth,
    State(state): State<AppState>,
    Json(request): Json<CreateAnswerRequest>,
) -> Response<String> {
    match insert_answer(&state.pool, claims.id, &request).await {
        Ok(id) => created(json!({
            "status": "ok",
            "answerRecordId": id.to_string(),
        })),
        Err(e) => server_error(&e.to_string()),
    }
}

/// Requires a valid token to access; both player ids come from the request body.
async fn post_duel_results(
    Auth(_claims): Auth,
    State(state): State<AppState>,
    Json(request): Json<CreateDuelResultRequest>,
) -> Response<String> {
    match insert_duel_result(&state.pool, &request).await {
        Ok(id) => created(json!({
            "status": "ok",
            "duelId": id.to_string(),
        })),
        Err(e) => server_error(&e.to_string()),
    }
}

async fn user_duels(
    Auth(_claims): Auth,
    State(state): State<AppState>,
    Query(query): Query<UserDuelsQuery>,
) -> Response<String> {
    match get_user_duels(&state.pool, query.user_id).await {
        Ok(duels) => ok_json(json!(duels)),
        Err(e) => server_error(&e.to_string()),
    }
}

async fn question_stats(
    Auth(_claims): Auth,
    State(state): State<AppState>,
    Query(query): Query<QuestionStatsQuery>,
) -> Response<String> {
    match get_question_stats(&state.pool, query.question_id).await {
        Ok(Some(stats)) => ok_json(json!(stats)),
        Ok(None) => not_found("No answers recorded for this question"),
        Err(e) => server_error(&e.to_string()),
    }
}

async fn health() -> impl IntoResponse {
    Json(json!({ "status": "healthy" }))
}

/// Extractor that validates the `Authorization: Bearer <token>` header and yields
/// the decoded JWT claims. Mirrors the extractor used in auth-service.
pub struct Auth(Claims);

impl<S> FromRequestParts<S> for Auth
where
    S: Send + Sync,
{
    type Rejection = Response<String>;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        let access_token = parts
            .headers
            .get(header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.split(' ').nth(1));

        match access_token {
            Some(token) => match decode_jwt(token, var("JWT_SECRET").unwrap()) {
                Ok(claims) => Ok(Auth(claims)),
                Err(e) => Err(unauthorized(&e)),
            },
            None => Err(unauthorized("No token provided")),
        }
    }
}

fn json_response(status: u16, body: Value) -> Response<String> {
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "application/json")
        .body(body.to_string())
        .unwrap_or_default()
}

fn ok_json(body: Value) -> Response<String> {
    json_response(200, body)
}

fn created(body: Value) -> Response<String> {
    json_response(201, body)
}

fn not_found(msg: &str) -> Response<String> {
    json_response(404, json!({ "status": "error", "message": msg }))
}

fn unauthorized(msg: &str) -> Response<String> {
    json_response(401, json!({ "status": "error", "message": msg }))
}

fn server_error(msg: &str) -> Response<String> {
    tracing::error!("internal error: {}", msg);
    json_response(500, json!({ "status": "error", "message": msg }))
}
