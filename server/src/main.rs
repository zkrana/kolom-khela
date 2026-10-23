mod api;
mod auth;
mod bot;
mod db;
mod email;
mod guard;
mod game;
mod pens;
mod room;

use axum::middleware::from_fn;
use sqlx::AnyPool;
use std::collections::HashMap;
use std::sync::Arc;
use tower_http::compression::CompressionLayer;
use tower_http::cors::CorsLayer;
use tower_http::services::{ServeDir, ServeFile};

pub struct AppState {
    pub pool: AnyPool,
    pub rooms: tokio::sync::Mutex<HashMap<String, room::RoomHandle>>,
    /// Every rate limit in one place: login attempts, sign-ups, guest joins, rooms, uploads.
    pub limits: guard::Limiter,
    /// Games finished in the last 24 hours, for the live counter. Counted at most every 30 seconds.
    pub day_games: std::sync::Mutex<Option<(std::time::Instant, i64)>>,
}

async fn seed_admin(pool: &AnyPool) {
    let (Ok(email), Ok(pass)) = (std::env::var("ADMIN_EMAIL"), std::env::var("ADMIN_PASSWORD")) else {
        println!("ADMIN_EMAIL / ADMIN_PASSWORD not set: no admin account was created.");
        return;
    };
    let email = email.trim().to_lowercase();
    let exists: Option<i64> = sqlx::query_scalar(db::q("SELECT id FROM users WHERE email = ?")).bind(&email).fetch_optional(pool).await.ok().flatten();
    match exists {
        Some(id) => {
            let _ = sqlx::query(db::q("UPDATE users SET role = 'admin', blocked = 0 WHERE id = ?")).bind(id).execute(pool).await;
        }
        None => {
            let hash = tokio::task::spawn_blocking(move || auth::hash_password(&pass)).await.unwrap_or_default();
            let _ = sqlx::query(db::q("INSERT INTO users (email, name, pass_hash, role, contact, created_at) VALUES (?, 'Admin', ?, 'admin', '', ?)"))
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

    let state = Arc::new(AppState { pool, rooms: Default::default(), limits: Default::default(), day_games: Default::default() });
    let dir = std::env::var("STATIC_DIR").unwrap_or_else(|_| "../web/out".into());

    // The same process serves the game page, so there is one thing to host and no cross-site setup.
    let files = ServeDir::new(&dir)
        .append_index_html_on_directories(true)
        .not_found_service(ServeFile::new(format!("{dir}/404.html")));

    let mut app = api::router(state.clone())
        .layer(axum::extract::DefaultBodyLimit::max(64 * 1024))
        .fallback_service(files)
        .layer(from_fn(guard::headers))
        .layer(CompressionLayer::new());
    if std::env::var("DEV_CORS").is_ok() {
        app = app.layer(CorsLayer::permissive());
    }

    let port = std::env::var("PORT").unwrap_or_else(|_| "8080".into());
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}")).await.expect("cannot bind the port");
    println!("Kolom Khela on http://localhost:{port}");
    axum::serve(listener, app).with_graceful_shutdown(async { let _ = tokio::signal::ctrl_c().await; }).await.unwrap();
}
