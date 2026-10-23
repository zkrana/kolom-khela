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
        None => err(StatusCode::UNAUTHORIZED, "Please log in"),
    }
}
async fn need_admin(st: &AppState, h: &HeaderMap) -> Result<User, ApiErr> {
    let u = need_user(st, h).await?;
    if u.role != "admin" {
        return err(StatusCode::FORBIDDEN, "Admins only");
    }
    Ok(u)
}

async fn session_for(st: &AppState, user_id: i64) -> Result<String, ApiErr> {
    let token = auth::new_token();
    sqlx::query("INSERT INTO sessions (token_hash, user_id, created_at) VALUES (?, ?, ?)")
        .bind(auth::hash_token(&token))
        .bind(user_id)
        .bind(db::now())
        .execute(&st.pool)
        .await
        .map_err(|_| ApiErr(StatusCode::INTERNAL_SERVER_ERROR, "Could not start a session".into()))?;
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
    if !email.contains('@') || email.len() > 120 || email.contains(' ') {
        return err(StatusCode::BAD_REQUEST, "Enter a valid email");
    }
    if name.chars().count() < 2 || name.chars().count() > 24 {
        return err(StatusCode::BAD_REQUEST, "Name must be 2 to 24 characters");
    }
    if b.password.len() < 6 || b.password.len() > 100 {
        return err(StatusCode::BAD_REQUEST, "Password must be at least 6 characters");
    }
    let pw = b.password.clone();
    let hash = tokio::task::spawn_blocking(move || auth::hash_password(&pw)).await.unwrap_or_default();
    let res = sqlx::query("INSERT INTO users (email, name, pass_hash, created_at) VALUES (?, ?, ?, ?)")
        .bind(&email)
        .bind(&name)
        .bind(&hash)
        .bind(db::now())
        .execute(&st.pool)
        .await;
    let id = match res {
        Ok(r) => r.last_insert_rowid(),
        Err(_) => return err(StatusCode::CONFLICT, "That email is already registered"),
    };
    let token = session_for(&st, id).await?;
    Ok(Json(json!({ "token": token, "user": { "id": id, "name": name, "email": email, "role": "player", "coins": 0, "pen": "econo" } })))
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
                return err(StatusCode::TOO_MANY_REQUESTS, "Too many tries. Wait a few minutes.");
            }
        }
    }
    let user: Option<User> = sqlx::query_as(&format!("{} WHERE u.email = ?", auth::USER_SELECT)).bind(&email).fetch_optional(&st.pool).await.ok().flatten();
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
        return err(StatusCode::UNAUTHORIZED, "Wrong email or password");
    };
    if user.blocked != 0 {
        return err(StatusCode::FORBIDDEN, "This account is blocked");
    }
    st.login_fails.lock().unwrap().remove(&email);
    let token = session_for(&st, user.id).await?;
    Ok(Json(json!({ "token": token, "user": { "id": user.id, "name": user.name, "email": user.email, "role": user.role, "coins": user.coins, "pen": user.pen, "avatarAt": user.avatar_at } })))
}

async fn logout(State(st): St, headers: HeaderMap) -> Res<Value> {
    if let Some(t) = headers.get("authorization").and_then(|v| v.to_str().ok()).and_then(|v| v.strip_prefix("Bearer ")) {
        let _ = sqlx::query("DELETE FROM sessions WHERE token_hash = ?").bind(auth::hash_token(t)).execute(&st.pool).await;
    }
    Ok(Json(json!({ "ok": true })))
}

