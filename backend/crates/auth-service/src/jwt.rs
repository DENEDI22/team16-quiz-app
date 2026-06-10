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
            id: user.id,
            email: user.email,
            role: user_role,
            exp: (Utc::now() + Duration::minutes(10)).timestamp(),
        },
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| e.to_string());

    token
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::jwt::{UserRole, decode_jwt};

    fn sample_user(is_admin: bool) -> User {
        User {
            id: uuid::Uuid::nil(),
            email: "user@example.com".to_string(),
            password_hash: String::new(),
            username: "tester".to_string(),
            is_admin,
        }
    }

    #[test]
    fn hashing_produces_a_verifiable_hash() {
        let hash = hash_password("correct horse battery staple").expect("hashing should succeed");

        assert!(verify_password("correct horse battery staple", &hash));
    }

    #[test]
    fn hashing_is_salted_so_two_hashes_differ() {
        let a = hash_password("same-password").unwrap();
        let b = hash_password("same-password").unwrap();

        assert_ne!(a, b, "argon2 should use a random salt per hash");
    }

    #[test]
    fn verify_rejects_the_wrong_password() {
        let hash = hash_password("the-right-password").unwrap();

        assert!(!verify_password("the-wrong-password", &hash));
    }

    #[test]
    fn verify_rejects_a_malformed_hash() {
        assert!(!verify_password("anything", "not-a-valid-phc-string"));
    }

    #[test]
    fn jwt_encodes_admin_role_and_email() {
        let secret = "issuer-secret".to_string();
        let token =
            get_jwt(sample_user(true), secret.clone()).expect("jwt issuance should succeed");

        let claims = decode_jwt(&token, secret).expect("issued token should decode");
        assert_eq!(claims.email, "user@example.com");
        assert!(matches!(claims.role, UserRole::Admin));
        assert!(claims.exp > 0);
    }

    #[test]
    fn jwt_encodes_non_admin_as_user_role() {
        let secret = "issuer-secret".to_string();
        let token = get_jwt(sample_user(false), secret.clone()).unwrap();

        let claims = decode_jwt(&token, secret).unwrap();
        assert!(matches!(claims.role, UserRole::User));
    }
}
