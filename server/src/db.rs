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
        // One row per room ever opened, kept after it closes: who opened it, how long it lived, how many matches.
        format!("CREATE TABLE IF NOT EXISTS rooms_log (
            id {pk},
            code TEXT NOT NULL,
            host_id BIGINT NOT NULL,
            host_name TEXT NOT NULL,
            created_at BIGINT NOT NULL,
            closed_at BIGINT,
            close_reason TEXT,
            games BIGINT NOT NULL DEFAULT 0,
            peak_people BIGINT NOT NULL DEFAULT 0,
            used_bots BIGINT NOT NULL DEFAULT 0
        )"),
        "CREATE INDEX IF NOT EXISTS idx_games_finished ON games(finished_at)".to_string(),
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
        // Full match records for the admin statistics.
        "ALTER TABLE games ADD COLUMN room_id BIGINT",
        "ALTER TABLE games ADD COLUMN difficulty TEXT NOT NULL DEFAULT 'medium'",
        "ALTER TABLE games ADD COLUMN started_at BIGINT NOT NULL DEFAULT 0",
        "ALTER TABLE games ADD COLUMN turns BIGINT NOT NULL DEFAULT 0",
        "ALTER TABLE games ADD COLUMN skips BIGINT NOT NULL DEFAULT 0",
        "ALTER TABLE games ADD COLUMN host_id BIGINT",
        "ALTER TABLE games ADD COLUMN end_reason TEXT NOT NULL DEFAULT ''",
        "ALTER TABLE games ADD COLUMN players BIGINT NOT NULL DEFAULT 0",
        "ALTER TABLE games ADD COLUMN bots BIGINT NOT NULL DEFAULT 0",
        "ALTER TABLE games ADD COLUMN guests BIGINT NOT NULL DEFAULT 0",
        "ALTER TABLE game_players ADD COLUMN seat BIGINT NOT NULL DEFAULT 0",
        "ALTER TABLE game_players ADD COLUMN pen TEXT NOT NULL DEFAULT ''",
        "ALTER TABLE game_players ADD COLUMN knocks BIGINT NOT NULL DEFAULT 0",
        "ALTER TABLE game_players ADD COLUMN falls BIGINT NOT NULL DEFAULT 0",
        "ALTER TABLE game_players ADD COLUMN lives BIGINT NOT NULL DEFAULT 0",
        "ALTER TABLE game_players ADD COLUMN bot BIGINT NOT NULL DEFAULT 0",
        "ALTER TABLE game_players ADD COLUMN guest BIGINT NOT NULL DEFAULT 0",
    ] {
        let _ = sqlx::query(q(s)).execute(pool).await;
    }
    // Rooms live in memory, so any room still open in the records died with the previous process.
    let _ = sqlx::query(q("UPDATE rooms_log SET closed_at = ?, close_reason = 'restart' WHERE closed_at IS NULL")).bind(now()).execute(pool).await;
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
    pub seat: i64,
    pub pen: String,
    pub knocks: i64,
    pub falls: i64,
    pub lives: i64,
    pub bot: bool,
    pub guest: bool,
}

/// The match itself, beside the per-player rows.
pub struct GameRecord<'a> {
    pub code: &'a str,
    pub room_id: Option<i64>,
    pub difficulty: &'a str,
    pub started_at: i64,
    pub turns: i64,
    pub skips: i64,
    pub host_id: i64,
    pub end_reason: &'a str,
}

pub async fn save_game(pool: &AnyPool, g: &GameRecord<'_>, results: &[PlayerResult]) {
    let winner = results.iter().find(|r| r.rank == 1).map(|r| r.user_id);
    let loser = results.iter().find(|r| r.looser).map(|r| r.user_id);
    let bots = results.iter().filter(|r| r.bot).count() as i64;
    let guests = results.iter().filter(|r| r.guest).count() as i64;
    let Ok(gid) = sqlx::query_scalar::<_, i64>(q("INSERT INTO games (code, finished_at, winner_id, loser_id, room_id, difficulty, started_at, turns, skips, host_id, end_reason, players, bots, guests)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING id"))
        .bind(g.code)
        .bind(now())
        .bind(winner)
        .bind(loser)
        .bind(g.room_id)
        .bind(g.difficulty)
        .bind(g.started_at)
        .bind(g.turns)
        .bind(g.skips)
        .bind(g.host_id)
        .bind(g.end_reason)
        .bind(results.len() as i64)
        .bind(bots)
        .bind(guests)
        .fetch_one(pool)
        .await
    else {
        return;
    };
    for r in results {
        let _ = sqlx::query(q("INSERT INTO game_players (game_id, user_id, name, coins, rank, looser, seat, pen, knocks, falls, lives, bot, guest) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"))
            .bind(gid)
            .bind(r.user_id)
            .bind(&r.name)
            .bind(r.coins)
            .bind(r.rank)
            .bind(r.looser as i64)
            .bind(r.seat)
            .bind(&r.pen)
            .bind(r.knocks)
            .bind(r.falls)
            .bind(r.lives)
            .bind(r.bot as i64)
            .bind(r.guest as i64)
            .execute(pool)
            .await;
    }
}

/// A room has opened. Returns its row id, which the room keeps so it can close the record later.
pub async fn room_opened(pool: &AnyPool, code: &str, host_id: i64, host_name: &str) -> Option<i64> {
    sqlx::query_scalar::<_, i64>(q("INSERT INTO rooms_log (code, host_id, host_name, created_at) VALUES (?, ?, ?, ?) RETURNING id"))
        .bind(code)
        .bind(host_id)
        .bind(host_name)
        .bind(now())
        .fetch_one(pool)
        .await
        .ok()
}

/// `reason`: "idle" (nobody left for a minute), "admin" (closed from the admin page) or "gone" (the server stopped it).
pub async fn room_closed(pool: &AnyPool, id: i64, reason: &str, games: i64, peak_people: i64, used_bots: bool) {
    let _ = sqlx::query(q("UPDATE rooms_log SET closed_at = ?, close_reason = ?, games = ?, peak_people = ?, used_bots = ? WHERE id = ?"))
        .bind(now())
        .bind(reason)
        .bind(games)
        .bind(peak_people)
        .bind(used_bots as i64)
        .bind(id)
        .execute(pool)
        .await;
}
