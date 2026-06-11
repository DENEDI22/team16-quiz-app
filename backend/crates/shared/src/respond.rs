//! Standard response envelope shared by all services (docs/api-contracts.md §1.4).
//!
//! 2xx: `{ "success": true, "data": … }`
//! 4xx/5xx: `{ "success": false, "error": { "message": … } }`

use axum::http::{Response, StatusCode, header};
use serde_json::{Value, json};

fn json_response(status: StatusCode, body: Value) -> Response<String> {
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "application/json")
        .body(body.to_string())
        .unwrap_or_default()
}

pub fn success(status: StatusCode, data: Value) -> Response<String> {
    json_response(status, json!({ "success": true, "data": data }))
}

pub fn ok(data: Value) -> Response<String> {
    success(StatusCode::OK, data)
}

pub fn created(data: Value) -> Response<String> {
    success(StatusCode::CREATED, data)
}

pub fn error(status: StatusCode, message: &str) -> Response<String> {
    json_response(
        status,
        json!({ "success": false, "error": { "message": message } }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn success_wraps_data() {
        let resp = ok(json!({ "token": "abc" }));

        assert_eq!(resp.status(), StatusCode::OK);
        let body: Value = serde_json::from_str(resp.body()).unwrap();
        assert_eq!(body["success"], true);
        assert_eq!(body["data"]["token"], "abc");
    }

    #[test]
    fn created_uses_201() {
        assert_eq!(created(json!({})).status(), StatusCode::CREATED);
    }

    #[test]
    fn error_wraps_message() {
        let resp = error(StatusCode::UNAUTHORIZED, "No token provided");

        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
        let body: Value = serde_json::from_str(resp.body()).unwrap();
        assert_eq!(body["success"], false);
        assert_eq!(body["error"]["message"], "No token provided");
    }
}
