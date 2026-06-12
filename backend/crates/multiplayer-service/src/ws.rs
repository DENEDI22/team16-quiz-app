//! WebSocket plumbing: authenticates the `hello` handshake, finds or spawns
//! the duel actor for the lobby, then pumps messages between the socket and
//! the actor's channels. All game logic lives in `duel.rs`.

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::response::IntoResponse;
use shared::jwt::decode_jwt;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::duel::{DuelEvent, duel_task};
use crate::models::{ClientMsg, PlayerInfo, ServerMsg};
use crate::{AppState, cache};

const OUTBOUND_BUFFER: usize = 64;

pub async fn ws_upgrade(
    Path(lobby_id): Path<Uuid>,
    State(state): State<AppState>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state, lobby_id))
}

async fn handle_socket(mut socket: WebSocket, state: AppState, lobby_id: Uuid) {
    // A browser WebSocket cannot send an Authorization header, so identity
    // arrives in the first message instead (docs/api-contracts.md §2.4).
    let (player, token) = match wait_for_hello(&mut socket, &state).await {
        Some(v) => v,
        None => return,
    };

    let events_tx = match find_or_spawn_duel(&state, lobby_id).await {
        Ok(tx) => tx,
        Err(message) => {
            send_error(&mut socket, &message).await;
            return;
        }
    };

    let conn_id = Uuid::new_v4();
    let (outbound_tx, mut outbound_rx) = mpsc::channel::<ServerMsg>(OUTBOUND_BUFFER);
    let connect = DuelEvent::Connect {
        conn_id,
        player: player.clone(),
        token,
        outbound: outbound_tx,
    };
    if events_tx.send(connect).await.is_err() {
        send_error(&mut socket, "duel is no longer running").await;
        return;
    }

    loop {
        tokio::select! {
            outgoing = outbound_rx.recv() => match outgoing {
                Some(msg) => {
                    if send_msg(&mut socket, &msg).await.is_err() {
                        break;
                    }
                }
                // The actor dropped our sender: the duel ended or we were
                // replaced by a reconnect. Either way, no Disconnect event.
                None => return,
            },
            incoming = socket.recv() => match incoming {
                Some(Ok(Message::Text(text))) => {
                    match serde_json::from_str::<ClientMsg>(&text) {
                        Ok(ClientMsg::SubmitAnswer { token, question_index, answer_id }) => {
                            let answer = DuelEvent::Answer {
                                user_id: player.id,
                                question_index,
                                answer_id,
                                token,
                            };
                            if events_tx.send(answer).await.is_err() {
                                break;
                            }
                        }
                        Ok(ClientMsg::Hello { .. }) => {} // already greeted
                        Err(_) => send_error(&mut socket, "unparseable message").await,
                    }
                }
                Some(Ok(Message::Close(_))) | Some(Err(_)) | None => break,
                Some(Ok(_)) => {} // ping/pong/binary: ignore
            }
        }
    }

    let _ = events_tx
        .send(DuelEvent::Disconnect {
            conn_id,
            user_id: player.id,
        })
        .await;
}

async fn wait_for_hello(socket: &mut WebSocket, state: &AppState) -> Option<(PlayerInfo, String)> {
    while let Some(Ok(msg)) = socket.recv().await {
        let Message::Text(text) = msg else { continue };
        match serde_json::from_str::<ClientMsg>(&text) {
            Ok(ClientMsg::Hello { token }) => match decode_jwt(&token, &state.jwt_secret) {
                Ok(claims) => {
                    let player = PlayerInfo {
                        id: claims.id,
                        email: claims.email,
                    };
                    return Some((player, token));
                }
                Err(e) => {
                    send_error(socket, &format!("invalid token: {e}")).await;
                    return None;
                }
            },
            _ => {
                send_error(socket, "expected hello message").await;
                return None;
            }
        }
    }
    None
}

/// Returns the event sender of the lobby's duel actor, spawning one if this
/// is the first connection. The double-checked locking is needed because the
/// Redis lookup must happen outside the registry lock (it awaits).
async fn find_or_spawn_duel(
    state: &AppState,
    lobby_id: Uuid,
) -> Result<mpsc::Sender<DuelEvent>, String> {
    if let Some(tx) = state.duels.lock().unwrap().get(&lobby_id) {
        return Ok(tx.clone());
    }

    let mut redis = state.redis.clone();
    let lobby = cache::get_lobby_by_key(&mut redis, lobby_id)
        .await
        .map_err(|e| format!("redis error: {e}"))?
        .ok_or_else(|| "lobby does not exist or has expired".to_string())?;

    let mut duels = state.duels.lock().unwrap();
    if let Some(tx) = duels.get(&lobby_id) {
        return Ok(tx.clone()); // someone else spawned it while we hit Redis
    }
    let (tx, rx) = mpsc::channel::<DuelEvent>(64);
    duels.insert(lobby_id, tx.clone());
    tokio::spawn(duel_task(state.clone(), lobby, rx));
    Ok(tx)
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
