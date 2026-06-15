use sqlx::PgPool;
use uuid::Uuid;

use crate::jwt::User;

pub async fn migrate(pool: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query(
        "
CREATE TABLE IF NOT EXISTS public.users (
	id uuid DEFAULT gen_random_uuid() NOT NULL,
	email varchar NOT NULL,
	username varchar NOT NULL,
	password_hash varchar NOT NULL,
	is_admin bool DEFAULT false NOT NULL,
	is_deleted bool DEFAULT false NOT NULL,
	CONSTRAINT email_unique UNIQUE (email),
	CONSTRAINT users_pk PRIMARY KEY (id)
)
 ",
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_user(pool: &PgPool, id: &Uuid) -> Result<User, sqlx::Error> {
    sqlx::query_as(
        "SELECT id, email, username, password_hash, is_admin
         FROM users
         WHERE id = $1 AND is_deleted = false",
    )
    .bind(id)
    .fetch_one(pool)
    .await
}

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

/// Resolves a batch of user ids to their public usernames (used by other
/// services, e.g. scoreboard, to label leaderboard entries). Unknown or
/// soft-deleted ids are simply omitted from the result.
pub async fn get_usernames(
    pool: &PgPool,
    ids: &[Uuid],
) -> Result<Vec<(Uuid, String)>, sqlx::Error> {
    sqlx::query_as(
        "SELECT id, username
         FROM users
         WHERE id = ANY($1) AND is_deleted = false",
    )
    .bind(ids)
    .fetch_all(pool)
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
