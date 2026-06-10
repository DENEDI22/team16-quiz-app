use axum::extract::ws::{Message, WebSocket};
use chrono::Utc;
use uuid::Uuid;

use crate::AppState;
use crate::models::{
    AnswerOption, ClientMsg, PostAnswerPayload, PreparedQuestion, QuizServiceResponse, ServerMsg,
};

pub async fn handle_socket(mut socket: WebSocket, state: AppState) {
    let (user_id, session_id) = match wait_for_start(&mut socket).await {
        Some(v) => v,
        None => return,
    };

    const MAX_LIVES: u8 = 3;
    let mut lives: u8 = MAX_LIVES;

    if send_msg(
        &mut socket,
        &ServerMsg::GameStarted {
            session_id: session_id.clone(),
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
        let question = match fetch_question(&state).await {
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
                question_id: question.question_id.clone(),
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

        let (submitted_id, answer_id, time_to_answer) = match wait_for_answer(&mut socket).await {
            Some(v) => v,
            None => return,
        };

        if submitted_id != question.question_id {
            send_error(&mut socket, "unexpected questionId in submit_answer").await;
            return;
        }

        let correct = answer_id == question.correct_answer_id;

        post_answer_to_scoreboard(
            state.clone(),
            user_id.clone(),
            session_id.clone(),
            question.question_id.clone(),
            answer_id.clone(),
            correct,
            time_to_answer,
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
                correct_answer_id: question.correct_answer_id.clone(),
                total_score,
                lives_remaining: lives,
            },
        )
        .await
        .ok();

        if lives == 0 {
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

async fn wait_for_start(socket: &mut WebSocket) -> Option<(String, String)> {
    while let Some(Ok(msg)) = socket.recv().await {
        if let Message::Text(text) = msg {
            match serde_json::from_str::<ClientMsg>(&text) {
                Ok(ClientMsg::StartGame { user_id }) => {
                    let session_id = format!("sess_{}", &Uuid::new_v4().to_string()[..8]);
                    return Some((user_id, session_id));
                }
                _ => {
                    send_error(socket, "expected start_game message").await;
                    return None;
                }
            }
        }
    }
    None
}

async fn wait_for_answer(socket: &mut WebSocket) -> Option<(String, String, u64)> {
    while let Some(Ok(msg)) = socket.recv().await {
        if let Message::Text(text) = msg {
            match serde_json::from_str::<ClientMsg>(&text) {
                Ok(ClientMsg::SubmitAnswer {
                    question_id,
                    answer_id,
                    time_to_answer,
                }) => return Some((question_id, answer_id, time_to_answer)),
                _ => {
                    send_error(socket, "expected submit_answer message").await;
                    return None;
                }
            }
        }
    }
    None
}

async fn fetch_question(state: &AppState) -> Result<PreparedQuestion, String> {
    let resp = state
        .http_client
        .get(format!("{}/questions", state.quiz_service_url))
        .send()
        .await
        .map_err(|e| format!("quiz-service unreachable: {e}"))?;

    if !resp.status().is_success() {
        return Err("quiz-service returned no question".into());
    }

    let body: QuizServiceResponse = resp
        .json()
        .await
        .map_err(|e| format!("failed to parse quiz-service response: {e}"))?;

    Ok(prepare_question(body.data))
}

fn prepare_question(q: crate::models::QuizQuestion) -> PreparedQuestion {
    let mut all_options: Vec<String> = q.incorrect_answers.clone();
    all_options.push(q.correct_answer.clone());
    all_options.sort();

    let options: Vec<AnswerOption> = all_options
        .iter()
        .enumerate()
        .map(|(i, text)| AnswerOption {
            id: format!("a_{}", i + 1),
            text: text.clone(),
        })
        .collect();

    let correct_answer_id = options
        .iter()
        .find(|o| o.text == q.correct_answer)
        .map(|o| o.id.clone())
        .unwrap_or_else(|| "a_1".to_string());

    let question_id = match q.id {
        Some(id) => format!("q_{}", id),
        None => {
            let hash = q
                .question
                .bytes()
                .fold(0u32, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u32));
            format!("q_{}", hash)
        }
    };

    PreparedQuestion {
        question_id,
        question_text: q.question,
        options,
        correct_answer_id,
    }
}

fn post_answer_to_scoreboard(
    state: AppState,
    user_id: String,
    session_id: String,
    question_id: String,
    answer_id: String,
    is_correct: bool,
    time_to_answer: u64,
) {
    tokio::spawn(async move {
        let payload = PostAnswerPayload {
            question_id,
            user_id,
            answer_id,
            is_correct,
            timestamp: Utc::now().to_rfc3339(),
            time_to_answer,
            is_multiplayer: false,
            session_id,
        };
        let result = state
            .http_client
            .post(format!("{}/post-answer", state.scoreboard_service_url))
            .json(&payload)
            .send()
            .await;
        if let Err(e) = result {
            tracing::warn!("failed to post answer to scoreboard: {e}");
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
