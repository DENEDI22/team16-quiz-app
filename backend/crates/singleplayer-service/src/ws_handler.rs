use axum::extract::ws::{Message, WebSocket};
use chrono::Utc;
use reqwest::header::AUTHORIZATION;
use shared::jwt::decode_jwt;
use uuid::Uuid;

use crate::models::{
    AnswerOption, ClientMsg, PostAnswerPayload, PreparedQuestion, QuizQuestion,
    QuizServiceResponse, ServerMsg, SinglePlayerResultPayload,
};
use crate::{AppState, GameSettings};

pub async fn handle_socket(mut socket: WebSocket, state: AppState, settings: GameSettings) {
    let mut token = match wait_for_start(&mut socket, &state).await {
        Some(t) => t,
        None => return,
    };
    let session_id = Uuid::new_v4();

    const MAX_LIVES: u8 = 3;
    let mut lives: u8 = MAX_LIVES;

    if send_msg(
        &mut socket,
        &ServerMsg::GameStarted {
            session_id,
            lives_remaining: lives,
        },
    )
    .await
    .is_err()
    {
        return;
    }

    let mut total_score: i32 = 0;
    let mut correct_count: usize = 0;
    let mut question_index: usize = 0;

    loop {
        let question = match fetch_question(&state, &token, &settings).await {
            Ok(q) => q,
            Err(e) => {
                send_error(&mut socket, &e).await;
                return;
            }
        };

        question_index += 1;

        if send_msg(
            &mut socket,
            &ServerMsg::Question {
                question_id: question.question_id,
                question_text: question.question_text.clone(),
                options: question.options.clone(),
                question_index,
            },
        )
        .await
        .is_err()
        {
            return;
        }

        let (submitted_id, answer_id, time_to_answer_ms, submitted_token) =
            match wait_for_answer(&mut socket).await {
                Some(v) => v,
                None => return,
            };

        // Keep forwarding the freshest valid token the client has sent (§2.4).
        if submitted_token != token {
            match decode_jwt(&submitted_token, &state.jwt_secret) {
                Ok(_) => token = submitted_token,
                Err(e) => tracing::warn!("ignoring invalid refreshed token: {e}"),
            }
        }

        if submitted_id != question.question_id {
            send_error(&mut socket, "unexpected questionId in submit_answer").await;
            return;
        }

        let correct = answer_id == question.correct_answer_id;

        post_answer_to_scoreboard(
            state.clone(),
            token.clone(),
            session_id,
            question.question_id,
            answer_id,
            correct,
            time_to_answer_ms,
            question.category.clone(),
            question.difficulty.clone(),
        );

        if correct {
            total_score += 100;
            correct_count += 1;
        } else {
            lives -= 1;
        }

        send_msg(
            &mut socket,
            &ServerMsg::AnswerResult {
                correct,
                correct_answer_id: question.correct_answer_id,
                total_score,
                lives_remaining: lives,
            },
        )
        .await
        .ok();

        if lives == 0 {
            post_singleplayer_result(
                state.clone(),
                token.clone(),
                session_id,
                total_score,
                correct_count as i32,
                &settings,
            );

            send_msg(
                &mut socket,
                &ServerMsg::GameOver {
                    total_score,
                    correct_answers: correct_count,
                },
            )
            .await
            .ok();

            return;
        }
    }
}

/// Waits for `start_game`, validates its token, and returns the token.
async fn wait_for_start(socket: &mut WebSocket, state: &AppState) -> Option<String> {
    while let Some(Ok(msg)) = socket.recv().await {
        if let Message::Text(text) = msg {
            match serde_json::from_str::<ClientMsg>(&text) {
                Ok(ClientMsg::StartGame { token }) => match decode_jwt(&token, &state.jwt_secret) {
                    Ok(_claims) => return Some(token),
                    Err(e) => {
                        send_error(socket, &format!("invalid token: {e}")).await;
                        return None;
                    }
                },
                _ => {
                    send_error(socket, "expected start_game message").await;
                    return None;
                }
            }
        }
    }
    None
}

async fn wait_for_answer(socket: &mut WebSocket) -> Option<(Uuid, i32, i32, String)> {
    while let Some(Ok(msg)) = socket.recv().await {
        if let Message::Text(text) = msg {
            match serde_json::from_str::<ClientMsg>(&text) {
                Ok(ClientMsg::SubmitAnswer {
                    token,
                    question_id,
                    answer_id,
                    time_to_answer_ms,
                }) => return Some((question_id, answer_id, time_to_answer_ms, token)),
                _ => {
                    send_error(socket, "expected submit_answer message").await;
                    return None;
                }
            }
        }
    }
    None
}

