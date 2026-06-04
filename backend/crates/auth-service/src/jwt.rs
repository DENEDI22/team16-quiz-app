use std::env::var;

use argon2::{
    Argon2, PasswordHash, PasswordHasher, PasswordVerifier,
    password_hash::{SaltString, rand_core::OsRng},
};
use chrono::{Duration, Utc};
use jsonwebtoken::{EncodingKey, Header, encode};
use serde::{Deserialize, Serialize};
use shared::jwt::{Claims, UserRole};
use sqlx::prelude::FromRow;

#[derive(FromRow, Deserialize, Serialize)]
pub struct User {
    id: uuid::Uuid,
    email: String,
    pub(crate) password_hash: String,
    username: String,
    is_admin: bool,
}

pub fn verify_password(password: &str, hash: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(hash) else {
        return false;
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

pub fn hash_password(password: &str) -> Result<String, String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| e.to_string())
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
