use sqlx::PgPool;

use crate::jwt::User;

fn migrate() {}

fn get_user_info() {}

pub async fn find_user_by_email(pool: &PgPool, email: &str) -> Result<User, sqlx::Error> {
    sqlx::query_as(
        "SELECT id, username, password_hash, is_admin
         FROM users
         WHERE email = $1 AND is_deleted = false",
    )
    .bind(email)
    .fetch_one(pool)
    .await
}
