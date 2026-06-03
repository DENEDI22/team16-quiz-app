use std::env::var;

use argon2::{Argon2, PasswordHash, PasswordVerifier};
use chrono::{Duration, Utc};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use sqlx::prelude::FromRow;

#[derive(Deserialize, Serialize)]
pub enum UserRole {
    User,
    Admin,
}

#[derive(FromRow, Deserialize, Serialize)]
pub struct User {
    id: uuid::Uuid,
    email: String,
    pub(crate) password_hash: String,
    username: String,
    is_admin: bool,
}

#[derive(Deserialize, Serialize)]
pub struct Claims {
    email: String,
    role: UserRole,
    exp: i64,
}

pub fn verify_password(password: &str, hash: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(hash) else {
        return false;
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

pub fn get_jwt(user: User, secret: String) -> Result<String, String> {
    let user_role = if user.is_admin {
        UserRole::Admin
    } else {
        UserRole::User
    };
    let token = encode(
        &Header::default(),
        &Claims {
            email: user.email,
            role: user_role,
            exp: (Utc::now() + Duration::minutes(10)).timestamp(),
        },
        &EncodingKey::from_secret(secret.as_bytes()),
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
