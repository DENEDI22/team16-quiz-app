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

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{
        "response_code": 0,
        "results": [
            {
                "type": "multiple",
                "category": "Science: Computers",
                "difficulty": "medium",
                "question": "What does CPU stand for?",
                "correct_answer": "Central Processing Unit",
                "incorrect_answers": [
                    "Central Process Unit",
                    "Computer Personal Unit",
                    "Central Processor Unit"
                ]
            }
        ]
    }"#;

    #[test]
    fn parses_an_opentdb_response() {
        let parsed: OpenTdbResponse = serde_json::from_str(SAMPLE).unwrap();

        assert_eq!(parsed.response_code, 0);
        assert_eq!(parsed.results.len(), 1);

        let q = &parsed.results[0];
        assert_eq!(q.category, "Science: Computers");
        assert_eq!(q.difficulty, "medium");
        assert_eq!(q.question, "What does CPU stand for?");
        assert_eq!(q.correct_answer, "Central Processing Unit");
        assert_eq!(q.incorrect_answers.len(), 3);
        assert_eq!(q.incorrect_answers[0], "Central Process Unit");
    }

    #[test]
    fn ignores_unknown_fields_like_type() {
        // OpenTDB sends a "type" field that Question does not model; it must be ignored.
        let parsed: OpenTdbResponse = serde_json::from_str(SAMPLE).unwrap();
        assert_eq!(parsed.results[0].correct_answer, "Central Processing Unit");
    }

    #[test]
    fn deserializes_a_bare_question() {
        let q: Question = serde_json::from_str(
            r#"{
                "category": "History",
                "difficulty": "easy",
                "question": "Q?",
                "correct_answer": "A",
                "incorrect_answers": ["B", "C", "D"]
            }"#,
        )
        .unwrap();

        assert_eq!(q.category, "History");
        assert_eq!(q.incorrect_answers, vec!["B", "C", "D"]);
    }

    #[test]
    fn rejects_a_response_missing_results() {
        let result: Result<OpenTdbResponse, _> = serde_json::from_str(r#"{"response_code": 0}"#);

        assert!(result.is_err());
    }
}
