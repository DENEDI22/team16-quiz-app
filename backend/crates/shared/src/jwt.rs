use chrono::{DateTime, Utc};
use jsonwebtoken::{DecodingKey, Validation, decode, errors::ErrorKind};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct Claims {
    pub email: String,
    pub role: UserRole,
    pub exp: i64,
}

#[derive(Debug, Deserialize, Serialize)]
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use jsonwebtoken::{EncodingKey, Header, encode};

    fn encode_token(secret: &str, email: &str, role: UserRole, exp: i64) -> String {
        encode(
            &Header::default(),
            &Claims {
                email: email.to_string(),
                role,
                exp,
            },
            &EncodingKey::from_secret(secret.as_bytes()),
        )
        .expect("encoding a token should succeed")
    }

    #[test]
    fn decodes_a_valid_token() {
        let secret = "test-secret";
        let exp = (Utc::now() + Duration::minutes(10)).timestamp();
        let token = encode_token(secret, "user@example.com", UserRole::Admin, exp);

        let claims = decode_jwt(&token, secret.to_string()).expect("token should decode");

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

        let err = decode_jwt(&token, secret.to_string()).unwrap_err();

        assert_eq!(err, "Token is expired");
    }

    #[test]
    fn rejects_a_token_signed_with_a_different_secret() {
        let exp = (Utc::now() + Duration::minutes(10)).timestamp();
        let token = encode_token("the-real-secret", "user@example.com", UserRole::User, exp);

        let err = decode_jwt(&token, "a-different-secret".to_string()).unwrap_err();

        assert_eq!(err, "Invalid signature");
    }

    #[test]
    fn rejects_a_malformed_token() {
        let err = decode_jwt("this.is.not-a-jwt", "test-secret".to_string()).unwrap_err();

        // Not one of the specially-handled kinds, so the raw error string is returned.
        assert!(!err.is_empty());
        assert_ne!(err, "Token is expired");
        assert_ne!(err, "Invalid signature");
    }
}
