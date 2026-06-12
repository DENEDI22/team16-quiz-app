use redis::{AsyncCommands, RedisResult, aio::ConnectionManager};
use uuid::Uuid;

use crate::models::{Lobby, LobbyStatus, PlayerInfo};

pub enum JoinError {
    NotFound,
    Full,
    OwnLobby,
    Internal(String),
}

pub async fn get_open_lobbies(manager: &mut ConnectionManager) -> RedisResult<Vec<Lobby>> {
    let ids: Vec<String> = manager.smembers("lobbies:open").await?;
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let keys: Vec<String> = ids.iter().map(|id| format!("lobby:{id}")).collect();
    let raw: Vec<Option<String>> = manager.mget(&keys).await?;

    let mut lobbies = Vec::new();
    let mut stale = Vec::new();

    for (id, value) in ids.into_iter().zip(raw) {
        match value {
            Some(json) => match serde_json::from_str::<Lobby>(&json) {
                Ok(lobby) => lobbies.push(lobby),
                Err(_) => stale.push(id),
            },
            None => stale.push(id),
        }
    }

    if !stale.is_empty() {
        let _: () = manager.srem("lobbies:open", &stale).await?;
    }

    Ok(lobbies)
}

pub async fn create_open_lobby(manager: &mut ConnectionManager, lobby: &Lobby) -> RedisResult<()> {
    let lobby_json = serde_json::to_string(&lobby).expect("Could not serialize a lobby");
    let id = lobby.id.to_string();
    let _: () = manager
        .set_ex(format!("lobby:{id}"), lobby_json, 1800)
        .await?;
    let _: () = manager.sadd("lobbies:open", id).await?;

    Ok(())
}

pub async fn get_lobby_by_key(
    manager: &mut ConnectionManager,
    id: Uuid,
) -> RedisResult<Option<Lobby>> {
    let raw: Option<String> = manager.get(format!("lobby:{id}")).await?;
    Ok(raw.and_then(|json| serde_json::from_str(&json).ok()))
}

/// Compare-and-set: writes the updated lobby only if the stored blob is
/// byte-identical to what we read. Validation and JSON handling stay in
/// Rust — re-encoding JSON inside Lua is a trap (cjson turns an empty
/// array into `{}`). Redis runs scripts one at a time, so two players
/// racing to join can never both pass the compare.
const CLAIM_LOBBY_SCRIPT: &str = r#"
local raw = redis.call('GET', KEYS[1])
if not raw then
    return redis.error_reply('NOTFOUND lobby does not exist')
end
if raw ~= ARGV[1] then
    return redis.error_reply('CONFLICT lobby changed concurrently')
end
local ttl = redis.call('TTL', KEYS[1])
if ttl <= 0 then
    ttl = 1800
end
redis.call('SET', KEYS[1], ARGV[2], 'EX', ttl)
redis.call('SREM', KEYS[2], ARGV[3])
return 1
"#;

pub async fn join_lobby(
    manager: &mut ConnectionManager,
    lobby_id: Uuid,
    guest: &PlayerInfo,
) -> Result<Lobby, JoinError> {
    // Read-validate-modify in Rust, write with CAS; on a concurrent change
    // re-read and re-validate (the rival probably took the slot).
    for _ in 0..3 {
        let raw: Option<String> = manager
            .get(format!("lobby:{lobby_id}"))
            .await
            .map_err(|e| JoinError::Internal(e.to_string()))?;
        let Some(raw) = raw else {
            return Err(JoinError::NotFound);
        };
        let Ok(mut lobby) = serde_json::from_str::<Lobby>(&raw) else {
            return Err(JoinError::NotFound); // unreadable blob counts as gone
        };

        if lobby.host.id == guest.id {
            return Err(JoinError::OwnLobby);
        }
        if lobby.status != LobbyStatus::Waiting || lobby.guest.is_some() {
            return Err(JoinError::Full);
        }
        lobby.guest = Some(guest.clone());
        lobby.status = LobbyStatus::Full;
        let updated = serde_json::to_string(&lobby).expect("lobby is always serializable");

        let result: Result<i64, redis::RedisError> = redis::Script::new(CLAIM_LOBBY_SCRIPT)
            .key(format!("lobby:{lobby_id}"))
            .key("lobbies:open")
            .arg(&raw)
            .arg(&updated)
            .arg(lobby_id.to_string())
            .invoke_async(manager)
            .await;

        match result {
            Ok(_) => return Ok(lobby),
            Err(e) => match e.code() {
                Some("NOTFOUND") => return Err(JoinError::NotFound),
                Some("CONFLICT") => continue,
                _ => return Err(JoinError::Internal(e.to_string())),
            },
        }
    }
    // Lost the CAS race repeatedly — someone else got the slot.
    Err(JoinError::Full)
}

/// Removes the lobby blob and its entry in the open set — the exact mirror
/// of create_open_lobby.
pub async fn delete_lobby(manager: &mut ConnectionManager, id: Uuid) -> RedisResult<()> {
    let id = id.to_string();
    let _: () = manager.del(format!("lobby:{id}")).await?;
    let _: () = manager.srem("lobbies:open", id).await?;
    Ok(())
}
