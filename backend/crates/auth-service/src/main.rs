mod jwt;
use axum::{
    Json, Router,
    extract::FromRequestParts,
    http::{Response, header},
    response::IntoResponse,
    routing::{get, post},
};
use serde_json::json;
use tokio::net::TcpListener;

use crate::jwt::{Claims, User, decode_jwt};

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/health", get(health))
        .route("/register", post(|| async { "Register" }))
        .route("/login", post(login_handler))
        .route("/me", get(me_handler));

    // listen globally on port 3000
    let listener = TcpListener::bind("0.0.0.0:3000")
        .await
        .expect("Address must be free and valid");
    println!("Server started successfully at 0.0.0.0:3000");
    axum::serve(listener, app)
        .await
        .expect("Error serving application");
}

async fn login_handler(Json(user): Json<User>) -> Response<String> {
    let token = jwt::get_jwt(user);

    match token {
        Ok(token) => Response::builder()
            .status(200)
            .header(header::CONTENT_TYPE, "application/json")
            .body(
                json!({
                    "success": true,
                    "data": {
                        "token": token.to_string(),
                    }
                })
                .to_string(),
            )
            .unwrap_or_default(),

        Err(e) => Response::builder()
            .status(401)
            .header(header::CONTENT_TYPE, "application/json")
            .body(
                json!({
                    "success": false,
                    "data": {
                        "message": e
                    }
                })
                .to_string(),
            )
            .unwrap_or_default(),
    }
}

async fn me_handler(Auth(claims): Auth) -> Response<String> {
    Response::builder()
        .status(200)
        .header(header::CONTENT_TYPE, "application/json")
        .body(
            json!({
                "success": true,
                "data": claims
            })
            .to_string(),
        )
        .unwrap_or_default()
}

pub struct Auth(Claims);

#[cfg_attr(cfg, async_trait)]
impl<S> FromRequestParts<S> for Auth
where
    S: Send + Sync,
{
    type Rejection = Response<String>;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        _: &S,
    ) -> Result<Self, Self::Rejection> {
        let access_token = parts
            .headers
            .get(header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.split(" ").nth(1));

        match access_token {
            Some(token) => {
                let claims = decode_jwt(token);

                match claims {
                    Ok(claims) => Ok(Auth(claims)),

                    Err(e) => Err(Response::builder()
                        .status(401)
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(
                            json!({
                                "success": false,
                                "data": {
                                    "message": e
                                }
                            })
                            .to_string(),
                        )
                        .unwrap_or_default()),
                }
            }

            None => Err(Response::builder()
                .status(401)
                .header(header::CONTENT_TYPE, "application/json")
                .body(
                    json!({
                        "success": false,
                        "data": {
                            "message": "No token provided"
                        }
                    })
                    .to_string(),
                )
                .unwrap_or_default()),
        }
    }
}
async fn health() -> impl IntoResponse {
    "healthy"
}
