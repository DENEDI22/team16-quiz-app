//! Bearer-token extractors shared by all services (docs/api-contracts.md §2).
//!
//! A service opts in by holding the JWT secret in its router state and
//! implementing `FromRef`:
//!
//! ```ignore
//! impl FromRef<AppState> for JwtSecret {
//!     fn from_ref(state: &AppState) -> Self {
//!         JwtSecret(state.jwt_secret.clone())
//!     }
//! }
//! ```

use axum::{
    extract::{FromRef, FromRequestParts},
    http::{Response, StatusCode, header, request::Parts},
};

use crate::jwt::{Claims, UserRole, decode_jwt};
use crate::respond;

/// The shared HMAC secret, provided via the service's router state.
#[derive(Clone)]
pub struct JwtSecret(pub String);

/// Rejects with 401 unless the request carries a valid `Authorization: Bearer` token.
pub struct Auth(pub Claims);

impl<S> FromRequestParts<S> for Auth
where
    JwtSecret: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = Response<String>;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let JwtSecret(secret) = JwtSecret::from_ref(state);

        let token = bearer_token(parts)
            .ok_or_else(|| respond::error(StatusCode::UNAUTHORIZED, "No token provided"))?;

        decode_jwt(token, &secret)
            .map(Auth)
            .map_err(|e| respond::error(StatusCode::UNAUTHORIZED, &e))
    }
}

/// Like [`Auth`], but additionally rejects with 403 unless the token's role is `Admin`.
pub struct AdminAuth(pub Claims);

impl<S> FromRequestParts<S> for AdminAuth
where
    JwtSecret: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = Response<String>;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let Auth(claims) = Auth::from_request_parts(parts, state).await?;

        if claims.role != UserRole::Admin {
            return Err(respond::error(StatusCode::FORBIDDEN, "Admin role required"));
        }
        Ok(AdminAuth(claims))
    }
}

pub fn bearer_token(parts: &Parts) -> Option<&str> {
    parts
        .headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
}
