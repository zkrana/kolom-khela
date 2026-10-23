use crate::auth::{self, User};
use crate::db;
use crate::pens;
use crate::room::{self, Cmd};
use crate::AppState;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, patch, post};
use axum::{Json, Router};
use futures_util::{SinkExt, StreamExt};
use rand::Rng;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc::unbounded_channel;

type St = State<Arc<AppState>>;

pub struct ApiErr(StatusCode, String);
impl IntoResponse for ApiErr {
    fn into_response(self) -> Response {
        (self.0, Json(json!({ "error": self.1 }))).into_response()
    }
}
fn err<T>(code: StatusCode, msg: &str) -> Result<T, ApiErr> {
    Err(ApiErr(code, msg.to_string()))
}
type Res<T> = Result<Json<T>, ApiErr>;

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/auth/register", post(register))
        .route("/api/auth/login", post(login))
        .route("/api/auth/guest", post(guest_join))
        .route("/api/me/upgrade", post(upgrade_guest))
        .route("/api/auth/logout", post(logout))
        .route("/api/me", get(me).patch(update_me))
        .route("/api/me/password", post(change_password))
        .route("/api/me/avatar", post(set_avatar).delete(delete_my_avatar).layer(DefaultBodyLimit::max(400_000)))
        .route("/api/avatar/:id", get(get_avatar))
        .route("/api/rooms", post(create_room))
        .route("/api/rooms/:code", get(room_exists))
        .route("/api/leaderboard", get(leaderboard))
        .route("/api/shop", get(shop))
        .route("/api/shop/buy", post(shop_buy))
        .route("/api/shop/select", post(shop_select))
        .route("/api/admin/stats", get(admin_stats))
        .route("/api/admin/users", get(admin_users))
        .route("/api/admin/users/:id", patch(admin_user_update).delete(admin_user_delete))
        .route("/api/admin/users/:id/avatar", delete(admin_avatar_delete))
        .route("/api/admin/rooms", get(admin_rooms))
        .route("/api/admin/rooms/:code", delete(admin_room_close))
        .route("/api/admin/games", get(admin_games))
        .route("/ws/:code", get(ws_upgrade))
        .with_state(state)
}

async fn need_user(st: &AppState, h: &HeaderMap) -> Result<User, ApiErr> {
    match auth::user_from_headers(st, h).await {
        Some(u) => Ok(u),
        None => err(StatusCode::UNAUTHORIZED, "আগে লগইন করুন"),
    }
}
async fn need_admin(st: &AppState, h: &HeaderMap) -> Result<User, ApiErr> {
    let u = need_user(st, h).await?;
    if u.role != "admin" {
        return err(StatusCode::FORBIDDEN, "শুধু অ্যাডমিনদের জন্য");
    }
    Ok(u)
}

async fn session_for(st: &AppState, user_id: i64) -> Result<String, ApiErr> {
    let token = auth::new_token();
    sqlx::query(db::q("INSERT INTO sessions (token_hash, user_id, created_at) VALUES (?, ?, ?)"))
        .bind(auth::hash_token(&token))
        .bind(user_id)
        .bind(db::now())
        .execute(&st.pool)
        .await
        .map_err(|_| ApiErr(StatusCode::INTERNAL_SERVER_ERROR, "লগইন শুরু করা যায়নি".into()))?;
    Ok(token)
}

#[derive(Deserialize)]
struct RegisterBody {
    email: String,
    name: String,
    password: String,
}

