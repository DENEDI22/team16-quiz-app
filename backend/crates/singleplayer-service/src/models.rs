use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMsg {
    StartGame {
        #[serde(rename = "userId")]
        user_id: String,
    },
    SubmitAnswer {
        #[serde(rename = "questionId")]
        question_id: String,
        #[serde(rename = "answerId")]
        answer_id: String,
        #[serde(rename = "timeToAnswer")]
        time_to_answer: u64,
    },
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMsg {
    GameStarted {
        #[serde(rename = "sessionId")]
        session_id: String,
        #[serde(rename = "livesRemaining")]
        lives_remaining: u8,
    },
    Question {
        #[serde(rename = "questionId")]
        question_id: String,
        #[serde(rename = "questionText")]
        question_text: String,
        options: Vec<AnswerOption>,
        #[serde(rename = "questionIndex")]
        question_index: usize,
    },
    AnswerResult {
        correct: bool,
        #[serde(rename = "correctAnswerId")]
        correct_answer_id: String,
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
    pub id: String,
    pub text: String,
}

#[derive(Debug, Clone)]
pub struct PreparedQuestion {
    pub question_id: String,
    pub question_text: String,
    pub options: Vec<AnswerOption>,
    pub correct_answer_id: String,
}

#[derive(Debug, Deserialize)]
pub struct QuizServiceResponse {
    pub data: QuizQuestion,
}

#[derive(Debug, Deserialize)]
pub struct QuizQuestion {
    pub id: Option<i32>,
    pub question: String,
    pub correct_answer: String,
    pub incorrect_answers: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct PostAnswerPayload {
    #[serde(rename = "questionId")]
    pub question_id: String,
    #[serde(rename = "userId")]
    pub user_id: String,
    #[serde(rename = "answerId")]
    pub answer_id: String,
    #[serde(rename = "isCorrect")]
    pub is_correct: bool,
    pub timestamp: String,
    #[serde(rename = "timeToAnswer")]
    pub time_to_answer: u64,
    #[serde(rename = "isMultiplayer")]
    pub is_multiplayer: bool,
    #[serde(rename = "sessionId")]
    pub session_id: String,
}
