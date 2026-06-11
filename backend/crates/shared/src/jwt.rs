use chrono::Utc;
use jsonwebtoken::{
    DecodingKey, EncodingKey, Header, Validation, decode, encode, errors::ErrorKind,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Claims {
    /// The authenticated user's id (subject of the token).
    pub id: Uuid,
    pub email: String,
    pub role: UserRole,
    pub exp: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum UserRole {
    User,
    Admin,
}

pub fn encode_jwt(claims: &Claims, secret: &str) -> Result<String, String> {
    encode(
        &Header::default(),
        claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| e.to_string())
}

pub fn decode_jwt(token: &str, secret: &str) -> Result<Claims, String> {
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
    }
}

/// Decodes a token whose signature must be valid but whose `exp` may lie up to
/// `grace_seconds` in the past. Used by the stateless refresh flow
/// (docs/api-contracts.md §2.3).
pub fn decode_jwt_with_grace(
    token: &str,
    secret: &str,
    grace_seconds: i64,
) -> Result<Claims, String> {
    let mut validation = Validation::default();
    validation.validate_exp = false;

    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    );

    let claims = match token_data {
        Ok(token_data) => token_data.claims,
        Err(e) => match e.kind() {
            ErrorKind::InvalidSignature => return Err("Invalid signature".to_string()),
            _ => return Err(e.to_string()),
        },
    };

    if claims.exp + grace_seconds < Utc::now().timestamp() {
        return Err("Token is expired beyond the refresh window".to_string());
    }
    Ok(claims)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn encode_token(secret: &str, email: &str, role: UserRole, exp: i64) -> String {
        encode_jwt(
            &Claims {
                id: Uuid::nil(),
                email: email.to_string(),
                role,
                exp,
            },
            secret,
        )
        .expect("encoding a token should succeed")
    }

    #[test]
    fn decodes_a_valid_token() {
        let secret = "test-secret";
        let exp = (Utc::now() + Duration::minutes(10)).timestamp();
        let token = encode_token(secret, "user@example.com", UserRole::Admin, exp);

        let claims = decode_jwt(&token, secret).expect("token should decode");

        assert_eq!(claims.email, "user@example.com");
        assert_eq!(claims.exp, exp);
        assert!(matches!(claims.role, UserRole::Admin));
    }

    #[test]
    fn rejects_an_expired_token() {
        let secret = "test-secret";
        // Well outside the default validation leeway.
        let exp = (Utc::now() - Duration::hours(1)).timestamp();
        let token = encode_token(secret, "user@example.com", UserRole::User, exp);

        let err = decode_jwt(&token, secret).unwrap_err();

        assert_eq!(err, "Token is expired");
    }

    #[test]
    fn rejects_a_token_signed_with_a_different_secret() {
        let exp = (Utc::now() + Duration::minutes(10)).timestamp();
        let token = encode_token("the-real-secret", "user@example.com", UserRole::User, exp);

        let err = decode_jwt(&token, "a-different-secret").unwrap_err();

        assert_eq!(err, "Invalid signature");
    }

    #[test]
    fn rejects_a_malformed_token() {
        let err = decode_jwt("this.is.not-a-jwt", "test-secret").unwrap_err();

        // Not one of the specially-handled kinds, so the raw error string is returned.
        assert!(!err.is_empty());
        assert_ne!(err, "Token is expired");
        assert_ne!(err, "Invalid signature");
    }

    #[test]
    fn grace_decode_accepts_a_recently_expired_token() {
        let secret = "test-secret";
        let exp = (Utc::now() - Duration::minutes(30)).timestamp();
        let token = encode_token(secret, "user@example.com", UserRole::User, exp);

        let claims = decode_jwt_with_grace(&token, secret, 3600)
            .expect("token expired within the grace window should decode");

        assert_eq!(claims.email, "user@example.com");
    }

    #[test]
    fn grace_decode_rejects_a_token_beyond_the_window() {
        let secret = "test-secret";
        let exp = (Utc::now() - Duration::hours(2)).timestamp();
        let token = encode_token(secret, "user@example.com", UserRole::User, exp);

        let err = decode_jwt_with_grace(&token, secret, 3600).unwrap_err();

        assert_eq!(err, "Token is expired beyond the refresh window");
    }

    #[test]
    fn grace_decode_still_rejects_a_bad_signature() {
        let exp = Utc::now().timestamp();
        let token = encode_token("the-real-secret", "user@example.com", UserRole::User, exp);

        let err = decode_jwt_with_grace(&token, "a-different-secret", 3600).unwrap_err();

        assert_eq!(err, "Invalid signature");
    }
}