async fn register(State(st): St, Json(b): Json<RegisterBody>) -> Res<Value> {
    let email = b.email.trim().to_lowercase();
    let name = b.name.trim().to_string();
    if !valid_email(&email) {
        return err(StatusCode::BAD_REQUEST, "সঠিক ইমেইল দিন");
    }
    if name.chars().count() < 2 || name.chars().count() > 24 {
        return err(StatusCode::BAD_REQUEST, "নাম ২ থেকে ২৪ অক্ষরের হতে হবে");
    }
    if b.password.len() < 6 || b.password.len() > 100 {
        return err(StatusCode::BAD_REQUEST, "পাসওয়ার্ড কমপক্ষে ৬ অক্ষরের হতে হবে");
    }
    let pw = b.password.clone();
    let hash = tokio::task::spawn_blocking(move || auth::hash_password(&pw)).await.unwrap_or_default();
    let res = sqlx::query_scalar::<_, i64>(db::q("INSERT INTO users (email, name, pass_hash, contact, created_at) VALUES (?, ?, ?, '', ?) RETURNING id"))
        .bind(&email)
        .bind(&name)
        .bind(&hash)
        .bind(db::now())
        .fetch_one(&st.pool)
        .await;
    let id = match res {
        Ok(id) => id,
        Err(_) => return err(StatusCode::CONFLICT, "এই ইমেইল দিয়ে আগেই অ্যাকাউন্ট খোলা আছে"),
    };
    let token = session_for(&st, id).await?;
    Ok(Json(json!({ "token": token, "user": { "id": id, "name": name, "email": email, "role": "player", "coins": 0, "pen": "econo" } })))
}

fn valid_email(e: &str) -> bool {
    let at = e.find('@');
    e.len() <= 120 && !e.contains(' ') && !e.ends_with("@guest.invalid") && matches!(at, Some(i) if i > 0 && e[i + 1..].contains('.') && !e.ends_with('.'))
}

#[derive(Deserialize)]
struct GuestBody {
    email: String,
    name: String,
}

fn client_ip(h: &HeaderMap) -> String {
    h.get("x-forwarded-for").and_then(|v| v.to_str().ok()).and_then(|v| v.split(',').next()).map(|s| s.trim().to_string()).unwrap_or_else(|| "local".into())
}

/// Someone opened an invite link: name and email is all it takes to sit down at the board. The account has no password
/// (it cannot be logged into from elsewhere) until they choose to save it from the profile page.
async fn guest_join(State(st): St, headers: HeaderMap, Json(b): Json<GuestBody>) -> Res<Value> {
    let email = b.email.trim().to_lowercase();
    let name = b.name.trim().to_string();
    if !valid_email(&email) {
        return err(StatusCode::BAD_REQUEST, "সঠিক ইমেইল দিন");
    }
    if name.chars().count() < 2 || name.chars().count() > 24 {
        return err(StatusCode::BAD_REQUEST, "নাম ২ থেকে ২৪ অক্ষরের হতে হবে");
    }
    {
        // At most 15 new guests per address every 10 minutes.
        let mut log = st.guest_log.lock().unwrap();
        let hits = log.entry(client_ip(&headers)).or_default();
        hits.retain(|t| t.elapsed() < Duration::from_secs(600));
        if hits.len() >= 15 {
            return err(StatusCode::TOO_MANY_REQUESTS, "এখান থেকে অনেক বেশি নতুন খেলোয়াড় এসেছে। কয়েক মিনিট পর আবার চেষ্টা করুন।");
        }
        hits.push(Instant::now());
    }
    let taken: Option<i64> = sqlx::query_scalar(db::q("SELECT id FROM users WHERE email = ? AND guest = 0")).bind(&email).fetch_optional(&st.pool).await.ok().flatten();
    if taken.is_some() {
        return err(StatusCode::CONFLICT, "এই ইমেইলে আগেই অ্যাকাউন্ট আছে। লগইন করুন।");
    }
    let fake = format!("guest-{}@guest.invalid", &auth::new_token()[..12]);
    let res = sqlx::query_scalar::<_, i64>(db::q("INSERT INTO users (email, name, pass_hash, guest, contact, created_at) VALUES (?, ?, '', 1, ?, ?) RETURNING id"))
        .bind(&fake)
        .bind(&name)
        .bind(&email)
        .bind(db::now())
        .fetch_one(&st.pool)
        .await;
    let Ok(id) = res else { return err(StatusCode::INTERNAL_SERVER_ERROR, "খেলায় ঢোকা যায়নি। আবার চেষ্টা করুন।") };
    let token = session_for(&st, id).await?;
    Ok(Json(json!({ "token": token, "user": { "id": id, "name": name, "role": "player", "coins": 0, "pen": "econo", "guest": true } })))
}

