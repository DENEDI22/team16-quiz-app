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
    pub(crate) username: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn login_request_deserializes_from_json() {
        let req: LoginRequest =
            serde_json::from_str(r#"{"email":"a@b.com","password":"secret"}"#).unwrap();

        assert_eq!(req.email, "a@b.com");
        assert_eq!(req.password, "secret");
    }

    #[test]
    fn login_request_rejects_missing_fields() {
        let result: Result<LoginRequest, _> = serde_json::from_str(r#"{"email":"a@b.com"}"#);

        assert!(result.is_err(), "password is required");
    }

    #[test]
    fn register_request_deserializes_from_json() {
        let req: RegisterRequest =
            serde_json::from_str(r#"{"email":"a@b.com","password":"secret","username":"alice"}"#)
                .unwrap();

        assert_eq!(req.email, "a@b.com");
        assert_eq!(req.password, "secret");
        assert_eq!(req.username, "alice");
    }

    #[test]
    fn register_request_rejects_missing_username() {
        let result: Result<RegisterRequest, _> =
            serde_json::from_str(r#"{"email":"a@b.com","password":"secret"}"#);

        assert!(result.is_err(), "username is required");
    }
}
