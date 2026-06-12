use redis::{AsyncCommands, RedisResult, aio::ConnectionManager};
use uuid::Uuid;

use crate::models::{Lobby, PlayerInfo};

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

/// Claims the guest slot. Runs as a Lua script so the whole
/// check-and-update is one atomic Redis operation: two players racing to
/// join can never both succeed, because Redis executes scripts one at a
/// time. The script signals failures via error codes (the first word of
/// `redis.error_reply`), which `e.code()` exposes on the Rust side.
const JOIN_LOBBY_SCRIPT: &str = r#"
local raw = redis.call('GET', KEYS[1])
if not raw then
    return redis.error_reply('NOTFOUND lobby does not exist')
end
local lobby = cjson.decode(raw)
if lobby.host.id == ARGV[2] then
    return redis.error_reply('OWNLOBBY cannot join your own lobby')
end
if lobby.status ~= 'waiting' or lobby.guest ~= nil then
    return redis.error_reply('LOBBYFULL lobby already has a guest')
end
lobby.guest = cjson.decode(ARGV[1])
lobby.status = 'full'
local ttl = redis.call('TTL', KEYS[1])
if ttl <= 0 then
    ttl = 1800
end
local updated = cjson.encode(lobby)
redis.call('SET', KEYS[1], updated, 'EX', ttl)
redis.call('SREM', KEYS[2], ARGV[3])
return updated
"#;

pub async fn join_lobby(
    manager: &mut ConnectionManager,
    lobby_id: Uuid,
    guest: &PlayerInfo,
) -> Result<Lobby, JoinError> {
    let guest_json = serde_json::to_string(guest).expect("player info is always serializable");
    let result: Result<String, redis::RedisError> = redis::Script::new(JOIN_LOBBY_SCRIPT)
        .key(format!("lobby:{lobby_id}"))
        .key("lobbies:open")
        .arg(guest_json)
        .arg(guest.id.to_string())
        .arg(lobby_id.to_string())
        .invoke_async(manager)
        .await;

    match result {
        Ok(raw) => serde_json::from_str(&raw).map_err(|e| JoinError::Internal(e.to_string())),
        Err(e) => match e.code() {
            Some("NOTFOUND") => Err(JoinError::NotFound),
            Some("LOBBYFULL") => Err(JoinError::Full),
            Some("OWNLOBBY") => Err(JoinError::OwnLobby),
            _ => Err(JoinError::Internal(e.to_string())),
        },
    }
}

/// Removes the lobby blob and its entry in the open set — the exact mirror
/// of create_open_lobby.
pub async fn delete_lobby(manager: &mut ConnectionManager, id: Uuid) -> RedisResult<()> {
    let id = id.to_string();
    let _: () = manager.del(format!("lobby:{id}")).await?;
    let _: () = manager.srem("lobbies:open", id).await?;
    Ok(())
}
