use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct LoginRequest {
    pub(crate) email: String,
    pub(crate) password: String,
}

#[derive(Deserialize, Serialize)]
pub struct RegisterRequest {
    pub(crate) email: String,
    pub(crate) password: String,
    username: String,
}
