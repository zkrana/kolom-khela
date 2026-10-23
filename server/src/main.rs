mod api;
mod auth;
mod db;
mod game;
mod pens;
mod room;

use axum::extract::Request;
use axum::http::{header, HeaderValue};
use axum::middleware::{from_fn, Next};
use axum::response::Response;
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tower_http::compression::CompressionLayer;
use tower_http::cors::CorsLayer;
use tower_http::services::{ServeDir, ServeFile};

// Static files may be cached for a few minutes; API answers (who am I, coins, rooms) never may, or the page would show stale data.
async fn cache_headers(req: Request, next: Next) -> Response {
    let api = req.uri().path().starts_with("/api/");
    let mut res = next.run(req).await;
    if !res.headers().contains_key(header::CACHE_CONTROL) {
        res.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static(if api { "no-store" } else { "public, max-age=300" }));
    }
    res
}

pub struct AppState {
    pub pool: SqlitePool,
    pub rooms: tokio::sync::Mutex<HashMap<String, room::RoomHandle>>,
    pub login_fails: std::sync::Mutex<HashMap<String, (u32, Instant)>>,
}

async fn seed_admin(pool: &SqlitePool) {
    let (Ok(email), Ok(pass)) = (std::env::var("ADMIN_EMAIL"), std::env::var("ADMIN_PASSWORD")) else {
        println!("ADMIN_EMAIL / ADMIN_PASSWORD not set: no admin account was created.");
        return;
    };
    let email = email.trim().to_lowercase();
    let exists: Option<i64> = sqlx::query_scalar("SELECT id FROM users WHERE email = ?").bind(&email).fetch_optional(pool).await.ok().flatten();
    match exists {
        Some(id) => {
            let _ = sqlx::query("UPDATE users SET role = 'admin', blocked = 0 WHERE id = ?").bind(id).execute(pool).await;
        }
        None => {
            let hash = tokio::task::spawn_blocking(move || auth::hash_password(&pass)).await.unwrap_or_default();
            let _ = sqlx::query("INSERT INTO users (email, name, pass_hash, role, created_at) VALUES (?, 'Admin', ?, 'admin', ?)")
                .bind(&email)
                .bind(hash)
                .bind(db::now())
                .execute(pool)
                .await;
            println!("Admin account created for {email}");
        }
    }
}

#[tokio::main]
async fn main() {
    let db_path = std::env::var("DB_PATH").unwrap_or_else(|_| "kolom.db".into());
    let pool = db::connect(&db_path).await;
    db::migrate(&pool).await;
    seed_admin(&pool).await;

    let state = Arc::new(AppState { pool, rooms: Default::default(), login_fails: Default::default() });
    let dir = std::env::var("STATIC_DIR").unwrap_or_else(|_| "../web/out".into());

    // The same process serves the game page, so there is one thing to host and no cross-site setup.
    let files = ServeDir::new(&dir)
        .append_index_html_on_directories(true)
        .not_found_service(ServeFile::new(format!("{dir}/404.html")));

    let mut app = api::router(state.clone())
        .fallback_service(files)
        .layer(from_fn(cache_headers))
        .layer(CompressionLayer::new());
    if std::env::var("DEV_CORS").is_ok() {
        app = app.layer(CorsLayer::permissive());
    }

    let port = std::env::var("PORT").unwrap_or_else(|_| "8080".into());
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}")).await.expect("cannot bind the port");
    println!("Kolom Khela on http://localhost:{port}");
    axum::serve(listener, app).with_graceful_shutdown(async { let _ = tokio::signal::ctrl_c().await; }).await.unwrap();
}
