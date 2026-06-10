use sqlx::PgPool;
use uuid::Uuid;

use crate::models::{CreateAnswerRequest, CreateDuelResultRequest, DuelResults, QuestionStats};
use crate::stats::build_question_stats;

pub async fn migrate(pool: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query(
        "
CREATE TABLE IF NOT EXISTS answers (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    question uuid NOT NULL,
    user_id uuid NOT NULL,
    answer_id int NOT NULL,
    is_correct bool NOT NULL,
    timestamp timestamptz NOT NULL,
    time_to_answer int NOT NULL,
    is_multiplayer bool NOT NULL,
    session_id uuid NOT NULL,
    CONSTRAINT answers_pk PRIMARY KEY (id)
)
        ",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "
CREATE TABLE IF NOT EXISTS duel_results (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    session_id uuid NOT NULL,
    host_user_id uuid NOT NULL,
    guest_user_id uuid NOT NULL,
    host_score int NOT NULL,
    guest_score int NOT NULL,
    timestamp timestamptz NOT NULL,
    CONSTRAINT duel_results_pk PRIMARY KEY (id)
)
        ",
    )
    .execute(pool)
    .await?;

    Ok(())
}

/// Persists a single answer for the authenticated `user_id` (taken from the JWT,
/// not from the request body) and returns the generated record id.
pub async fn insert_answer(
    pool: &PgPool,
    user_id: Uuid,
    req: &CreateAnswerRequest,
) -> Result<Uuid, sqlx::Error> {
    let row: (Uuid,) = sqlx::query_as(
        "INSERT INTO answers
            (question, user_id, answer_id, is_correct, timestamp, time_to_answer, is_multiplayer, session_id)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
         RETURNING id",
    )
    .bind(req.question_id)
    .bind(user_id)
    .bind(req.answer_id)
    .bind(req.is_correct)
    .bind(req.timestamp)
    .bind(req.time_to_answer)
    .bind(req.is_multiplayer)
    .bind(req.session_id)
    .fetch_one(pool)
    .await?;
    Ok(row.0)
}

pub async fn insert_duel_result(
    pool: &PgPool,
    req: &CreateDuelResultRequest,
) -> Result<Uuid, sqlx::Error> {
    let row: (Uuid,) = sqlx::query_as(
        "INSERT INTO duel_results
            (session_id, host_user_id, guest_user_id, host_score, guest_score, timestamp)
         VALUES ($1, $2, $3, $4, $5, $6)
         RETURNING id",
    )
    .bind(req.session_id)
    .bind(req.host_user_id)
    .bind(req.guest_user_id)
    .bind(req.host_score)
    .bind(req.guest_score)
    .bind(req.timestamp)
    .fetch_one(pool)
    .await?;
    Ok(row.0)
}

pub async fn get_user_duels(pool: &PgPool, user_id: Uuid) -> Result<Vec<DuelResults>, sqlx::Error> {
    sqlx::query_as::<_, DuelResults>(
        "SELECT id, session_id, host_user_id, guest_user_id, host_score, guest_score, timestamp
         FROM duel_results
         WHERE host_user_id = $1 OR guest_user_id = $1
         ORDER BY timestamp DESC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

pub async fn get_question_stats(
    pool: &PgPool,
    question_id: Uuid,
) -> Result<Option<QuestionStats>, sqlx::Error> {
    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM answers WHERE question = $1")
        .bind(question_id)
        .fetch_one(pool)
        .await?;

    if total == 0 {
        return Ok(None);
    }

    let counts: Vec<(i32, i64)> = sqlx::query_as(
        "SELECT answer_id, COUNT(*)
         FROM answers
         WHERE question = $1
         GROUP BY answer_id
         ORDER BY answer_id",
    )
    .bind(question_id)
    .fetch_all(pool)
    .await?;

    let correct_answer_id: Option<i32> = sqlx::query_scalar(
        "SELECT answer_id FROM answers WHERE question = $1 AND is_correct = true LIMIT 1",
    )
    .bind(question_id)
    .fetch_optional(pool)
    .await?;

    Ok(Some(build_question_stats(
        question_id,
        total,
        &counts,
        correct_answer_id,
    )))
}
