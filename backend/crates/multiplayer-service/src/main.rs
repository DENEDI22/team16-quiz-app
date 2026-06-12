mod cache;
mod duel;
mod models;
mod ws;

use std::env::var;

use axum::{
    Json, Router,
    extract::{FromRequestParts, Path, State},
    http::{Response, request::Parts},
    response::IntoResponse,
    routing::{delete, get, post},
};
use redis::{Client, aio::ConnectionManager};
use reqwest::header;
use serde_json::{Value, json};
use shared::jwt::{Claims, decode_jwt};
use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;
use uuid::Uuid;

use crate::{
    cache::{get_lobby_by_key, get_open_lobbies},
    models::{CreateLobbyRequest, Lobby, LobbySettings, PlayerInfo},
};

#[derive(Clone)]
pub struct AppState {
    pub http_client: reqwest::Client,
    pub quiz_service_url: String,
    pub scoreboard_service_url: String,
    pub redis: ConnectionManager,
    pub jwt_secret: String,
    pub duels: duel::DuelRegistry,
}
#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    dotenvy::dotenv().ok();

    let redis_url = std::env::var("REDIS_URL").expect("REDIS_URL must be set");
    let address = std::env::var("ADDRESS").expect("ADDRESS must be set");

    let state = AppState {
        http_client: reqwest::Client::new(),
        quiz_service_url: std::env::var("QUIZ_SERVICE_URL").expect("QUIZ_SERVICE_URL must be set"),
        scoreboard_service_url: std::env::var("SCOREBOARD_SERVICE_URL")
            .expect("SCOREBOARD_SERVICE_URL must be set"),
        redis: ConnectionManager::new(Client::open(redis_url).expect("Connection to redis failed"))
            .await
            .expect("Error creating connection Manager"),
        jwt_secret: std::env::var("JWT_SECRET").expect("JWT_SECRET must be set"),
        duels: duel::DuelRegistry::default(),
    };

    let app = Router::new()
        .route("/health", get(health))
        .route("/lobbies", get(get_lobbies_handler))
        .route("/lobbies/", post(create_lobby))
        .route("/lobbies/{id}", delete(delete_lobby))
        .route("/lobbies/{id}/join", post(join_lobby))
        .route("/duels/{id}/ws", get(ws::ws_upgrade))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let listener = TcpListener::bind(&address)
        .await
        .expect("Address must be free and valid");
    tracing::info!("multiplayer-service listening on {}", address);
    axum::serve(listener, app)
        .await
        .expect("Error serving application");
}
async fn join_lobby(
    Auth(claims): Auth,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    let guest = PlayerInfo {
        id: claims.id,
        username: display_name(&claims),
    };
    let mut redis = state.redis.clone();
    match cache::join_lobby(&mut redis, id, &guest).await {
        Ok(lobby) => ok_json(json!(lobby)),
        Err(cache::JoinError::NotFound) => not_found("Lobby does not exist or has expired"),
        Err(cache::JoinError::Full) => conflict("Lobby already has a guest"),
        Err(cache::JoinError::OwnLobby) => conflict("You cannot join your own lobby"),
        Err(cache::JoinError::Internal(e)) => server_error("Error joining the lobby", &e),
    }
}

async fn delete_lobby(
    Auth(claims): Auth,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    let mut redis = state.redis.clone();
    let lobby = match get_lobby_by_key(&mut redis, id).await {
        Ok(Some(lobby)) => lobby,
        Ok(None) => return not_found("Lobby does not exist or has expired"),
        Err(e) => return server_error("Error deleting the lobby", &e.to_string()),
    };
    if lobby.host.id != claims.id {
        return forbidden("Only the host can delete the lobby");
    }
    match crate::cache::delete_lobby(&mut redis, id).await {
        Ok(()) => ok_json(json!({"status": "success"})),
        Err(e) => server_error("Error deleting the lobby", &e.to_string()),
    }
}

async fn create_lobby(
    State(state): State<AppState>,
    Auth(claims): Auth,
    Json(request): Json<CreateLobbyRequest>,
) -> impl IntoResponse {
    if !(10..=50).contains(&request.question_count) {
        return bad_request("questionCount must be between 10 and 50");
    }
    let name = request.name.trim().to_string();
    if name.is_empty() || name.chars().count() > 40 {
        return bad_request("name must be 1-40 characters");
    }
    let player = PlayerInfo {
        id: claims.id,
        username: display_name(&claims),
    };
    let settings = LobbySettings {
        difficulty: request.difficulty,
        categories: request.categories,
        question_count: request.question_count,
    };
    let lobby = Lobby {
        id: Uuid::new_v4(),
        name,
        host: player,
        guest: None,
        settings,
        status: models::LobbyStatus::Waiting,
        created_at: chrono::Utc::now(),
    };
    let mut redis = state.redis.clone();
    match cache::create_open_lobby(&mut redis, &lobby).await {
        Ok(()) => {
            return created(json!(lobby));
        }
        Err(e) => server_error("Internal error creating the lobby", &e.to_string()),
    }
}

async fn get_lobbies_handler(State(state): State<AppState>) -> impl IntoResponse {
    let mut redis = state.redis.clone();

    match get_open_lobbies(&mut redis).await {
        Ok(lobbies) => ok_json(json!(lobbies)),
        Err(e) => server_error(&e.to_string(), &e.to_string()),
    }
}

/// Display name shown to other players. Tokens minted before the username
/// claim existed decode with an empty string; never fall back to the email.
pub fn display_name(claims: &Claims) -> String {
    if claims.username.trim().is_empty() {
        "Spieler".to_string()
    } else {
        claims.username.clone()
    }
}

async fn health() -> impl IntoResponse {
    Json(json!({ "status": "healthy" }))
}

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
            Some(token) => match decode_jwt(token, &var("JWT_SECRET").unwrap()) {
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

fn forbidden(msg: &str) -> Response<String> {
    json_response(403, json!({ "status": "error", "message": msg }))
}

fn conflict(msg: &str) -> Response<String> {
    json_response(409, json!({ "status": "error", "message": msg }))
}

fn unauthorized(msg: &str) -> Response<String> {
    json_response(401, json!({ "status": "error", "message": msg }))
}

fn server_error(msg: &str, error: &str) -> Response<String> {
    tracing::error!("internal error: {}", error);
    json_response(500, json!({ "status": "error", "message": msg }))
}

fn bad_request(msg: &str) -> Response<String> {
    json_response(400, json!({"status": "error", "message": msg }))
}
