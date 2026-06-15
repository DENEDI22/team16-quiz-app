mod db;
mod models;
mod stats;

use std::collections::HashMap;
use std::env::var;

use axum::{
    Json, Router,
    extract::{FromRef, Query, State},
    http::{HeaderMap, Response, StatusCode, header},
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
use tower_http::cors::CorsLayer;
use uuid::Uuid;

use crate::db::{
    get_account_stats, get_answer_history, get_category_leaderboard, get_duel_leaderboard,
    get_highscores, get_question_stats, get_singleplayer_leaderboard, get_user_duels,
    insert_answer, insert_duel_result, insert_singleplayer_result, migrate,
};
use crate::models::{
    CreateAnswerRequest, CreateDuelResultRequest, CreateSinglePlayerResultRequest,
};

/// Minimum answers in a category required to appear on its leaderboard, so a
/// lone 1/1 answer can't top the accuracy ranking.
const MIN_CATEGORY_ANSWERS: i64 = 10;

#[derive(Clone)]
struct AppState {
    pool: PgPool,
    jwt_secret: String,
    http_client: reqwest::Client,
    auth_service_url: String,
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

#[derive(Debug, Deserialize)]
struct CategoryLeaderboardQuery {
    category: String,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    dotenv().ok();
    let jwt_secret = var("JWT_SECRET").expect("JWT Secret not set in environment");
    let auth_service_url =
        var("AUTH_SERVICE_URL").expect("AUTH_SERVICE_URL not set in environment");
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(5)
        .connect(&var("DATABASE_URL").unwrap())
        .await
        .expect("Database connection failed");
    migrate(&pool).await.expect("Migration failed");
    let state = AppState {
        pool,
        jwt_secret,
        http_client: reqwest::Client::new(),
        auth_service_url,
    };

    let app = Router::new()
        .route("/health", get(health))
        .route("/post-answer", post(post_answer))
        .route("/singleplayer-result", post(post_singleplayer_result))
        .route("/duel-results", post(post_duel_results))
        .route("/user-duels", get(user_duels))
        .route("/question-stats", get(question_stats))
        .route("/highscores", get(highscores))
        .route("/answer-history", get(answer_history))
        .route("/account-stats", get(account_stats))
        .route("/leaderboard/duels", get(leaderboard_duels))
        .route("/leaderboard/singleplayer", get(leaderboard_singleplayer))
        .route("/leaderboard/category", get(leaderboard_category))
        .layer(CorsLayer::permissive())
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

async fn highscores(State(state): State<AppState>) -> Response<String> {
    match get_highscores(&state.pool).await {
        Ok(scores) => respond::ok(json!(scores)),
        Err(e) => server_error(&e.to_string()),
    }
}

async fn answer_history(Auth(claims): Auth, State(state): State<AppState>) -> Response<String> {
    match get_answer_history(&state.pool, claims.id).await {
        Ok(history) => respond::ok(json!(history)),
        Err(e) => server_error(&e.to_string()),
    }
}

/// Records a finished singleplayer game's aggregate score for the authenticated
/// user (the user id comes from the token, not the body).
async fn post_singleplayer_result(
    Auth(claims): Auth,
    State(state): State<AppState>,
    Json(request): Json<CreateSinglePlayerResultRequest>,
) -> Response<String> {
    match insert_singleplayer_result(&state.pool, claims.id, &request).await {
        Ok(id) => respond::created(json!({ "singlePlayerResultId": id.to_string() })),
        Err(e) => server_error(&e.to_string()),
    }
}

/// Combined personal stats for the account overview (Req 1). Opponent
/// usernames are resolved via auth-service using the caller's own token.
async fn account_stats(
    Auth(claims): Auth,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Response<String> {
    let mut stats = match get_account_stats(&state.pool, claims.id).await {
        Ok(stats) => stats,
        Err(e) => return server_error(&e.to_string()),
    };

    let ids: Vec<Uuid> = stats.last_duels.iter().map(|d| d.opponent_id).collect();
    let names = resolve_usernames(&state, &headers, &ids).await;
    for duel in &mut stats.last_duels {
        if let Some(name) = names.get(&duel.opponent_id) {
            duel.opponent_username = name.clone();
        }
    }

    respond::ok(json!(stats))
}

async fn leaderboard_duels(
    Auth(_claims): Auth,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Response<String> {
    let mut entries = match get_duel_leaderboard(&state.pool).await {
        Ok(entries) => entries,
        Err(e) => return server_error(&e.to_string()),
    };
    let ids: Vec<Uuid> = entries.iter().map(|e| e.user_id).collect();
    let names = resolve_usernames(&state, &headers, &ids).await;
    for entry in &mut entries {
        if let Some(name) = names.get(&entry.user_id) {
            entry.username = name.clone();
        }
    }
    respond::ok(json!(entries))
}

async fn leaderboard_singleplayer(
    Auth(_claims): Auth,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Response<String> {
    let mut boards = match get_singleplayer_leaderboard(&state.pool).await {
        Ok(boards) => boards,
        Err(e) => return server_error(&e.to_string()),
    };
    let ids: Vec<Uuid> = boards
        .iter()
        .flat_map(|b| b.entries.iter().map(|e| e.user_id))
        .collect();
    let names = resolve_usernames(&state, &headers, &ids).await;
    for board in &mut boards {
        for entry in &mut board.entries {
            if let Some(name) = names.get(&entry.user_id) {
                entry.username = name.clone();
            }
        }
    }
    respond::ok(json!(boards))
}

async fn leaderboard_category(
    Auth(_claims): Auth,
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<CategoryLeaderboardQuery>,
) -> Response<String> {
    let mut entries =
        match get_category_leaderboard(&state.pool, &query.category, MIN_CATEGORY_ANSWERS).await {
            Ok(entries) => entries,
            Err(e) => return server_error(&e.to_string()),
        };
    let ids: Vec<Uuid> = entries.iter().map(|e| e.user_id).collect();
    let names = resolve_usernames(&state, &headers, &ids).await;
    for entry in &mut entries {
        if let Some(name) = names.get(&entry.user_id) {
            entry.username = name.clone();
        }
    }
    respond::ok(json!(entries))
}

/// Resolves user ids to usernames via auth-service, forwarding the caller's
/// bearer token (token pass-through, docs/api-contracts.md §2.1). Best-effort:
/// on any failure the returned map is simply incomplete, and entries keep their
/// empty username rather than failing the whole request.
async fn resolve_usernames(
    state: &AppState,
    headers: &HeaderMap,
    ids: &[Uuid],
) -> HashMap<Uuid, String> {
    let mut map = HashMap::new();
    if ids.is_empty() {
        return map;
    }
    let Some(auth_header) = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
    else {
        return map;
    };

    let ids_param = ids
        .iter()
        .map(|id| id.to_string())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
        .join(",");

    let result = state
        .http_client
        .get(format!("{}/users/usernames", state.auth_service_url))
        .query(&[("ids", ids_param)])
        .header(header::AUTHORIZATION, auth_header)
        .send()
        .await;

    let resp = match result {
        Ok(resp) if resp.status().is_success() => resp,
        Ok(resp) => {
            tracing::warn!("auth-service username lookup returned {}", resp.status());
            return map;
        }
        Err(e) => {
            tracing::warn!("auth-service username lookup failed: {e}");
            return map;
        }
    };

    let body: serde_json::Value = match resp.json().await {
        Ok(body) => body,
        Err(e) => {
            tracing::warn!("failed to parse auth-service response: {e}");
            return map;
        }
    };

    if let Some(items) = body.get("data").and_then(|d| d.as_array()) {
        for item in items {
            if let (Some(id), Some(username)) = (
                item.get("id").and_then(|v| v.as_str()),
                item.get("username").and_then(|v| v.as_str()),
            ) && let Ok(uuid) = Uuid::parse_str(id)
            {
                map.insert(uuid, username.to_string());
            }
        }
    }

    map
}

async fn health() -> impl IntoResponse {
    Json(json!({ "status": "healthy" }))
}

fn server_error(msg: &str) -> Response<String> {
    tracing::error!("internal error: {}", msg);
    respond::error(StatusCode::INTERNAL_SERVER_ERROR, msg)
}
