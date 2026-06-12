//! The duel actor: one tokio task per running duel owns all game state and
//! serializes both players' events through a single channel, so the game
//! logic itself is free of locks and races ("who answered first" is simply
//! "whose event arrived first").

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use chrono::Utc;
use reqwest::header::AUTHORIZATION;
use serde::Serialize;
use shared::jwt::decode_jwt;
use tokio::sync::mpsc;
use tokio::time::{Instant, sleep, sleep_until};
use uuid::Uuid;

use crate::models::{
    AnswerOption, DuelResultPayload, Lobby, LobbySettings, PlayerInfo, PostAnswerPayload,
    PreparedQuestion, QuizQuestion, QuizServiceResponse, ServerMsg,
};
use crate::{AppState, cache};

const QUESTION_SECONDS: u64 = 5;
/// Pause after each question result so clients can show it.
const RESULT_PAUSE_SECONDS: u64 = 2;
/// How long an actor waits for both players before giving up (matches the
/// lobby TTL in Redis).
const WAITING_TIMEOUT_SECONDS: u64 = 1800;
/// Per-question fetch attempts are capped at `count * this` so a too-small
/// question pool can never loop forever.
const FETCH_ATTEMPT_FACTOR: usize = 4;

/// Connections look up the duel's event sender here; the actor removes its
/// own entry when it ends.
pub type DuelRegistry = Arc<Mutex<HashMap<Uuid, mpsc::Sender<DuelEvent>>>>;

pub enum DuelEvent {
    Connect {
        /// Identifies this physical connection so a stale disconnect can't
        /// detach a newer one after a reconnect.
        conn_id: Uuid,
        player: PlayerInfo,
        token: String,
        outbound: mpsc::Sender<ServerMsg>,
    },
    Answer {
        user_id: Uuid,
        question_index: usize,
        answer_id: i32,
        token: String,
    },
    Disconnect {
        conn_id: Uuid,
        user_id: Uuid,
    },
}

struct PlayerConn {
    info: PlayerInfo,
    token: String,
    conn_id: Uuid,
    outbound: Option<mpsc::Sender<ServerMsg>>,
    score: i32,
}

impl PlayerConn {
    fn new(info: PlayerInfo) -> Self {
        Self {
            info,
            token: String::new(),
            conn_id: Uuid::nil(),
            outbound: None,
            score: 0,
        }
    }

    fn connected(&self) -> bool {
        self.outbound.is_some()
    }

    fn attach(&mut self, conn_id: Uuid, token: String, outbound: mpsc::Sender<ServerMsg>) {
        self.conn_id = conn_id;
        self.token = token;
        self.outbound = Some(outbound);
    }

    fn detach(&mut self, conn_id: Uuid) -> bool {
        if self.conn_id == conn_id && self.outbound.is_some() {
            self.outbound = None;
            true
        } else {
            false
        }
    }

    /// Best-effort send; a slow or gone client must never block the game.
    fn send(&self, msg: ServerMsg) {
        if let Some(tx) = &self.outbound {
            if tx.try_send(msg).is_err() {
                tracing::warn!("dropping message to {}: channel full or closed", self.info.id);
            }
        }
    }
}

pub async fn duel_task(state: AppState, lobby: Lobby, events: mpsc::Receiver<DuelEvent>) {
    let lobby_id = lobby.id;
    run_duel(&state, lobby, events).await;
    state.duels.lock().unwrap().remove(&lobby_id);
    tracing::info!("duel actor for lobby {lobby_id} ended");
}

