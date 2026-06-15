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
    AnswerOption, DuelCheckpoint, DuelResultPayload, Lobby, LobbySettings, PlayerAnswerResult,
    PlayerInfo, PostAnswerPayload, PreparedQuestion, QuizQuestion, QuizServiceResponse, ServerMsg,
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
        if let Some(tx) = &self.outbound
            && tx.try_send(msg).is_err()
        {
            tracing::warn!(
                "dropping message to {}: channel full or closed",
                self.info.id
            );
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
    let guest = guest.expect("guest is connected when the waiting phase ends");

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

    run_game(
        state, lobby_id, events, host, guest, session_id, questions, 0,
    )
    .await;
}

/// Rebuilds a duel from its Redis checkpoint after a service restart and
/// continues play at the checkpointed question. Spawned by ws.rs when a
/// player reconnects and no live actor exists.
pub async fn resume_task(
    state: AppState,
    lobby_id: Uuid,
    checkpoint: DuelCheckpoint,
    events: mpsc::Receiver<DuelEvent>,
) {
    tracing::info!(
        "resuming duel {lobby_id} at question {}/{}",
        checkpoint.next_index + 1,
        checkpoint.questions.len()
    );
    let mut host = PlayerConn::new(checkpoint.host);
    host.score = checkpoint.host_score;
    let mut guest = PlayerConn::new(checkpoint.guest);
    guest.score = checkpoint.guest_score;
    // Nobody is attached yet; the reconnecting player's Connect event is
    // already queued and the in-game Connect handler sends them Resumed plus
    // the current question. The opponent attaches the same way when they
    // return.
    run_game(
        &state,
        lobby_id,
        events,
        host,
        guest,
        checkpoint.session_id,
        checkpoint.questions,
        checkpoint.next_index,
    )
    .await;
    state.duels.lock().unwrap().remove(&lobby_id);
    tracing::info!("resumed duel actor for lobby {lobby_id} ended");
}

