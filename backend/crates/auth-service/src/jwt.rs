use axum::{
    extract::{FromRequest, FromRequestParts},
    http::{Response, header},
};
use chrono::{Duration, Utc};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Deserialize, Serialize)]
pub enum UserRole {
    User,
    Admin,
}

#[derive(Deserialize)]
pub struct User {
    email: String,
    password: String,
}

#[derive(Deserialize, Serialize)]
pub struct Claims {
    email: String,
    role: UserRole,
    exp: i64,
}

pub fn get_jwt(user: User) -> Result<String, String> {
    let token = encode(
        &Header::default(),
        &Claims {
            email: user.email,
            role: UserRole::Admin,
            exp: (Utc::now() + Duration::minutes(10)).timestamp(),
        },
        &EncodingKey::from_secret("team-16-secret-key".as_bytes()),
    )
    .map_err(|e| e.to_string());

    token
}

pub fn decode_jwt(token: &str) -> Result<Claims, String> {
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret("team-16-secret-key".as_bytes()),
        &Validation::default(),
    );

    match token_data {
        Ok(token_data) => Ok(token_data.claims),

        Err(e) => Err(e.to_string()),
    }
}
