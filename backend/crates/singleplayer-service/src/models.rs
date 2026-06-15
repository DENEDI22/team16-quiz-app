use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMsg {
    /// Carries the user's access token; the user identity comes from its
    /// claims, never from a client-supplied id (docs/api-contracts.md §2.4).
    StartGame { token: String },
    SubmitAnswer {
        /// The client's *current* access token, refreshed as needed; the
        /// freshest valid one is forwarded to scoreboard-service.
        token: String,
        #[serde(rename = "questionId")]
        question_id: Uuid,
        /// 1-based option index (docs/api-contracts.md §1.2).
        #[serde(rename = "answerId")]
        answer_id: i32,
        /// Duration in milliseconds (docs/api-contracts.md §1.6).
        #[serde(rename = "timeToAnswerMs")]
        time_to_answer_ms: i32,
    },
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMsg {
    GameStarted {
        #[serde(rename = "sessionId")]
        session_id: Uuid,
        #[serde(rename = "livesRemaining")]
        lives_remaining: u8,
    },
    Question {
        #[serde(rename = "questionId")]
        question_id: Uuid,
        #[serde(rename = "questionText")]
        question_text: String,
        options: Vec<AnswerOption>,
        #[serde(rename = "questionIndex")]
        question_index: usize,
    },
    AnswerResult {
        correct: bool,
        #[serde(rename = "correctAnswerId")]
        correct_answer_id: i32,
        #[serde(rename = "totalScore")]
        total_score: i32,
        #[serde(rename = "livesRemaining")]
        lives_remaining: u8,
    },
    GameOver {
        #[serde(rename = "totalScore")]
        total_score: i32,
        #[serde(rename = "correctAnswers")]
        correct_answers: usize,
    },
    Error {
        message: String,
    },
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
    /// The question's concrete category and difficulty, forwarded to
    /// scoreboard-service so it can build category/difficulty leaderboards.
    pub category: String,
    pub difficulty: String,
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
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub difficulty: String,
}

/// Body for scoreboard-service's `POST /post-answer`. The user is identified by
/// the forwarded bearer token, so no user id appears here.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PostAnswerPayload {
    pub question_id: Uuid,
    pub answer_id: i32,
    pub is_correct: bool,
    pub timestamp: String,
    pub time_to_answer_ms: i32,
    pub is_multiplayer: bool,
    pub session_id: Uuid,
    pub category: String,
    pub difficulty: String,
}

/// Body for scoreboard-service's `POST /singleplayer-result`, sent once at game
/// over. The user is identified by the forwarded bearer token.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SinglePlayerResultPayload {
    pub session_id: Uuid,
    pub score: i32,
    pub correct_answers: i32,
    /// The session's selected difficulty bucket, or `All`.
    pub difficulty: String,
    /// The session's selected categories, or `All`.
    pub categories: String,
    pub timestamp: String,
}
