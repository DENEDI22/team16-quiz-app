use chrono::{DateTime, Utc};
use jsonwebtoken::{DecodingKey, Validation, decode, errors::ErrorKind};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct Claims {
    pub email: String,
    pub role: UserRole,
    pub exp: i64,
}

#[derive(Deserialize, Serialize)]
pub enum UserRole {
    User,
    Admin,
}

pub fn decode_jwt(token: &str, secret: String) -> Result<Claims, String> {
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    );

    match token_data {
        Ok(token_data) => Ok(token_data.claims),
        Err(e) => match e.kind() {
            ErrorKind::ExpiredSignature => Err("Token is expired".to_string()),
            ErrorKind::InvalidSignature => Err("Invalid signature".to_string()),
            _ => Err(e.to_string()),
        },

        Err(e) => Err(e.to_string()),
    }
}