#[derive(Deserialize)]
struct UpgradeBody {
    email: String,
    password: String,
}

/// A guest keeps their coins and pens by choosing a real email and a password.
async fn upgrade_guest(State(st): St, headers: HeaderMap, Json(b): Json<UpgradeBody>) -> Res<Value> {
    let u = need_user(&st, &headers).await?;
    if u.guest != 1 {
        return err(StatusCode::BAD_REQUEST, "এই অ্যাকাউন্ট আগেই সেভ করা আছে");
    }
    let email = b.email.trim().to_lowercase();
    if !valid_email(&email) {
        return err(StatusCode::BAD_REQUEST, "সঠিক ইমেইল দিন");
    }
    if b.password.len() < 6 || b.password.len() > 100 {
        return err(StatusCode::BAD_REQUEST, "পাসওয়ার্ড কমপক্ষে ৬ অক্ষরের হতে হবে");
    }
    let pw = b.password.clone();
    let hash = tokio::task::spawn_blocking(move || auth::hash_password(&pw)).await.unwrap_or_default();
    let done = sqlx::query(db::q("UPDATE users SET email = ?, pass_hash = ?, guest = 0, contact = '' WHERE id = ? AND guest = 1"))
        .bind(&email)
        .bind(&hash)
        .bind(u.id)
        .execute(&st.pool)
        .await;
    match done {
        Ok(_) => Ok(Json(json!({ "ok": true }))),
        Err(_) => err(StatusCode::CONFLICT, "এই ইমেইল আগেই নেওয়া হয়ে গেছে। ওটা দিয়ে লগইন করুন।"),
    }
}

#[derive(Deserialize)]
struct LoginBody {
    email: String,
    password: String,
}

async fn login(State(st): St, Json(b): Json<LoginBody>) -> Res<Value> {
    let email = b.email.trim().to_lowercase();
    {
        // 8 wrong passwords for one email locks it for 5 minutes.
        let fails = st.login_fails.lock().unwrap();
        if let Some((n, at)) = fails.get(&email) {
            if *n >= 8 && at.elapsed() < Duration::from_secs(300) {
                return err(StatusCode::TOO_MANY_REQUESTS, "অনেকবার ভুল হয়েছে। কয়েক মিনিট অপেক্ষা করুন।");
            }
        }
    }
    let user: Option<User> = sqlx::query_as(db::q(&format!("{} WHERE u.email = ?", auth::USER_SELECT))).bind(&email).fetch_optional(&st.pool).await.ok().flatten();
    let hash = user.as_ref().map(|u| u.pass_hash.clone()).unwrap_or_default();
    let pw = b.password.clone();
    let ok = tokio::task::spawn_blocking(move || auth::verify_password(&pw, &hash)).await.unwrap_or(false);
    let Some(user) = user.filter(|_| ok) else {
        let mut fails = st.login_fails.lock().unwrap();
        let e = fails.entry(email).or_insert((0, Instant::now()));
        if e.1.elapsed() > Duration::from_secs(300) {
            *e = (0, Instant::now());
        }
        e.0 += 1;
        e.1 = Instant::now();
        return err(StatusCode::UNAUTHORIZED, "ইমেইল বা পাসওয়ার্ড ভুল");
    };
    if user.blocked != 0 {
        return err(StatusCode::FORBIDDEN, "এই অ্যাকাউন্ট ব্লক করা আছে");
    }
    st.login_fails.lock().unwrap().remove(&email);
    let token = session_for(&st, user.id).await?;
    Ok(Json(json!({ "token": token, "user": { "id": user.id, "name": user.name, "email": user.email, "role": user.role, "coins": user.coins, "pen": user.pen, "avatarAt": user.avatar_at, "guest": false } })))
}