async fn run_duel(state: &AppState, lobby: Lobby, mut events: mpsc::Receiver<DuelEvent>) {
    let lobby_id = lobby.id;
    let settings = lobby.settings.clone();
    let mut host = PlayerConn::new(lobby.host.clone());
    let mut guest: Option<PlayerConn> = lobby.guest.clone().map(PlayerConn::new);

    // ---- Phase 1: wait until both players are connected --------------------
    let waiting_deadline = Instant::now() + Duration::from_secs(WAITING_TIMEOUT_SECONDS);
    loop {
        let event = tokio::select! {
            ev = events.recv() => match ev {
                Some(ev) => ev,
                None => return,
            },
            _ = sleep_until(waiting_deadline) => {
                tracing::info!("duel {lobby_id}: nobody showed up, giving up");
                return;
            }
        };

        match event {
            DuelEvent::Connect {
                conn_id,
                player,
                token,
                outbound,
            } => {
                if player.id == host.info.id {
                    host.attach(conn_id, token, outbound);
                } else {
                    // The guest may have REST-joined after this actor was
                    // spawned, so our lobby snapshot can be stale: re-read.
                    if guest.is_none() {
                        let mut redis = state.redis.clone();
                        if let Ok(Some(fresh)) = cache::get_lobby_by_key(&mut redis, lobby_id).await
                        {
                            guest = fresh.guest.map(PlayerConn::new);
                        }
                    }
                    match guest.as_mut() {
                        Some(g) if g.info.id == player.id => g.attach(conn_id, token, outbound),
                        _ => {
                            let _ = outbound.try_send(ServerMsg::Error {
                                message: "you are not part of this lobby".to_string(),
                            });
                            continue;
                        }
                    }
                }

                let guest_connected = guest.as_ref().is_some_and(|g| g.connected());
                if host.connected() && guest_connected {
                    break;
                }
                if host.connected() && !guest_connected {
                    host.send(ServerMsg::Waiting);
                }
            }
            DuelEvent::Disconnect { conn_id, user_id } => {
                if user_id == host.info.id {
                    host.detach(conn_id);
                } else if let Some(g) = guest.as_mut() {
                    g.detach(conn_id);
                }
                // Everyone gone: stop the actor. The lobby stays in Redis, so
                // coming back later simply spawns a fresh actor.
                if !host.connected() && !guest.as_ref().is_some_and(|g| g.connected()) {
                    return;
                }
            }
            DuelEvent::Answer { .. } => {} // no game yet, ignore
        }
    }
    let mut guest = guest.expect("guest is connected when the waiting phase ends");

    // ---- Phase 2: setup ----------------------------------------------------
    // The duel now lives in this task's memory; the lobby is no longer
    // joinable and can leave Redis.
    let mut redis = state.redis.clone();
    if let Err(e) = cache::delete_lobby(&mut redis, lobby_id).await {
        tracing::warn!("duel {lobby_id}: failed to delete lobby from redis: {e}");
    }

    let questions = match fetch_questions(state, &host.token, &settings).await {
        Ok(q) => q,
        Err(message) => {
            let msg = ServerMsg::Error { message };
            host.send(msg.clone());
            guest.send(msg);
            return;
        }
    };

    let session_id = Uuid::new_v4();
    let total_questions = questions.len();
    let started_msg = ServerMsg::GameStarted {
        session_id,
        host: host.info.clone(),
        guest: guest.info.clone(),
        total_questions,
    };
    host.send(started_msg.clone());
    guest.send(started_msg);

    // ---- Phase 3: question loop --------------------------------------------
    for (i, question) in questions.iter().enumerate() {
        let question_index = i + 1;
        let question_msg = ServerMsg::Question {
            question_index,
            question_id: question.question_id,
            question_text: question.question_text.clone(),
            options: question.options.clone(),
        };
        host.send(question_msg.clone());
        guest.send(question_msg.clone());

        let started = Instant::now();
        let deadline = sleep(Duration::from_secs(QUESTION_SECONDS));
        tokio::pin!(deadline);

        // First valid answer or the deadline ends the question.
        let mut outcome: Option<(Uuid, i32, f64)> = None;
        loop {
            let event = tokio::select! {
                _ = &mut deadline => break,
                ev = events.recv() => match ev {
                    Some(ev) => ev,
                    None => return,
                },
            };
            match event {
                DuelEvent::Answer {
                    user_id,
                    question_index: answered_index,
                    answer_id,
                    token,
                } => {
                    if answered_index != question_index {
                        continue; // stale answer from a previous question
                    }
                    let player = if user_id == host.info.id {
                        &mut host
                    } else if user_id == guest.info.id {
                        &mut guest
                    } else {
                        continue;
                    };
                    // Keep forwarding the freshest valid token (§2.4).
                    if token != player.token && decode_jwt(&token, &state.jwt_secret).is_ok() {
                        player.token = token;
                    }
                    outcome = Some((user_id, answer_id, started.elapsed().as_secs_f64()));
                    break;
                }
                DuelEvent::Connect {
                    conn_id,
                    player,
                    token,
                    outbound,
                } => {
                    let is_host = player.id == host.info.id;
                    if !is_host && player.id != guest.info.id {
                        let _ = outbound.try_send(ServerMsg::Error {
                            message: "you are not part of this duel".to_string(),
                        });
                        continue;
                    }
                    if is_host {
                        host.attach(conn_id, token, outbound);
                    } else {
                        guest.attach(conn_id, token, outbound);
                    }
                    let resumed = ServerMsg::Resumed {
                        session_id,
                        host: host.info.clone(),
                        guest: guest.info.clone(),
                        host_score: host.score,
                        guest_score: guest.score,
                        question_index,
                        total_questions,
                    };
                    let (target, other) = if is_host {
                        (&host, &guest)
                    } else {
                        (&guest, &host)
                    };
                    target.send(resumed);
                    target.send(question_msg.clone());
                    other.send(ServerMsg::OpponentReconnected);
                }
                DuelEvent::Disconnect { conn_id, user_id } => {
                    let (target, other) = if user_id == host.info.id {
                        (&mut host, &guest)
                    } else if user_id == guest.info.id {
                        (&mut guest, &host)
                    } else {
                        continue;
                    };
                    if target.detach(conn_id) {
                        other.send(ServerMsg::OpponentDisconnected);
                    }
                    if !host.connected() && !guest.connected() {
                        tracing::info!("duel {lobby_id}: both players gone, abandoning");
                        return;
                    }
                }
            }
        }

        // Resolve the question and notify both players.
        let (answered_by, correct) = match outcome {
            Some((user_id, answer_id, elapsed)) => {
                let correct = answer_id == question.correct_answer_id;
                let player = if user_id == host.info.id {
                    &mut host
                } else {
                    &mut guest
                };
                player.score += score_delta(correct, elapsed);
                post_answer_to_scoreboard(
                    state.clone(),
                    player.token.clone(),
                    session_id,
                    question.question_id,
                    answer_id,
                    correct,
                    elapsed.round() as i32,
                );
                (Some(user_id), correct)
            }
            None => (None, false),
        };
        let result_msg = ServerMsg::QuestionResult {
            question_index,
            answered_by,
            correct,
            correct_answer_id: question.correct_answer_id,
            host_score: host.score,
            guest_score: guest.score,
        };
        host.send(result_msg.clone());
        guest.send(result_msg);

        if question_index < total_questions {
            sleep(Duration::from_secs(RESULT_PAUSE_SECONDS)).await;
        }
    }

    // ---- Phase 4: game over --------------------------------------------------
    let winner = match host.score.cmp(&guest.score) {
        std::cmp::Ordering::Greater => Some(host.info.id),
        std::cmp::Ordering::Less => Some(guest.info.id),
        std::cmp::Ordering::Equal => None,
    };
    let over_msg = ServerMsg::GameOver {
        host_score: host.score,
        guest_score: guest.score,
        winner,
    };
    host.send(over_msg.clone());
    guest.send(over_msg);

    post_duel_result(state, &host.token, session_id, &host, &guest).await;
}

