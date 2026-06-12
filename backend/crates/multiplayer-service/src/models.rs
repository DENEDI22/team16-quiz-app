use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Lobby {
    pub id: Uuid,
    pub host: PlayerInfo,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub guest: Option<PlayerInfo>,
    pub settings: LobbySettings,
    pub status: LobbyStatus,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerInfo {
    pub id: Uuid,
    pub email: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LobbySettings {
    pub difficulty: String,
    pub categories: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LobbyStatus {
    Waiting,
    Full,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateLobbyRequest {
    pub difficulty: String,
    pub categories: Vec<String>,
}