async fn fetch_question(
    state: &AppState,
    token: &str,
    settings: &GameSettings,
) -> Result<PreparedQuestion, String> {
    let resp = state
        .http_client
        .get(format!("{}/questions", state.quiz_service_url))
        .header(AUTHORIZATION, format!("Bearer {token}"))
        .query(&settings)
        .send()
        .await
        .map_err(|e| format!("quiz-service unreachable: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        tracing::warn!("quiz-service returned {status}: {body}");
        return Err(format!("quiz-service returned {status}"));
    }

    let body: QuizServiceResponse = resp
        .json()
        .await
        .map_err(|e| format!("failed to parse quiz-service response: {e}"))?;

    Ok(prepare_question(body.data))
}

/// Builds the canonical option list (docs/api-contracts.md §1.2): all option
/// texts sorted lexicographically, identified by their 1-based index.
fn prepare_question(q: QuizQuestion) -> PreparedQuestion {
    let mut all_options = q.incorrect_answers;
    all_options.push(q.correct_answer.clone());
    all_options.sort();

    let correct_answer_id = all_options
        .iter()
        .position(|text| *text == q.correct_answer)
        .map(|i| (i + 1) as i32)
        .expect("correct answer is always one of the options");

    let options = all_options
        .into_iter()
        .enumerate()
        .map(|(i, text)| AnswerOption {
            id: (i + 1) as i32,
            text,
        })
        .collect();

    PreparedQuestion {
        question_id: q.question_id,
        question_text: q.question,
        options,
        correct_answer_id,
        category: q.category,
        difficulty: q.difficulty,
    }
}

#[allow(clippy::too_many_arguments)]
fn post_answer_to_scoreboard(
    state: AppState,
    token: String,
    session_id: Uuid,
    question_id: Uuid,
    answer_id: i32,
    is_correct: bool,
    time_to_answer_ms: i32,
    category: String,
    difficulty: String,
) {
    tokio::spawn(async move {
        let payload = PostAnswerPayload {
            question_id,
            answer_id,
            is_correct,
            timestamp: Utc::now().to_rfc3339(),
            time_to_answer_ms,
            is_multiplayer: false,
            session_id,
            category,
            difficulty,
        };
        let result = state
            .http_client
            .post(format!("{}/post-answer", state.scoreboard_service_url))
            .header(AUTHORIZATION, format!("Bearer {token}"))
            .json(&payload)
            .send()
            .await;
        match result {
            Ok(resp) if !resp.status().is_success() => {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                tracing::warn!("scoreboard rejected answer: {status}: {body}");
            }
            Ok(_) => {}
            Err(e) => tracing::warn!("failed to post answer to scoreboard: {e}"),
        }
    });
}

/// Fire-and-forget POST of the finished game's aggregate score. The session's
/// selected settings become the result's bucket (a concrete value or "All").
fn post_singleplayer_result(
    state: AppState,
    token: String,
    session_id: Uuid,
    score: i32,
    correct_answers: i32,
    settings: &GameSettings,
) {
    let difficulty = settings
        .difficulty
        .clone()
        .filter(|d| !d.is_empty())
        .unwrap_or_else(|| "All".to_string());
    let categories = settings
        .categories
        .clone()
        .filter(|c| !c.is_empty())
        .unwrap_or_else(|| "All".to_string());

    tokio::spawn(async move {
        let payload = SinglePlayerResultPayload {
            session_id,
            score,
            correct_answers,
            difficulty,
            categories,
            timestamp: Utc::now().to_rfc3339(),
        };
        let result = state
            .http_client
            .post(format!("{}/singleplayer-result", state.scoreboard_service_url))
            .header(AUTHORIZATION, format!("Bearer {token}"))
            .json(&payload)
            .send()
            .await;
        match result {
            Ok(resp) if !resp.status().is_success() => {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                tracing::warn!("scoreboard rejected singleplayer result: {status}: {body}");
            }
            Ok(_) => {}
            Err(e) => tracing::warn!("failed to post singleplayer result to scoreboard: {e}"),
        }
    });
}

async fn send_msg(socket: &mut WebSocket, msg: &ServerMsg) -> Result<(), ()> {
    match serde_json::to_string(msg) {
        Ok(json) => socket
            .send(Message::Text(json.into()))
            .await
            .map_err(|_| ()),
        Err(_) => Err(()),
    }
}

async fn send_error(socket: &mut WebSocket, message: &str) {
    send_msg(
        socket,
        &ServerMsg::Error {
            message: message.to_owned(),
        },
    )
    .await
    .ok();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_question() -> QuizQuestion {
        QuizQuestion {
            question_id: Uuid::nil(),
            question: "What does CPU stand for?".to_string(),
            correct_answer: "Central Processing Unit".to_string(),
            incorrect_answers: vec![
                "Central Process Unit".to_string(),
                "Computer Personal Unit".to_string(),
                "Central Processor Unit".to_string(),
            ],
            category: "Science: Computers".to_string(),
            difficulty: "easy".to_string(),
        }
    }

    #[test]
    fn options_are_sorted_with_one_based_ids() {
        let prepared = prepare_question(sample_question());

        let texts: Vec<&str> = prepared.options.iter().map(|o| o.text.as_str()).collect();
        let mut sorted = texts.clone();
        sorted.sort();
        assert_eq!(texts, sorted, "options must be in canonical sorted order");
        assert_eq!(
            prepared.options.iter().map(|o| o.id).collect::<Vec<_>>(),
            vec![1, 2, 3, 4]
        );
    }

    #[test]
    fn correct_answer_id_points_at_the_correct_option() {
        let prepared = prepare_question(sample_question());

        let correct = prepared
            .options
            .iter()
            .find(|o| o.id == prepared.correct_answer_id)
            .expect("correct option exists");
        assert_eq!(correct.text, "Central Processing Unit");
    }

    #[test]
    fn question_id_is_passed_through() {
        let id = Uuid::from_u128(42);
        let mut q = sample_question();
        q.question_id = id;

        assert_eq!(prepare_question(q).question_id, id);
    }
}
