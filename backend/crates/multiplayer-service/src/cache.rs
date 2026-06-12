use redis::{AsyncCommands, RedisResult, aio::ConnectionManager};
use uuid::Uuid;

use crate::models::Lobby;

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
