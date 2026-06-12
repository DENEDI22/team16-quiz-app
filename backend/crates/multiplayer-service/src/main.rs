mod cache;
mod models;

use std::env::var;

use axum::{
    Json, Router,
    body::Body,
    extract::{FromRequestParts, State},
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

use crate::models::{CreateLobbyRequest, Lobby, LobbySettings, PlayerInfo};

#[derive(Clone)]
pub struct AppState {
    pub http_client: reqwest::Client,
    pub quiz_service_url: String,
    pub scoreboard_service_url: String,
    pub redis: ConnectionManager,
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
    };

    let app = Router::new()
        .route("/health", get(health))
        .route("/lobbies", get(get_lobbies_handler))
        .route("/lobbies", post(create_lobby))
        .route("/lobbies", delete(delete_lobby))
        .route("/join", post(join_lobby))
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
async fn join_lobby(State(state): State<AppState>) {}

async fn delete_lobby(State(state): State<AppState>) {}

async fn create_lobby(
    State(state): State<AppState>,
    Auth(claims): Auth,
    Json(request): Json<CreateLobbyRequest>,
) -> impl IntoResponse {
    let player = PlayerInfo {
        id: claims.id,
        email: claims.email,
    };
    let settings = LobbySettings {
        difficulty: request.difficulty,
        categories: request.categories,
    };
    let lobby = Lobby {
        id: Uuid::new_v4(),
        host: player,
        guest: None,
        settings: settings,
        status: models::LobbyStatus::Waiting,
        created_at: chrono::Utc::now(),
    };
    let mut redis = state.redis.clone();
    match cache::create_open_lobby(&mut redis, &lobby).await {
        Ok(()) => {
            return ok_json(json!(lobby));
        }
        Err(_) => return server_error("Internal error creating the lobby"),
    };
}

async fn get_lobbies_handler(State(state): State<AppState>) {}

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
