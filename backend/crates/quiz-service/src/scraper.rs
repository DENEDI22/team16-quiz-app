use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Question {
    pub category: String,
    pub difficulty: String,
    pub question: String,
    pub correct_answer: String,
    pub incorrect_answers: Vec<String>,
}

#[derive(Deserialize)]
struct OpenTdbResponse {
    response_code: u8,
    results: Vec<Question>,
}

pub async fn fetch_questions(amount: u8) -> Result<Vec<Question>, reqwest::Error> {
    let url = format!(
        "https://opentdb.com/api.php?amount={}&type=multiple",
        amount
    );
    let response: OpenTdbResponse = reqwest::get(&url).await?.json().await?;
    if response.response_code != 0 {
        tracing::warn!(
            "OpenTDB returned non-zero response_code: {}",
            response.response_code
        );
    }
    Ok(response.results)
}
