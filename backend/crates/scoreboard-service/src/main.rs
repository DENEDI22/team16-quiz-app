mod db;
mod models;
mod stats;

use std::env::var;

use axum::{
    Json, Router,
    extract::{FromRef, Query, State},
    http::{Response, StatusCode},
    response::IntoResponse,
    routing::{get, post},
};
use dotenvy::dotenv;
use serde::Deserialize;
use serde_json::json;
use shared::auth::{Auth, JwtSecret};
use shared::respond;
use sqlx::PgPool;
use tokio::{self, net::TcpListener};
use uuid::Uuid;

use crate::db::{get_question_stats, get_user_duels, insert_answer, insert_duel_result, migrate};
use crate::models::{CreateAnswerRequest, CreateDuelResultRequest};

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
        Ok(id) => respond::created(json!({ "answerRecordId": id.to_string() })),
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
        Ok(id) => respond::created(json!({ "duelId": id.to_string() })),
        Err(e) => server_error(&e.to_string()),
    }
}

async fn user_duels(
    Auth(_claims): Auth,
    State(state): State<AppState>,
    Query(query): Query<UserDuelsQuery>,
) -> Response<String> {
    match get_user_duels(&state.pool, query.user_id).await {
        Ok(duels) => respond::ok(json!(duels)),
        Err(e) => server_error(&e.to_string()),
    }
}

async fn question_stats(
    Auth(_claims): Auth,
    State(state): State<AppState>,
    Query(query): Query<QuestionStatsQuery>,
) -> Response<String> {
    match get_question_stats(&state.pool, query.question_id).await {
        Ok(Some(stats)) => respond::ok(json!(stats)),
        Ok(None) => respond::error(
            StatusCode::NOT_FOUND,
            "No answers recorded for this question",
        ),
        Err(e) => server_error(&e.to_string()),
    }
}

async fn health() -> impl IntoResponse {
    Json(json!({ "status": "healthy" }))
}

fn server_error(msg: &str) -> Response<String> {
    tracing::error!("internal error: {}", msg);
    respond::error(StatusCode::INTERNAL_SERVER_ERROR, msg)
}
