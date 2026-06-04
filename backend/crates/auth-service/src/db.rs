use sqlx::PgPool;
use uuid::Uuid;

use crate::jwt::User;

fn migrate() {}

fn get_user_info() {}

pub async fn find_user_by_email(pool: &PgPool, email: &str) -> Result<User, sqlx::Error> {
    sqlx::query_as(
        "SELECT id, email, username, password_hash, is_admin
         FROM users
         WHERE email = $1 AND is_deleted = false",
    )
    .bind(email)
    .fetch_one(pool)
    .await
}

pub async fn create_user(
    pool: &PgPool,
    email: &str,
    password_hash: &str,
    username: &str,
) -> Result<Uuid, sqlx::Error> {
    let row: (Uuid,) = sqlx::query_as(
        "INSERT INTO users (email, password_hash, username) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(email)
    .bind(password_hash)
    .bind(username)
    .fetch_one(pool)
    .await?;
    Ok(row.0)
}
