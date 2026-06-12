use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Lobby {
    pub id: Uuid,
    /// Display name so players can recognize a friend's lobby in the list.
    pub name: String,
    pub host: PlayerInfo,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub guest: Option<PlayerInfo>,
    pub settings: LobbySettings,
    pub status: LobbyStatus,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerInfo {
    pub id: Uuid,
    /// Public display name from the JWT; never expose the email here —
    /// lobbies and duel messages are visible to other users.
    pub username: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LobbySettings {
    pub difficulty: String,
    pub categories: Vec<String>,
    pub question_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LobbyStatus {
    Waiting,
    Full,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateLobbyRequest {
    pub name: String,
    pub difficulty: String,
    pub categories: Vec<String>,
    pub question_count: u32,
}

// ---------------------------------------------------------------------------
// Duel WebSocket protocol (mirrors singleplayer-service's message style:
// tagged with `type`, snake_case tags, camelCase fields).
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", rename_all_fields = "camelCase")]
pub enum ClientMsg {
    /// First message on every (re)connect; identity comes from the token's
    /// claims (docs/api-contracts.md §2.4).
    Hello { token: String },
    SubmitAnswer {
        /// The client's *current* access token, refreshed as needed (§2.4).
        token: String,
        /// 1-based index of the question being answered.
        question_index: usize,
        /// 1-based option index (docs/api-contracts.md §1.2).
        answer_id: i32,
    },
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", rename_all_fields = "camelCase")]
pub enum ServerMsg {
    /// Sent to the host while no guest has joined yet.
    Waiting,
    GameStarted {
        session_id: Uuid,
        host: PlayerInfo,
        guest: PlayerInfo,
        total_questions: usize,
    },
    Question {
        question_index: usize,
        question_id: Uuid,
        question_text: String,
        options: Vec<AnswerOption>,
    },
    /// Ends a question for both players. Within the 5-second window each
    /// player may answer once; the question resolves when the window ends
    /// (if anyone answered), when both have answered, or — if the window
    /// ran out with no answers — as soon as the first answer arrives.
    QuestionResult {
        question_index: usize,
        correct_answer_id: i32,
        /// `None` = this player did not answer.
        host_result: Option<PlayerAnswerResult>,
        guest_result: Option<PlayerAnswerResult>,
        host_score: i32,
        guest_score: i32,
    },
    /// Snapshot for a player that reconnected mid-game; followed by a fresh
    /// `Question` message for the current question.
    Resumed {
        session_id: Uuid,
        host: PlayerInfo,
        guest: PlayerInfo,
        host_score: i32,
        guest_score: i32,
        question_index: usize,
        total_questions: usize,
    },
    OpponentDisconnected,
    OpponentReconnected,
    GameOver {
        host_score: i32,
        guest_score: i32,
        /// `None` on a draw.
        winner: Option<Uuid>,
    },
    Error {
        message: String,
    },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerAnswerResult {
    pub answer_id: i32,
    pub correct: bool,
    pub score_delta: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct AnswerOption {
    /// 1-based option index (docs/api-contracts.md §1.2).
    pub id: i32,
    pub text: String,
}

#[derive(Debug, Clone)]
pub struct PreparedQuestion {
    pub question_id: Uuid,
    pub question_text: String,
    pub options: Vec<AnswerOption>,
    pub correct_answer_id: i32,
}

/// quiz-service's `{ "success": true, "data": … }` envelope around a question.
#[derive(Debug, Deserialize)]
pub struct QuizServiceResponse {
    pub data: QuizQuestion,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuizQuestion {
    pub question_id: Uuid,
    pub question: String,
    pub correct_answer: String,
    pub incorrect_answers: Vec<String>,
}

/// Body for scoreboard-service's `POST /post-answer`. The user is identified
/// by the forwarded bearer token, so no user id appears here.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PostAnswerPayload {
    pub question_id: Uuid,
    pub answer_id: i32,
    pub is_correct: bool,
    pub timestamp: String,
    pub time_to_answer_seconds: i32,
    pub is_multiplayer: bool,
    pub session_id: Uuid,
}

/// Body for scoreboard-service's `POST /duel-results`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DuelResultPayload {
    pub session_id: Uuid,
    pub host_user_id: Uuid,
    pub guest_user_id: Uuid,
    pub host_score: i32,
    pub guest_score: i32,
    pub timestamp: String,
}