async fn me(State(st): St, headers: HeaderMap) -> Res<Value> {
    let u = need_user(&st, &headers).await?;
    let row: (i64, i64, i64, i64) = sqlx::query_as(
        "SELECT COUNT(*), COALESCE(SUM(CASE WHEN rank = 1 THEN 1 ELSE 0 END), 0), COALESCE(SUM(looser), 0), COALESCE(SUM(coins), 0) FROM game_players WHERE user_id = ?",
    )
    .bind(u.id)
    .fetch_one(&st.pool)
    .await
    .unwrap_or((0, 0, 0, 0));
    Ok(Json(json!({
        "user": { "id": u.id, "name": u.name, "email": u.email, "role": u.role, "coins": u.coins, "pen": u.pen, "avatarAt": u.avatar_at, "joined": u.created_at },
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
        return err(StatusCode::SERVICE_UNAVAILABLE, "The server is full right now. Try again soon.");
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
        None => err(StatusCode::NOT_FOUND, "Room not found. It may have closed."),
    }
}

async fn leaderboard(State(st): St) -> Res<Value> {
    let rows: Vec<(i64, String, i64, i64, Option<i64>)> = sqlx::query_as(
        "SELECT p.user_id, MAX(p.name), SUM(CASE WHEN p.rank = 1 THEN 1 ELSE 0 END) AS wins, COUNT(*) AS played,
                (SELECT updated_at FROM avatars a WHERE a.user_id = p.user_id)
         FROM game_players p GROUP BY p.user_id ORDER BY wins DESC, played ASC LIMIT 10",
    )
    .fetch_all(&st.pool)
    .await
    .unwrap_or_default();
    Ok(Json(json!(rows
        .into_iter()
        .map(|(id, n, w, p, av)| json!({ "id": id, "name": n, "wins": w, "played": p, "avatar": av }))
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
            return err(StatusCode::BAD_REQUEST, "Name must be 2 to 24 characters");
        }
        let _ = sqlx::query("UPDATE users SET name = ? WHERE id = ?").bind(&n).bind(u.id).execute(&st.pool).await;
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
        return err(StatusCode::BAD_REQUEST, "New password must be at least 6 characters");
    }
    let (cur, hash) = (b.current.clone(), u.pass_hash.clone());
    if !tokio::task::spawn_blocking(move || auth::verify_password(&cur, &hash)).await.unwrap_or(false) {
        return err(StatusCode::UNAUTHORIZED, "Current password is wrong");
    }
    let new = b.new.clone();
    let hash = tokio::task::spawn_blocking(move || auth::hash_password(&new)).await.unwrap_or_default();
    let _ = sqlx::query("UPDATE users SET pass_hash = ? WHERE id = ?").bind(hash).bind(u.id).execute(&st.pool).await;
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
        return err(StatusCode::PAYLOAD_TOO_LARGE, "Picture is too big (300 KB max)");
    }
    let Some(mime) = image_mime(&body) else {
        return err(StatusCode::BAD_REQUEST, "Use a JPG, PNG or WebP picture");
    };
    let at = db::now();
    let _ = sqlx::query("INSERT INTO avatars (user_id, mime, data, updated_at) VALUES (?, ?, ?, ?) ON CONFLICT(user_id) DO UPDATE SET mime = excluded.mime, data = excluded.data, updated_at = excluded.updated_at")
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
    let _ = sqlx::query("DELETE FROM avatars WHERE user_id = ?").bind(u.id).execute(&st.pool).await;
    Ok(Json(json!({ "ok": true })))
}

async fn get_avatar(State(st): St, Path(id): Path<i64>) -> Response {
    let row: Option<(String, Vec<u8>)> = sqlx::query_as("SELECT mime, data FROM avatars WHERE user_id = ?").bind(id).fetch_optional(&st.pool).await.ok().flatten();
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
    let _ = sqlx::query("DELETE FROM avatars WHERE user_id = ?").bind(id).execute(&st.pool).await;
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
    let Some(p) = pens::find(&b.id) else { return err(StatusCode::NOT_FOUND, "No such pen") };
    if owned(&u).contains(&p.id) {
        return err(StatusCode::CONFLICT, "You already own this pen");
    }
    // One statement, so two quick taps cannot spend the same coins twice.
    let new_pens = if u.pens.is_empty() { p.id.to_string() } else { format!("{},{}", u.pens, p.id) };
    let done = sqlx::query("UPDATE users SET coins = coins - ?1, pens = ?2 WHERE id = ?3 AND coins >= ?1 AND pens = ?4")
        .bind(p.price)
        .bind(&new_pens)
        .bind(u.id)
        .bind(&u.pens)
        .execute(&st.pool)
        .await
        .map(|r| r.rows_affected())
        .unwrap_or(0);
    if done == 0 {
        return err(StatusCode::PAYMENT_REQUIRED, "Not enough coins yet. Win games to earn more.");
    }
    Ok(Json(json!({ "ok": true, "coins": u.coins - p.price })))
}

async fn shop_select(State(st): St, headers: HeaderMap, Json(b): Json<PenBody>) -> Res<Value> {
    let u = need_user(&st, &headers).await?;
    let Some(p) = pens::find(&b.id) else { return err(StatusCode::NOT_FOUND, "No such pen") };
    if !owned(&u).contains(&p.id) {
        return err(StatusCode::FORBIDDEN, "Buy this pen first");
    }
    let _ = sqlx::query("UPDATE users SET pen = ? WHERE id = ?").bind(p.id).bind(u.id).execute(&st.pool).await;
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
    let rows: Vec<User> = sqlx::query_as(&format!("{} ORDER BY u.id DESC LIMIT 500", auth::USER_SELECT)).fetch_all(&st.pool).await.unwrap_or_default();
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
        return err(StatusCode::BAD_REQUEST, "You cannot change your own account here");
    }
    if let Some(b) = p.blocked {
        let _ = sqlx::query("UPDATE users SET blocked = ? WHERE id = ?").bind(b as i64).bind(id).execute(&st.pool).await;
        if b {
            let _ = sqlx::query("DELETE FROM sessions WHERE user_id = ?").bind(id).execute(&st.pool).await;
        }
    }
    if let Some(c) = p.add_coins {
        let c = c.clamp(-100_000, 100_000);
        let _ = sqlx::query("UPDATE users SET coins = MAX(0, coins + ?) WHERE id = ?").bind(c).bind(id).execute(&st.pool).await;
    }
    if let Some(r) = p.role {
        if r == "admin" || r == "player" {
            let _ = sqlx::query("UPDATE users SET role = ? WHERE id = ?").bind(r).bind(id).execute(&st.pool).await;
        }
    }
    Ok(Json(json!({ "ok": true })))
}

async fn admin_user_delete(State(st): St, headers: HeaderMap, Path(id): Path<i64>) -> Res<Value> {
    let me = need_admin(&st, &headers).await?;
    if id == me.id {
        return err(StatusCode::BAD_REQUEST, "You cannot delete yourself");
    }
    let _ = sqlx::query("DELETE FROM users WHERE id = ?").bind(id).execute(&st.pool).await;
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
    let rows: Vec<(i64, String, i64, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT g.id, g.code, g.finished_at,
            (SELECT name FROM game_players p WHERE p.game_id = g.id AND p.rank = 1 LIMIT 1),
            (SELECT name FROM game_players p WHERE p.game_id = g.id AND p.looser = 1 LIMIT 1)
         FROM games g ORDER BY g.id DESC LIMIT 100",
    )
    .fetch_all(&st.pool)
    .await
    .unwrap_or_default();
    Ok(Json(json!(rows
        .into_iter()
        .map(|(id, code, at, w, l)| json!({ "id": id, "code": code, "finishedAt": at, "winner": w, "looser": l }))
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
}

async fn play(socket: WebSocket, user: User, handle: room::RoomHandle) {
    let (mut sink, mut stream) = socket.split();
    let (tx, mut rx) = unbounded_channel::<String>();
    if handle.tx.send(Cmd::Join { user_id: user.id, name: user.name.clone(), pen: user.pen.clone(), avatar: user.avatar_at, tx }).is_err() {
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