/// Score for a resolved question: `100 * (1 / seconds)` for a correct answer,
/// clamped so answers faster than one second score exactly 100, and 0 for a
/// wrong answer.
fn score_delta(correct: bool, elapsed_seconds: f64) -> i32 {
    if !correct {
        return 0;
    }
    (100.0 / elapsed_seconds.max(1.0)).round() as i32
}

/// Query-string parameters understood by quiz-service's `GET /questions`.
#[derive(Serialize)]
struct QuestionQuery {
    categories: Option<String>,
    difficulty: Option<String>,
}

async fn fetch_questions(
    state: &AppState,
    token: &str,
    settings: &LobbySettings,
) -> Result<Vec<PreparedQuestion>, String> {
    let query = QuestionQuery {
        categories: (!settings.categories.is_empty()).then(|| settings.categories.join(",")),
        difficulty: (!settings.difficulty.is_empty()).then(|| settings.difficulty.clone()),
    };

    let count = settings.question_count as usize;
    let mut seen = HashSet::new();
    let mut questions = Vec::with_capacity(count);
    for _ in 0..count * FETCH_ATTEMPT_FACTOR {
        if questions.len() >= count {
            break;
        }
        let q = fetch_one_question(state, token, &query).await?;
        if seen.insert(q.question_id) {
            questions.push(prepare_question(q));
        }
    }

    if questions.is_empty() {
        return Err("no questions available for these settings".to_string());
    }
    if questions.len() < count {
        tracing::warn!(
            "question pool too small: wanted {count}, playing with {}",
            questions.len()
        );
    }
    Ok(questions)
}

