use crate::db;
use crate::AppState;
use argon2::password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use axum::http::HeaderMap;
use rand::RngCore;
use serde::Serialize;
use sha2::{Digest, Sha256};

#[derive(sqlx::FromRow, Clone, Serialize)]
pub struct User {
    pub id: i64,
    pub email: String,
    pub name: String,
    #[serde(skip)]
    pub pass_hash: String,
    pub role: String,
    pub blocked: i64,
    pub created_at: i64,
    pub coins: i64,
    pub pen: String,
    #[serde(skip)]
    pub pens: String,
    pub guest: i64,
    pub contact: String,
    /// When the picture was last set; 0 when there is none.
    #[serde(rename = "avatarAt")]
    pub avatar_at: i64,
}

pub const USER_SELECT: &str = "SELECT u.id, u.email, u.name, u.pass_hash, u.role, u.blocked, u.created_at, u.coins, u.pen, u.pens, u.guest, COALESCE(u.contact, '') AS contact, COALESCE((SELECT a.updated_at FROM avatars a WHERE a.user_id = u.id), 0) AS avatar_at FROM users u";

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

pub fn hash_token(token: &str) -> String {
    hex(&Sha256::digest(token.as_bytes()))
}

pub fn new_token() -> String {
    let mut b = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut b);
    hex(&b)
}

pub fn hash_password(p: &str) -> String {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default().hash_password(p.as_bytes(), &salt).map(|h| h.to_string()).unwrap_or_default()
}

pub fn verify_password(p: &str, hash: &str) -> bool {
    PasswordHash::new(hash).map(|h| Argon2::default().verify_password(p.as_bytes(), &h).is_ok()).unwrap_or(false)
}

pub async fn user_by_token(st: &AppState, token: &str) -> Option<User> {
    let u: Option<User> = sqlx::query_as(db::q(&format!("{USER_SELECT} JOIN sessions s ON s.user_id = u.id WHERE s.token_hash = ?")))
    .bind(hash_token(token))
    .fetch_optional(&st.pool)
    .await
    .map_err(|e| eprintln!("session lookup failed: {e}"))
    .ok()
    .flatten();
    u.filter(|u| u.blocked == 0)
}

pub async fn user_from_headers(st: &AppState, headers: &HeaderMap) -> Option<User> {
    let raw = headers.get("authorization")?.to_str().ok()?;
    let token = raw.strip_prefix("Bearer ")?;
    user_by_token(st, token).await
}