async fn logout(State(st): St, headers: HeaderMap) -> Res<Value> {
    if let Some(t) = headers.get("authorization").and_then(|v| v.to_str().ok()).and_then(|v| v.strip_prefix("Bearer ")) {
        let _ = sqlx::query(db::q("DELETE FROM sessions WHERE token_hash = ?")).bind(auth::hash_token(t)).execute(&st.pool).await;
    }
    Ok(Json(json!({ "ok": true })))
}

async fn me(State(st): St, headers: HeaderMap) -> Res<Value> {
    let u = need_user(&st, &headers).await?;
    let row: (i64, i64, i64, i64) = sqlx::query_as(
        db::q("SELECT COUNT(*), CAST(COALESCE(SUM(CASE WHEN rank = 1 THEN 1 ELSE 0 END), 0) AS BIGINT), CAST(COALESCE(SUM(looser), 0) AS BIGINT), CAST(COALESCE(SUM(coins), 0) AS BIGINT) FROM game_players WHERE user_id = ?"),
    )
    .bind(u.id)
    .fetch_one(&st.pool)
    .await
    .unwrap_or((0, 0, 0, 0));
    Ok(Json(json!({
        "user": { "id": u.id, "name": u.name, "email": u.email, "role": u.role, "coins": u.coins, "pen": u.pen, "avatarAt": u.avatar_at, "joined": u.created_at, "guest": u.guest == 1 },
        "stats": { "played": row.0, "wins": row.1, "loser": row.2, "coins": row.3 }
    })))
}

fn new_code() -> String {
    const A: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    let mut r = rand::thread_rng();
    (0..5).map(|_| A[r.gen_range(0..A.len())] as char).collect()
}

async fn create_room(State(st): St, headers: HeaderMap) -> Res<Value> {
    let u = need_user(&st, &headers).await?;
    let mut rooms = st.rooms.lock().await;
    if rooms.len() >= 200 {
        return err(StatusCode::SERVICE_UNAVAILABLE, "সার্ভারে এখন জায়গা নেই। একটু পরে চেষ্টা করুন।");
    }
    let mut code = new_code();
    while rooms.contains_key(&code) {
        code = new_code();
    }
    let handle = room::spawn_room(st.clone(), code.clone(), u.id, u.name.clone());
    rooms.insert(code.clone(), handle);
    Ok(Json(json!({ "code": code })))
}

async fn room_exists(State(st): St, Path(code): Path<String>) -> Res<Value> {
    let code = code.to_uppercase();
    let rooms = st.rooms.lock().await;
    match rooms.get(&code) {
        Some(h) => {
            let i = h.info.lock().unwrap().clone();
            Ok(Json(json!({ "code": i.code, "host": i.host, "players": i.players, "phase": i.phase })))
        }
        None => err(StatusCode::NOT_FOUND, "রুম পাওয়া যায়নি। হয়তো বন্ধ হয়ে গেছে।"),
    }
}