async fn fetch_one_question(
    state: &AppState,
    token: &str,
    query: &QuestionQuery,
) -> Result<QuizQuestion, String> {
    let resp = state
        .http_client
        .get(format!("{}/questions", state.quiz_service_url))
        .header(AUTHORIZATION, format!("Bearer {token}"))
        .query(query)
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
    Ok(body.data)
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
    }
}

/// Fire-and-forget: a slow scoreboard must never stall the game loop.
fn post_answer_to_scoreboard(
    state: AppState,
    token: String,
    session_id: Uuid,
    question_id: Uuid,
    answer_id: i32,
    is_correct: bool,
    time_to_answer_seconds: i32,
) {
    tokio::spawn(async move {
        let payload = PostAnswerPayload {
            question_id,
            answer_id,
            is_correct,
            timestamp: Utc::now().to_rfc3339(),
            time_to_answer_seconds,
            is_multiplayer: true,
            session_id,
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

async fn post_duel_result(
    state: &AppState,
    token: &str,
    session_id: Uuid,
    host: &PlayerConn,
    guest: &PlayerConn,
) {
    let payload = DuelResultPayload {
        session_id,
        host_user_id: host.info.id,
        guest_user_id: guest.info.id,
        host_score: host.score,
        guest_score: guest.score,
        timestamp: Utc::now().to_rfc3339(),
    };
    let result = state
        .http_client
        .post(format!("{}/duel-results", state.scoreboard_service_url))
        .header(AUTHORIZATION, format!("Bearer {token}"))
        .json(&payload)
        .send()
        .await;
    match result {
        Ok(resp) if !resp.status().is_success() => {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            tracing::warn!("scoreboard rejected duel result: {status}: {body}");
        }
        Ok(_) => {}
        Err(e) => tracing::warn!("failed to post duel result to scoreboard: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrong_answers_score_zero() {
        assert_eq!(score_delta(false, 0.5), 0);
        assert_eq!(score_delta(false, 4.9), 0);
    }

    #[test]
    fn instant_answers_are_clamped_to_100() {
        assert_eq!(score_delta(true, 0.01), 100);
        assert_eq!(score_delta(true, 1.0), 100);
    }

    #[test]
    fn slower_answers_score_less() {
        assert_eq!(score_delta(true, 2.0), 50);
        assert_eq!(score_delta(true, 4.0), 25);
        assert_eq!(score_delta(true, 5.0), 20);
    }

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
        }
    }

    #[test]
    fn options_are_sorted_with_one_based_ids() {
        let prepared = prepare_question(sample_question());
        let ids: Vec<i32> = prepared.options.iter().map(|o| o.id).collect();
        assert_eq!(ids, vec![1, 2, 3, 4]);

        let correct = prepared
            .options
            .iter()
            .find(|o| o.id == prepared.correct_answer_id)
            .expect("correct option exists");
        assert_eq!(correct.text, "Central Processing Unit");
    }
}
