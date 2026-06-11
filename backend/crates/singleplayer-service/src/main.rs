mod models;
mod ws_handler;

use axum::{
    Router,
    extract::{Query, State, WebSocketUpgrade},
    response::IntoResponse,
    routing::get,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;

#[derive(Clone)]
pub struct AppState {
    pub http_client: reqwest::Client,
    pub quiz_service_url: String,
    pub scoreboard_service_url: String,
    pub jwt_secret: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GameSettings {
    pub categories: Option<String>,
    pub difficulty: Option<String>,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    dotenvy::dotenv().ok();

    let quiz_service_url = std::env::var("QUIZ_SERVICE_URL").expect("QUIZ_SERVICE_URL must be set");
    let scoreboard_service_url =
        std::env::var("SCOREBOARD_SERVICE_URL").expect("SCOREBOARD_SERVICE_URL must be set");
    let address = std::env::var("ADDRESS").expect("ADDRESS must be set");
    let jwt_secret = std::env::var("JWT_SECRET").expect("JWT_SECRET must be set");

    let state = AppState {
        http_client: reqwest::Client::new(),
        quiz_service_url,
        scoreboard_service_url,
        jwt_secret,
    };

    let app = Router::new()
        .route("/health", get(health))
        .route("/ws", get(ws_upgrade))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let listener = TcpListener::bind(&address)
        .await
        .expect("Address must be free and valid");
    tracing::info!("singleplayer-service listening on {}", address);
    axum::serve(listener, app)
        .await
        .expect("Error serving application");
}

async fn ws_upgrade(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    Query(settings): Query<GameSettings>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| ws_handler::handle_socket(socket, state, settings))
}

async fn health() -> impl IntoResponse {
    axum::Json(json!({ "status": "healthy" }))
}
