use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::prelude::FromRow;
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Answer {
    pub id: Uuid,
    pub question: Uuid,
    pub user_id: Uuid,
    pub answer_id: i32,
    pub is_correct: bool,
    pub timestamp: DateTime<Utc>,
    pub time_to_answer: i32,
    pub is_multiplayer: bool,
    pub session_id: Uuid,
}

#[derive(Serialize, Deserialize, Debug, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct DuelResults {
    #[serde(rename = "duelId")]
    pub id: Uuid,
    pub session_id: Uuid,
    pub host_user_id: Uuid,
    pub guest_user_id: Uuid,
    pub host_score: i32,
    pub guest_score: i32,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateAnswerRequest {
    pub question_id: Uuid,
    // `user_id` is intentionally omitted: it is taken from the authenticated
    // JWT (claims.id), not trusted from the request body.
    pub answer_id: i32,
    pub is_correct: bool,
    pub timestamp: DateTime<Utc>,
    pub time_to_answer: i32,
    pub is_multiplayer: bool,
    pub session_id: Uuid,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateDuelResultRequest {
    pub session_id: Uuid,
    pub host_user_id: Uuid,
    pub guest_user_id: Uuid,
    pub host_score: i32,
    pub guest_score: i32,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuestionStats {
    pub question_id: String,
    pub total_answers: u32,
    pub question_type: QuestionType,
    pub correct_answer_id: String,
    pub options: Vec<AnswerOption>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnswerOption {
    pub answer_id: String,
    pub percentage: f32,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum QuestionType {
    Multiple,
    TrueFalse,
}
