use axum::{Json, Router, response::IntoResponse, routing::get};
use serde_json::json;

#[tokio::main]
async fn main() {
    // build our application with a single route
    let app = Router::new().route("/health", get(health));

    // listen globally on port 3000
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("Server started successfully at 0.0.0.0:3000");
    axum::serve(listener, app).await.unwrap();
}

async fn health() -> impl IntoResponse {
    "healthy"
}