/// The question loop plus game over. Writes a checkpoint to Redis at every
/// question boundary, so a dead process can be resumed losing at most the
/// question that was in flight (it restarts on resume).
#[allow(clippy::too_many_arguments)]
async fn run_game(
    state: &AppState,
    lobby_id: Uuid,
    mut events: mpsc::Receiver<DuelEvent>,
    mut host: PlayerConn,
    mut guest: PlayerConn,
    session_id: Uuid,
    questions: Vec<PreparedQuestion>,
    start_index: usize,
) {
    let total_questions = questions.len();
    let mut redis = state.redis.clone();

    // Resume point before the first (or resumed-at) question.
    save_checkpoint(
        &mut redis,
        lobby_id,
        session_id,
        &host,
        &guest,
        &questions,
        start_index,
    )
    .await;

    // ---- Phase 3: question loop --------------------------------------------
    for index0 in start_index..questions.len() {
        let question = &questions[index0];
        let question_index = index0 + 1;

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

        // Both players may answer (once each) within the window. The question
        // resolves when the window ends with at least one answer, when both
        // have answered, or — if the window ran out empty — on the first
        // answer that eventually arrives ("overtime").
        let mut window_open = true;
        let mut host_answer: Option<(i32, f64)> = None;
        let mut guest_answer: Option<(i32, f64)> = None;
        loop {
            let event = if window_open {
                tokio::select! {
                    _ = &mut deadline => {
                        window_open = false;
                        if host_answer.is_some() || guest_answer.is_some() {
                            break;
                        }
                        continue; // overtime: wait for the first answer
                    }
                    ev = events.recv() => match ev {
                        Some(ev) => ev,
                        None => return,
                    },
                }
            } else {
                match events.recv().await {
                    Some(ev) => ev,
                    None => return,
                }
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
                    let is_host = user_id == host.info.id;
                    if !is_host && user_id != guest.info.id {
                        continue;
                    }
                    let player = if is_host { &mut host } else { &mut guest };
                    // Keep forwarding the freshest valid token (§2.4).
                    if token != player.token && decode_jwt(&token, &state.jwt_secret).is_ok() {
                        player.token = token;
                    }
                    let slot = if is_host {
                        &mut host_answer
                    } else {
                        &mut guest_answer
                    };
                    if slot.is_some() {
                        continue; // one answer per player per question
                    }
                    *slot = Some((answer_id, started.elapsed().as_secs_f64()));
                    if !window_open {
                        break; // overtime ends with the first answer
                    }
                    if host_answer.is_some() && guest_answer.is_some() {
                        break; // both are in, no reason to wait out the clock
                    }
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

        // Resolve both players' answers and notify everyone.
        let host_result = resolve_answer(state, session_id, &mut host, host_answer, question);
        let guest_result = resolve_answer(state, session_id, &mut guest, guest_answer, question);
        let result_msg = ServerMsg::QuestionResult {
            question_index,
            correct_answer_id: question.correct_answer_id,
            host_result,
            guest_result,
            host_score: host.score,
            guest_score: guest.score,
        };
        host.send(result_msg.clone());
        guest.send(result_msg);

        // Checkpoint immediately after resolving, BEFORE the result pause:
        // a crash from here on resumes at the next question instead of
        // replaying (and double-scoring) one that already finished.
        save_checkpoint(
            &mut redis,
            lobby_id,
            session_id,
            &host,
            &guest,
            &questions,
            index0 + 1,
        )
        .await;

        if question_index < total_questions {
            sleep(Duration::from_secs(RESULT_PAUSE_SECONDS)).await;
        }
    }

    // ---- Phase 4: game over --------------------------------------------------
    if let Err(e) = cache::delete_duel_checkpoint(&mut redis, lobby_id).await {
        tracing::warn!("duel {lobby_id}: failed to delete checkpoint: {e}");
    }
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

#[allow(clippy::too_many_arguments)]
async fn save_checkpoint(
    redis: &mut redis::aio::ConnectionManager,
    lobby_id: Uuid,
    session_id: Uuid,
    host: &PlayerConn,
    guest: &PlayerConn,
    questions: &[PreparedQuestion],
    next_index: usize,
) {
    let checkpoint = DuelCheckpoint {
        session_id,
        host: host.info.clone(),
        host_score: host.score,
        guest: guest.info.clone(),
        guest_score: guest.score,
        questions: questions.to_vec(),
        next_index,
    };
    if let Err(e) = cache::save_duel_checkpoint(redis, lobby_id, &checkpoint).await {
        tracing::warn!("duel {lobby_id}: failed to write checkpoint: {e}");
    }
}

/// Applies one player's answer (if any): updates their score, reports the
/// answer to the scoreboard, and returns the per-player result for the
/// `QuestionResult` broadcast.
fn resolve_answer(
    state: &AppState,
    session_id: Uuid,
    player: &mut PlayerConn,
    answer: Option<(i32, f64)>,
    question: &PreparedQuestion,
) -> Option<PlayerAnswerResult> {
    let (answer_id, elapsed) = answer?;
    let correct = answer_id == question.correct_answer_id;
    let score_delta = score_delta(correct, elapsed);
    player.score += score_delta;
    post_answer_to_scoreboard(
        state.clone(),
        player.token.clone(),
        session_id,
        question.question_id,
        answer_id,
        correct,
        (elapsed * 1000.0).round() as i32,
        question.category.clone(),
        question.difficulty.clone(),
    );
    Some(PlayerAnswerResult {
        answer_id,
        correct,
        score_delta,
    })
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
        category: q.category,
        difficulty: q.difficulty,
    }
}

/// Fire-and-forget: a slow scoreboard must never stall the game loop.
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
            is_multiplayer: true,
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
            category: "Science: Computers".to_string(),
            difficulty: "easy".to_string(),
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
