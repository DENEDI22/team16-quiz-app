mod config;
mod db;
mod scraper;

use std::sync::Arc;

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
use tokio::sync::Mutex;
use tokio_cron_scheduler::{Job, JobScheduler};

use crate::scraper::Question;

type SharedQuestions = Arc<Mutex<Vec<Question>>>;

#[derive(Clone)]
struct AppState {
    questions: SharedQuestions,
    pool: PgPool,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let config_path = std::env::var("QUIZ_CONFIG").unwrap_or_else(|_| "config.toml".to_string());
    let cfg = config::Config::load(&config_path).expect("Failed to load config.toml");

    let pool = db::create_pool(&cfg.database.url)
        .await
        .expect("Failed to connect to database");
    db::migrate(&pool).await.expect("Failed to run migrations");

    let questions: SharedQuestions = Arc::new(Mutex::new(Vec::new()));

    // Pre-fill from DB so memory state survives restarts
    match db::load_questions(&pool).await {
        Ok(existing) => {
            tracing::info!("Loaded {} questions from database", existing.len());
            *questions.lock().await = existing;
        }
        Err(e) => tracing::error!("Failed to load questions from DB: {}", e),
    }

    let state = AppState {
        questions: Arc::clone(&questions),
        pool: pool.clone(),
    };

    run_scrape(&state.questions, &pool).await;
    start_cron(Arc::clone(&questions), pool).await;

    let app = Router::new()
        .route("/health", get(health))
        .route("/questions", get(get_questions))
        .route("/scrape", post(manual_scrape))
        .with_state(state);

    let listener = TcpListener::bind("0.0.0.0:3001")
        .await
        .expect("Address must be free and valid");
    tracing::info!("quiz-service listening on 0.0.0.0:3001");
    axum::serve(listener, app)
        .await
        .expect("Error serving application");
}

async fn start_cron(questions: SharedQuestions, pool: PgPool) {
    let scheduler = JobScheduler::new()
        .await
        .expect("Failed to create scheduler");

    let job = Job::new_async("0 0 * * * *", move |_uuid, _lock| {
        let questions = Arc::clone(&questions);
        let pool = pool.clone();
        Box::pin(async move {
            run_scrape(&questions, &pool).await;
        })
    })
    .expect("Failed to create cron job");

    scheduler.add(job).await.expect("Failed to add cron job");
    scheduler.start().await.expect("Failed to start scheduler");
}

async fn run_scrape(questions: &SharedQuestions, pool: &PgPool) {
    tracing::info!("Scraping questions from OpenTDB...");
    match scraper::fetch_questions(50).await {
        Ok(new_questions) => {
            match db::insert_questions(pool, &new_questions).await {
                Ok(inserted) => tracing::info!("Persisted {} new questions to DB", inserted),
                Err(e) => tracing::error!("DB insert failed: {}", e),
            }
            let count = new_questions.len();
            let mut lock = questions.lock().await;
            lock.extend(new_questions);
            tracing::info!("Scraped {}. Total in memory: {}", count, lock.len());
        }
        Err(e) => tracing::error!("Scrape failed: {}", e),
    }
}

async fn get_questions(State(state): State<AppState>) -> impl IntoResponse {
    let lock = state.questions.lock().await;
    Json(json!({
        "success": true,
        "count": lock.len(),
        "data": *lock,
    }))
}

async fn manual_scrape(State(state): State<AppState>) -> impl IntoResponse {
    run_scrape(&state.questions, &state.pool).await;
    (
        StatusCode::OK,
        Json(json!({ "success": true, "message": "Scrape triggered" })),
    )
}

async fn health() -> impl IntoResponse {
    Json(json!({ "status": "healthy" }))
}
