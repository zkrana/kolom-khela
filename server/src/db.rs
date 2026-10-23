use sqlx::any::AnyPoolOptions;
use sqlx::AnyPool;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

static IS_PG: AtomicBool = AtomicBool::new(false);

pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Queries are written once with `?` (or `?1`) placeholders. On Postgres they are rewritten to `$1`, `$2`...
/// Each distinct query is rewritten once and kept for the life of the process.
pub fn q(sql: &str) -> &'static str {
    static CACHE: OnceLock<Mutex<HashMap<String, &'static str>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    let mut map = cache.lock().unwrap();
    if let Some(s) = map.get(sql) {
        return s;
    }
    let out = if IS_PG.load(Ordering::Relaxed) { to_dollars(sql) } else { sql.to_string() };
    let leaked: &'static str = Box::leak(out.into_boxed_str());
    map.insert(sql.to_string(), leaked);
    leaked
}

fn to_dollars(sql: &str) -> String {
    let mut out = String::with_capacity(sql.len() + 8);
    let mut n = 0;
    let mut chars = sql.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '?' {
            if chars.peek().map_or(false, |d| d.is_ascii_digit()) {
                out.push('$');
            } else {
                n += 1;
                out.push_str(&format!("${n}"));
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// `DATABASE_URL=postgres://...` (a free Neon database, say) keeps data when the host restarts.
/// Without it the game uses a local SQLite file, which is fine for trying things out.
pub async fn connect(sqlite_path: &str) -> AnyPool {
    sqlx::any::install_default_drivers();
    let url = std::env::var("DATABASE_URL").ok().filter(|u| !u.trim().is_empty()).unwrap_or_else(|| format!("sqlite://{sqlite_path}?mode=rwc"));
    let pg = url.starts_with("postgres");
    IS_PG.store(pg, Ordering::Relaxed);
    println!("Database: {}", if pg { "Postgres" } else { "SQLite file" });
    AnyPoolOptions::new()
        .max_connections(5)
        .after_connect(move |conn, _| {
            Box::pin(async move {
                if !pg {
                    sqlx::query("PRAGMA foreign_keys = ON").execute(&mut *conn).await?;
                    sqlx::query("PRAGMA journal_mode = WAL").execute(&mut *conn).await?;
                }
                Ok(())
            })
        })
        .connect(&url)
        .await
        .expect("cannot open the database")
}

pub async fn migrate(pool: &AnyPool) {
    let pg = IS_PG.load(Ordering::Relaxed);
    let pk = if pg { "BIGSERIAL PRIMARY KEY" } else { "INTEGER PRIMARY KEY AUTOINCREMENT" };
    let blob = if pg { "BYTEA" } else { "BLOB" };
    let stmts = [
        format!("CREATE TABLE IF NOT EXISTS users (
            id {pk},
            email TEXT NOT NULL UNIQUE,
            name TEXT NOT NULL,
            pass_hash TEXT NOT NULL,
            role TEXT NOT NULL DEFAULT 'player',
            blocked BIGINT NOT NULL DEFAULT 0,
            created_at BIGINT NOT NULL
        )"),
        "CREATE TABLE IF NOT EXISTS sessions (
            token_hash TEXT PRIMARY KEY,
            user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
            created_at BIGINT NOT NULL
        )".to_string(),
        format!("CREATE TABLE IF NOT EXISTS games (
            id {pk},
            code TEXT NOT NULL,
            finished_at BIGINT NOT NULL,
            winner_id BIGINT,
            loser_id BIGINT
        )"),
        "CREATE TABLE IF NOT EXISTS game_players (
            game_id BIGINT NOT NULL REFERENCES games(id) ON DELETE CASCADE,
            user_id BIGINT NOT NULL,
            name TEXT NOT NULL,
            coins BIGINT NOT NULL,
            rank BIGINT NOT NULL,
            looser BIGINT NOT NULL DEFAULT 0
        )".to_string(),
        "CREATE INDEX IF NOT EXISTS idx_game_players_user ON game_players(user_id)".to_string(),
        format!("CREATE TABLE IF NOT EXISTS avatars (
            user_id BIGINT PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
            mime TEXT NOT NULL,
            data {blob} NOT NULL,
            updated_at BIGINT NOT NULL
        )"),
    ];
    for s in &stmts {
        sqlx::query(q(s)).execute(pool).await.expect("migration failed");
    }
    // Columns added after the first release. Running twice is harmless: a duplicate column just errors and is ignored.
    for s in [
        "ALTER TABLE users ADD COLUMN coins BIGINT NOT NULL DEFAULT 0",
        "ALTER TABLE users ADD COLUMN pen TEXT NOT NULL DEFAULT 'econo'",
        "ALTER TABLE users ADD COLUMN pens TEXT NOT NULL DEFAULT 'econo'",
        "ALTER TABLE users ADD COLUMN guest BIGINT NOT NULL DEFAULT 0",
        "ALTER TABLE users ADD COLUMN contact TEXT NOT NULL DEFAULT ''",
    ] {
        let _ = sqlx::query(q(s)).execute(pool).await;
    }
}

/// Coins for a win (the amount depends on the difficulty), with a daily cap so two accounts cannot farm
/// each other. Returns what was actually paid.
pub async fn reward_win(pool: &AnyPool, user_id: i64, amount: i64) -> i64 {
    let recent: i64 = sqlx::query_scalar(q("SELECT COUNT(*) FROM games WHERE winner_id = ? AND finished_at > ?"))
        .bind(user_id)
        .bind(now() - 86_400)
        .fetch_one(pool)
        .await
        .unwrap_or(0);
    if recent >= crate::pens::DAILY_REWARDED_WINS {
        return 0;
    }
    let amount = amount.clamp(0, 1000);
    let _ = sqlx::query(q("UPDATE users SET coins = coins + ? WHERE id = ?")).bind(amount).bind(user_id).execute(pool).await;
    amount
}

pub struct PlayerResult {
    pub user_id: i64,
    pub name: String,
    pub coins: i64,
    pub rank: i64,
    pub looser: bool,
}

pub async fn save_game(pool: &AnyPool, code: &str, results: &[PlayerResult]) {
    let winner = results.iter().find(|r| r.rank == 1).map(|r| r.user_id);
    let loser = results.iter().find(|r| r.looser).map(|r| r.user_id);
    let Ok(gid) = sqlx::query_scalar::<_, i64>(q("INSERT INTO games (code, finished_at, winner_id, loser_id) VALUES (?, ?, ?, ?) RETURNING id"))
        .bind(code)
        .bind(now())
        .bind(winner)
        .bind(loser)
        .fetch_one(pool)
        .await
    else {
        return;
    };
    for r in results {
        let _ = sqlx::query(q("INSERT INTO game_players (game_id, user_id, name, coins, rank, looser) VALUES (?, ?, ?, ?, ?, ?)"))
            .bind(gid)
            .bind(r.user_id)
            .bind(&r.name)
            .bind(r.coins)
            .bind(r.rank)
            .bind(r.looser as i64)
            .execute(pool)
            .await;
    }
}
