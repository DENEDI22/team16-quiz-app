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
    pub time_to_answer_ms: i32,
    pub is_multiplayer: bool,
    pub session_id: Uuid,
    pub category: String,
    pub difficulty: String,
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
    /// 1-based option index (docs/api-contracts.md §1.2).
    pub answer_id: i32,
    pub is_correct: bool,
    pub timestamp: DateTime<Utc>,
    /// Duration in milliseconds (docs/api-contracts.md §1.6).
    pub time_to_answer_ms: i32,
    pub is_multiplayer: bool,
    pub session_id: Uuid,
    /// The question's concrete category, denormalized for category leaderboards.
    pub category: String,
    /// The question's concrete difficulty (`easy` | `medium` | `hard`).
    pub difficulty: String,
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

/// Body for `POST /singleplayer-result`. The user is taken from the JWT.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSinglePlayerResultRequest {
    pub session_id: Uuid,
    pub score: i32,
    pub correct_answers: i32,
    /// The session's selected difficulty bucket: `easy` | `medium` | `hard` | `All`.
    pub difficulty: String,
    /// The session's selected categories, or `All`.
    pub categories: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuestionStats {
    pub question_id: Uuid,
    pub total_answers: u32,
    pub question_type: QuestionType,
    /// 1-based option index; 0 when no correct answer was recorded.
    pub correct_answer_id: i32,
    pub options: Vec<AnswerOption>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnswerOption {
    /// 1-based option index (docs/api-contracts.md §1.2).
    pub answer_id: i32,
    pub percentage: f32,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum QuestionType {
    Multiple,
    TrueFalse,
}

#[derive(Debug, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct UserHighscore {
    pub user_id: Uuid,
    pub total_answers: i64,
    pub correct_answers: i64,
}

#[derive(Debug, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct AnswerHistoryEntry {
    pub id: Uuid,
    pub question: Uuid,
    pub answer_id: i32,
    pub is_correct: bool,
    pub timestamp: DateTime<Utc>,
    pub time_to_answer_ms: i32,
    pub is_multiplayer: bool,
    pub session_id: Uuid,
    pub category: String,
    pub difficulty: String,
}

// ---------------------------------------------------------------------------
// Account overview (GET /account-stats)
// ---------------------------------------------------------------------------

/// Combined personal stats for the account overview page. All aggregation is
/// done server-side; the frontend only renders these values (Req 2).
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountStats {
    /// Best singleplayer score for each difficulty bucket the user has played.
    pub highscores_per_difficulty: Vec<DifficultyHighscore>,
    /// The user's last 10 duels, newest first.
    pub last_duels: Vec<AccountDuel>,
    /// Average of the user's own score across all their duels.
    pub avg_multiplayer_score: f64,
    pub duels_played: i64,
    /// Average answer time across all of the user's answers, in milliseconds.
    pub avg_time_to_answer_ms: f64,
    /// Wins / duels played (draws are not wins). 0.0 when no duels played.
    pub win_rate: f64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DifficultyHighscore {
    pub difficulty: String,
    pub highscore: i32,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountDuel {
    pub duel_id: Uuid,
    pub session_id: Uuid,
    pub opponent_id: Uuid,
    pub opponent_username: String,
    pub own_score: i32,
    pub opponent_score: i32,
    /// `win` | `loss` | `draw` from this user's perspective.
    pub outcome: String,
    pub timestamp: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Global leaderboards (GET /leaderboard/*)
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DuelLeaderboardEntry {
    pub user_id: Uuid,
    pub username: String,
    pub duels_won: i64,
}

/// Top-10 singleplayer highscores grouped by difficulty bucket.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SinglePlayerLeaderboard {
    pub difficulty: String,
    pub entries: Vec<SinglePlayerLeaderboardEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SinglePlayerLeaderboardEntry {
    pub user_id: Uuid,
    pub username: String,
    pub highscore: i32,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryLeaderboardEntry {
    pub user_id: Uuid,
    pub username: String,
    pub total_answers: i64,
    pub correct_answers: i64,
    /// Correct / total, in the range 0.0–1.0.
    pub accuracy: f64,
}
