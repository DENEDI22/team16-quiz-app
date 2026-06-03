use sqlx::{PgPool, postgres::PgPoolOptions};

use crate::scraper::Question;

pub async fn create_pool(url: &str) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new().max_connections(5).connect(url).await
}

pub async fn migrate(pool: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS questions (
            id              SERIAL PRIMARY KEY,
            category        TEXT NOT NULL,
            difficulty      TEXT NOT NULL,
            question        TEXT NOT NULL UNIQUE,
            correct_answer  TEXT NOT NULL,
            incorrect_answers TEXT[] NOT NULL,
            created_at      TIMESTAMPTZ DEFAULT NOW()
        )",
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn insert_questions(pool: &PgPool, questions: &[Question]) -> Result<u64, sqlx::Error> {
    let mut inserted = 0u64;
    for q in questions {
        let rows = sqlx::query(
            "INSERT INTO questions (category, difficulty, question, correct_answer, incorrect_answers)
             VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT (question) DO NOTHING",
        )
        .bind(&q.category)
        .bind(&q.difficulty)
        .bind(&q.question)
        .bind(&q.correct_answer)
        .bind(&q.incorrect_answers)
        .execute(pool)
        .await?
        .rows_affected();
        inserted += rows;
    }
    Ok(inserted)
}

pub async fn load_questions(pool: &PgPool) -> Result<Vec<Question>, sqlx::Error> {
    sqlx::query_as::<_, Question>(
        "SELECT category, difficulty, question, correct_answer, incorrect_answers FROM questions",
    )
    .fetch_all(pool)
    .await
}