async fn leaderboard(State(st): St) -> Res<Value> {
    let rows: Vec<(i64, String, i64, i64, i64)> = sqlx::query_as(
        db::q("SELECT p.user_id, MAX(p.name), CAST(SUM(CASE WHEN p.rank = 1 THEN 1 ELSE 0 END) AS BIGINT) AS wins, COUNT(*) AS played,
                COALESCE((SELECT updated_at FROM avatars a WHERE a.user_id = p.user_id), 0)
         FROM game_players p GROUP BY p.user_id ORDER BY wins DESC, played ASC LIMIT 10"),
    )
    .fetch_all(&st.pool)
    .await
    .unwrap_or_default();
    Ok(Json(json!(rows
        .into_iter()
        .map(|(id, n, w, p, av)| json!({ "id": id, "name": n, "wins": w, "played": p, "avatar": if av > 0 { Some(av) } else { None } }))
        .collect::<Vec<_>>())))
}

// ---------------------------------------------------------------- profile + avatar

#[derive(Deserialize)]
struct MePatch {
    name: Option<String>,
}

async fn update_me(State(st): St, headers: HeaderMap, Json(p): Json<MePatch>) -> Res<Value> {
    let u = need_user(&st, &headers).await?;
    if let Some(n) = p.name {
        let n = n.trim().to_string();
        if n.chars().count() < 2 || n.chars().count() > 24 {
            return err(StatusCode::BAD_REQUEST, "নাম ২ থেকে ২৪ অক্ষরের হতে হবে");
        }
        let _ = sqlx::query(db::q("UPDATE users SET name = ? WHERE id = ?")).bind(&n).bind(u.id).execute(&st.pool).await;
    }
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct PwBody {
    current: String,
    new: String,
}

async fn change_password(State(st): St, headers: HeaderMap, Json(b): Json<PwBody>) -> Res<Value> {
    let u = need_user(&st, &headers).await?;
    if b.new.len() < 6 || b.new.len() > 100 {
        return err(StatusCode::BAD_REQUEST, "নতুন পাসওয়ার্ড কমপক্ষে ৬ অক্ষরের হতে হবে");
    }
    let (cur, hash) = (b.current.clone(), u.pass_hash.clone());
    if !tokio::task::spawn_blocking(move || auth::verify_password(&cur, &hash)).await.unwrap_or(false) {
        return err(StatusCode::UNAUTHORIZED, "বর্তমান পাসওয়ার্ড ভুল");
    }
    let new = b.new.clone();
    let hash = tokio::task::spawn_blocking(move || auth::hash_password(&new)).await.unwrap_or_default();
    let _ = sqlx::query(db::q("UPDATE users SET pass_hash = ? WHERE id = ?")).bind(hash).bind(u.id).execute(&st.pool).await;
    Ok(Json(json!({ "ok": true })))
}

/// Only real raster images are accepted, judged by their first bytes (never by what the browser claims).
fn image_mime(b: &[u8]) -> Option<&'static str> {
    if b.len() > 12 && b[0] == 0xFF && b[1] == 0xD8 && b[2] == 0xFF {
        Some("image/jpeg")
    } else if b.len() > 12 && b.starts_with(&[0x89, b'P', b'N', b'G']) {
        Some("image/png")
    } else if b.len() > 12 && &b[0..4] == b"RIFF" && &b[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}

async fn set_avatar(State(st): St, headers: HeaderMap, body: Bytes) -> Res<Value> {
    let u = need_user(&st, &headers).await?;
    if body.len() > 300_000 {
        return err(StatusCode::PAYLOAD_TOO_LARGE, "ছবি অনেক বড় (সর্বোচ্চ ৩০০ KB)");
    }
    let Some(mime) = image_mime(&body) else {
        return err(StatusCode::BAD_REQUEST, "JPG, PNG বা WebP ছবি দিন");
    };
    let at = db::now();
    let _ = sqlx::query(db::q("INSERT INTO avatars (user_id, mime, data, updated_at) VALUES (?, ?, ?, ?) ON CONFLICT(user_id) DO UPDATE SET mime = excluded.mime, data = excluded.data, updated_at = excluded.updated_at"))
        .bind(u.id)
        .bind(mime)
        .bind(body.to_vec())
        .bind(at)
        .execute(&st.pool)
        .await;
    Ok(Json(json!({ "ok": true, "avatarAt": at })))
}

async fn delete_my_avatar(State(st): St, headers: HeaderMap) -> Res<Value> {
    let u = need_user(&st, &headers).await?;
    let _ = sqlx::query(db::q("DELETE FROM avatars WHERE user_id = ?")).bind(u.id).execute(&st.pool).await;
    Ok(Json(json!({ "ok": true })))
}

async fn get_avatar(State(st): St, Path(id): Path<i64>) -> Response {
    let row: Option<(String, Vec<u8>)> = sqlx::query_as(db::q("SELECT mime, data FROM avatars WHERE user_id = ?")).bind(id).fetch_optional(&st.pool).await.ok().flatten();
    match row {
        Some((mime, data)) => (
            [
                (header::CONTENT_TYPE, mime),
                (header::CACHE_CONTROL, "public, max-age=300".to_string()),
                (header::X_CONTENT_TYPE_OPTIONS, "nosniff".to_string()),
            ],
            data,
        )
            .into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn admin_avatar_delete(State(st): St, headers: HeaderMap, Path(id): Path<i64>) -> Res<Value> {
    need_admin(&st, &headers).await?;
    let _ = sqlx::query(db::q("DELETE FROM avatars WHERE user_id = ?")).bind(id).execute(&st.pool).await;
    Ok(Json(json!({ "ok": true })))
}

// ---------------------------------------------------------------- pen shop

fn owned(u: &User) -> Vec<&str> {
    u.pens.split(',').filter(|s| !s.is_empty()).collect()
}

async fn shop(State(st): St, headers: HeaderMap) -> Res<Value> {
    let u = need_user(&st, &headers).await?;
    let have = owned(&u);
    let list: Vec<Value> = pens::PENS
        .iter()
        .map(|p| json!({ "id": p.id, "name": p.name, "desc": p.desc, "price": p.price, "hl": p.hl, "r": p.r, "m": p.m, "owned": have.contains(&p.id) }))
        .collect();
    Ok(Json(json!({ "coins": u.coins, "selected": u.pen, "reward": pens::WIN_REWARD, "pens": list })))
}

#[derive(Deserialize)]
struct PenBody {
    id: String,
}

async fn shop_buy(State(st): St, headers: HeaderMap, Json(b): Json<PenBody>) -> Res<Value> {
    let u = need_user(&st, &headers).await?;
    let Some(p) = pens::find(&b.id) else { return err(StatusCode::NOT_FOUND, "এই নামে কোনো কলম নেই") };
    if owned(&u).contains(&p.id) {
        return err(StatusCode::CONFLICT, "এই কলম আপনার আগেই আছে");
    }
    // One statement, so two quick taps cannot spend the same coins twice.
    let new_pens = if u.pens.is_empty() { p.id.to_string() } else { format!("{},{}", u.pens, p.id) };
    let done = sqlx::query(db::q("UPDATE users SET coins = coins - ?1, pens = ?2 WHERE id = ?3 AND coins >= ?1 AND pens = ?4"))
        .bind(p.price)
        .bind(&new_pens)
        .bind(u.id)
        .bind(&u.pens)
        .execute(&st.pool)
        .await
        .map(|r| r.rows_affected())
        .unwrap_or(0);
    if done == 0 {
        return err(StatusCode::PAYMENT_REQUIRED, "কয়েন যথেষ্ট নেই। খেলা জিতে আরও কয়েন জমান।");
    }
    Ok(Json(json!({ "ok": true, "coins": u.coins - p.price })))
}

async fn shop_select(State(st): St, headers: HeaderMap, Json(b): Json<PenBody>) -> Res<Value> {
    let u = need_user(&st, &headers).await?;
    let Some(p) = pens::find(&b.id) else { return err(StatusCode::NOT_FOUND, "এই নামে কোনো কলম নেই") };
    if !owned(&u).contains(&p.id) {
        return err(StatusCode::FORBIDDEN, "আগে এই কলমটা কিনুন");
    }
    let _ = sqlx::query(db::q("UPDATE users SET pen = ? WHERE id = ?")).bind(p.id).bind(u.id).execute(&st.pool).await;
    Ok(Json(json!({ "ok": true })))
}

// ---------------------------------------------------------------- admin

async fn admin_stats(State(st): St, headers: HeaderMap) -> Res<Value> {
    need_admin(&st, &headers).await?;
    let c = |q: &'static str| {
        let pool = st.pool.clone();
        async move { sqlx::query_scalar::<_, i64>(q).fetch_one(&pool).await.unwrap_or(0) }
    };
    let rooms = st.rooms.lock().await.len();
    Ok(Json(json!({
        "users": c("SELECT COUNT(*) FROM users").await,
        "blocked": c("SELECT COUNT(*) FROM users WHERE blocked = 1").await,
        "games": c("SELECT COUNT(*) FROM games").await,
        "liveRooms": rooms,
    })))
}

async fn admin_users(State(st): St, headers: HeaderMap) -> Res<Value> {
    need_admin(&st, &headers).await?;
    let rows: Vec<User> = sqlx::query_as(db::q(&format!("{} ORDER BY u.id DESC LIMIT 500", auth::USER_SELECT))).fetch_all(&st.pool).await.unwrap_or_default();
    Ok(Json(json!(rows)))
}

#[derive(Deserialize)]
struct UserPatch {
    blocked: Option<bool>,
    role: Option<String>,
    add_coins: Option<i64>,
}

async fn admin_user_update(State(st): St, headers: HeaderMap, Path(id): Path<i64>, Json(p): Json<UserPatch>) -> Res<Value> {
    let me = need_admin(&st, &headers).await?;
    if id == me.id {
        return err(StatusCode::BAD_REQUEST, "এখান থেকে নিজের অ্যাকাউন্ট বদলানো যায় না");
    }
    if let Some(b) = p.blocked {
        let _ = sqlx::query(db::q("UPDATE users SET blocked = ? WHERE id = ?")).bind(b as i64).bind(id).execute(&st.pool).await;
        if b {
            let _ = sqlx::query(db::q("DELETE FROM sessions WHERE user_id = ?")).bind(id).execute(&st.pool).await;
        }
    }
    if let Some(c) = p.add_coins {
        let c = c.clamp(-100_000, 100_000);
        let _ = sqlx::query(db::q("UPDATE users SET coins = CASE WHEN coins + ?1 < 0 THEN 0 ELSE coins + ?1 END WHERE id = ?2")).bind(c).bind(id).execute(&st.pool).await;
    }
    if let Some(r) = p.role {
        if r == "admin" || r == "player" {
            let _ = sqlx::query(db::q("UPDATE users SET role = ? WHERE id = ?")).bind(r).bind(id).execute(&st.pool).await;
        }
    }
    Ok(Json(json!({ "ok": true })))
}

async fn admin_user_delete(State(st): St, headers: HeaderMap, Path(id): Path<i64>) -> Res<Value> {
    let me = need_admin(&st, &headers).await?;
    if id == me.id {
        return err(StatusCode::BAD_REQUEST, "নিজেকে ডিলিট করা যায় না");
    }
    let _ = sqlx::query(db::q("DELETE FROM users WHERE id = ?")).bind(id).execute(&st.pool).await;
    Ok(Json(json!({ "ok": true })))
}

async fn admin_rooms(State(st): St, headers: HeaderMap) -> Res<Value> {
    need_admin(&st, &headers).await?;
    let rooms = st.rooms.lock().await;
    let list: Vec<Value> = rooms
        .values()
        .map(|h| {
            let i = h.info.lock().unwrap().clone();
            json!({ "code": i.code, "host": i.host, "players": i.players, "phase": i.phase, "createdAt": i.created_at })
        })
        .collect();
    Ok(Json(json!(list)))
}

async fn admin_room_close(State(st): St, headers: HeaderMap, Path(code): Path<String>) -> Res<Value> {
    need_admin(&st, &headers).await?;
    if let Some(h) = st.rooms.lock().await.get(&code.to_uppercase()) {
        let _ = h.tx.send(Cmd::Close);
    }
    Ok(Json(json!({ "ok": true })))
}

async fn admin_games(State(st): St, headers: HeaderMap) -> Res<Value> {
    need_admin(&st, &headers).await?;
    let rows: Vec<(i64, String, i64, String, String)> = sqlx::query_as(
        db::q("SELECT g.id, g.code, g.finished_at,
            COALESCE((SELECT name FROM game_players p WHERE p.game_id = g.id AND p.rank = 1 LIMIT 1), ''),
            COALESCE((SELECT name FROM game_players p WHERE p.game_id = g.id AND p.looser = 1 LIMIT 1), '')
         FROM games g ORDER BY g.id DESC LIMIT 100"),
    )
    .fetch_all(&st.pool)
    .await
    .unwrap_or_default();
    Ok(Json(json!(rows
        .into_iter()
        .map(|(id, code, at, w, l)| json!({ "id": id, "code": code, "finishedAt": at, "winner": if w.is_empty() { None } else { Some(w) }, "looser": if l.is_empty() { None } else { Some(l) } }))
        .collect::<Vec<_>>())))
}

// ---------------------------------------------------------------- live play

async fn ws_upgrade(ws: WebSocketUpgrade, State(st): St, Path(code): Path<String>, Query(q): Query<HashMap<String, String>>) -> Response {
    let Some(user) = auth::user_by_token(&st, q.get("token").map(String::as_str).unwrap_or("")).await else {
        return (StatusCode::UNAUTHORIZED, "login required").into_response();
    };
    let code = code.to_uppercase();
    let handle = st.rooms.lock().await.get(&code).cloned();
    let Some(handle) = handle else {
        return (StatusCode::NOT_FOUND, "no such room").into_response();
    };
    ws.on_upgrade(move |socket| play(socket, user, handle))
}

#[derive(Deserialize)]
struct ClientMsg {
    t: String,
    dx: Option<f32>,
    dy: Option<f32>,
    power: Option<f32>,
    lx: Option<f32>,
    ly: Option<f32>,
    level: Option<String>,
}

async fn play(socket: WebSocket, user: User, handle: room::RoomHandle) {
    let (mut sink, mut stream) = socket.split();
    let (tx, mut rx) = unbounded_channel::<String>();
    if handle.tx.send(Cmd::Join { user_id: user.id, name: user.name.clone(), pen: user.pen.clone(), avatar: if user.avatar_at > 0 { Some(user.avatar_at) } else { None }, tx }).is_err() {
        let _ = sink.send(Message::Text(json!({ "t": "closed" }).to_string())).await;
        return;
    }
    let writer = tokio::spawn(async move {
        while let Some(m) = rx.recv().await {
            if sink.send(Message::Text(m)).await.is_err() {
                break;
            }
        }
    });
    while let Some(Ok(msg)) = stream.next().await {
        let Message::Text(text) = msg else {
            if matches!(msg, Message::Close(_)) {
                break;
            }
            continue;
        };
        if text.len() > 512 {
            continue;
        }
        let Ok(m) = serde_json::from_str::<ClientMsg>(&text) else { continue };
        let cmd = match m.t.as_str() {
            "start" => Cmd::Start { user_id: user.id },
            "difficulty" => Cmd::SetDifficulty { user_id: user.id, level: m.level.clone().unwrap_or_default() },
            "rematch" => Cmd::Rematch { user_id: user.id },
            "shoot" => Cmd::Shoot { user_id: user.id, dx: m.dx.unwrap_or(0.0), dy: m.dy.unwrap_or(0.0), power: m.power.unwrap_or(0.0), lx: m.lx.unwrap_or(0.0), ly: m.ly.unwrap_or(0.0) },
            _ => continue,
        };
        if handle.tx.send(cmd).is_err() {
            break;
        }
    }
    let _ = handle.tx.send(Cmd::Leave { user_id: user.id });
    writer.abort();
}
