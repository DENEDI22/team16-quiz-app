mod db;
mod scraper;

use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use serde_json::json;
use sqlx::PgPool;
use tokio::net::TcpListener;
use tokio_cron_scheduler::{Job, JobScheduler};

#[derive(Clone)]
struct AppState {
    pool: PgPool,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    // DATABASE_URL=postgres://(user)):(password)!@(address))/(db)
    dotenvy::dotenv().ok();
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    let pool = db::create_pool(&database_url)
        .await
        .expect("Failed to connect to database");
    db::create_table(&pool)
        .await
        .expect("Failed to create table");

    run_scrape(&pool).await;
    start_cron(pool.clone()).await;

    let app = Router::new()
        .route("/health", get(health))
        .route("/questions", get(get_question))
        .route("/scrape", post(manual_scrape))
        .with_state(AppState { pool });

    // 0.0.0.0:(port)
    let address = std::env::var("ADDRESS").expect("ADDRESS must be set");
    let listener = TcpListener::bind(&address)
        .await
        .expect("Address must be free and valid");
    tracing::info!("quiz-service listening on {}", address);
    axum::serve(listener, app)
        .await
        .expect("Error serving application");
}

async fn start_cron(pool: PgPool) {
    let scheduler = JobScheduler::new()
        .await
        .expect("Failed to create scheduler");

    // CRON_JOB="0 0 * * * *"
    let cron_job = std::env::var("CRON_JOB").expect("CRON_JOB must be set");

    let job = Job::new_async(cron_job, move |_uuid, _lock| {
        let pool = pool.clone();
        Box::pin(async move {
            run_scrape(&pool).await;
        })
    })
    .expect("Failed to create cron job");

    scheduler.add(job).await.expect("Failed to add cron job");
    scheduler.start().await.expect("Failed to start scheduler");
}

async fn run_scrape(pool: &PgPool) {
    tracing::info!("Scraping questions from OpenTDB...");
    match scraper::fetch_questions(50).await {
        Ok(questions) => match db::insert_questions(pool, &questions).await {
            Ok(inserted) => tracing::info!("Persisted {} new questions to DB", inserted),
            Err(e) => tracing::error!("DB insert failed: {}", e),
        },
        Err(e) => tracing::error!("Scrape failed: {}", e),
    }
}

async fn get_question(State(state): State<AppState>) -> impl IntoResponse {
    match db::get_random_question(&state.pool).await {
        Ok(Some(q)) => (StatusCode::OK, Json(json!({ "success": true, "data": q }))),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "message": "No questions in database yet" })),
        ),
        Err(e) => {
            tracing::error!("DB error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "success": false, "message": "Database error" })),
            )
        }
    }
}

async fn manual_scrape(State(state): State<AppState>) -> impl IntoResponse {
    run_scrape(&state.pool).await;
    (
        StatusCode::OK,
        Json(json!({ "success": true, "message": "Scrape triggered" })),
    )
}

async fn health() -> impl IntoResponse {
    Json(json!({ "status": "healthy" }))
}
